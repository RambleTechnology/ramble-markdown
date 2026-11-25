//! mermaid 图表渲染
//!
//! 渲染流程：mermaid 源码 → `mermaid-rs-renderer` 生成 SVG → resvg 栅格化为 PNG
//! → 写入临时目录，预览里用 `![](file:///...png)` 引用。
//!
//! 缓存策略（内存优先）：
//! - 以「源码 + 主题」的 64 位哈希作为键，只缓存 PNG 文件路径，不缓存图片内容本身；
//! - 每次展开文档后，只保留当前文档仍在使用的图表，旧版本图表连同临时文件一起删除，
//!   因此缓存大小恒等于「当前文档的图表数量」，不会随编辑次数无限增长；
//! - 预览文本复用调用方缓冲区，避免每次编辑都分配一整份新字符串。

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// 临时图保留时长：异常退出留下的文件最多存活一天
const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

enum Entry {
    /// 渲染成功，PNG 已落盘
    Png(PathBuf),
    /// 渲染失败的错误信息
    Error(String),
}

pub struct MermaidCache {
    blocks: HashMap<u64, Entry>,
    /// 当前文档用到的键（展开结束后据此回收旧缓存）
    used: Vec<u64>,
    dir: PathBuf,
}

impl Default for MermaidCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidCache {
    pub fn new() -> Self {
        let dir = std::env::temp_dir()
            .join("ramble-markdown")
            .join("mermaid");
        let _ = std::fs::create_dir_all(&dir);
        let cache = Self {
            blocks: HashMap::new(),
            used: Vec::new(),
            dir,
        };
        cache.cleanup_stale();
        cache
    }

    /// 把 markdown 中的 ```mermaid 代码块替换为图片引用，结果写入 `out`（会先清空）。
    ///
    /// `dark` 同时影响输出文件与缓存键：界面切换主题后图表颜色随之变化。
    pub fn expand(&mut self, md: &str, dark: bool, out: &mut String) {
        out.clear();
        // 只按原文长度预留，避免超大文档时预留出数倍空间
        out.reserve(md.len());
        self.used.clear();

        let mut lines = md.split_inclusive('\n');

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            let fence = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let info = fence.map(|_| {
                trimmed
                    .trim_start_matches(|c| c == '`' || c == '~')
                    .trim()
                    .to_ascii_lowercase()
            });

            let is_mermaid = match (&fence, &info) {
                (Some(_), Some(info)) => {
                    // 匹配 ```mermaid / ```mermaid extra / ~~~mermaid
                    info == "mermaid" || info.starts_with("mermaid ") || info.starts_with("mermaid\t")
                }
                _ => false,
            };

            if !is_mermaid {
                out.push_str(line);
                continue;
            }

            // 收集代码块内容，直到遇到闭合围栏
            let fence_char = fence.unwrap();
            let mut code = String::new();
            let mut closing: Option<&str> = None;
            for l in lines.by_ref() {
                let t = l.trim();
                if t.starts_with(fence_char)
                    && t.trim_start_matches(|c| c == fence_char).trim().is_empty()
                    && t.chars().count() >= 3
                {
                    closing = Some(l);
                    break;
                }
                code.push_str(l);
            }

            // 未闭合（用户正在输入）或内容为空：原文照搬，绝不改动用户输入
            if closing.is_none() {
                out.push_str(line);
                out.push_str(&code);
                continue;
            }
            if code.trim().is_empty() {
                out.push_str(line);
                out.push_str(&code);
                out.push_str(closing.unwrap());
                continue;
            }

            let code = code.trim_end_matches(['\n', '\r']);
            let key = key_of(code, dark);
            self.used.push(key);

            match self.lookup(key) {
                Some(Ok(uri)) => {
                    let _ = write!(out, "![mermaid]({uri})\n\n");
                }
                Some(Err(err)) => push_fallback(out, code, &err),
                None => match self.render(key, code, dark) {
                    Ok(uri) => {
                        let _ = write!(out, "![mermaid]({uri})\n\n");
                    }
                    Err(err) => push_fallback(out, code, &err),
                },
            }
        }

        self.prune();
    }

    /// 命中缓存时返回 URI 或错误信息（只复制很小的字符串，不复制图片内容）
    fn lookup(&self, key: u64) -> Option<Result<String, String>> {
        self.blocks.get(&key).map(|entry| match entry {
            Entry::Png(path) => Ok(crate::assets::file_uri(path)),
            Entry::Error(err) => Err(err.clone()),
        })
    }

    /// 渲染并写入 PNG，返回可直接放进 markdown 的 URI。
    fn render(&mut self, key: u64, code: &str, dark: bool) -> Result<String, String> {
        let path = self.dir.join(format!("{key:016x}.png"));
        match render_png(code, dark, &path) {
            Ok(()) => {
                let uri = crate::assets::file_uri(&path);
                self.blocks.insert(key, Entry::Png(path));
                Ok(uri)
            }
            Err(err) => {
                let _ = std::fs::remove_file(&path);
                self.blocks.insert(key, Entry::Error(err.clone()));
                Err(err)
            }
        }
    }

    /// 只保留当前文档用到的图表，其余条目连同临时文件一起回收。
    fn prune(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.blocks.retain(|key, entry| {
            if used.contains(key) {
                return true;
            }
            if let Entry::Png(path) = entry {
                let _ = std::fs::remove_file(path);
            }
            false
        });
    }

    /// 清理上次运行遗留的临时图（正常退出时文件也留在盘上供其它窗口复用）
    fn cleanup_stale(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            let stale = meta
                .modified()
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age > STALE_AFTER);
            if stale {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn key_of(code: &str, dark: bool) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    dark.hash(&mut hasher);
    0x7261_6d62_6c65_0001u64.hash(&mut hasher);
    hasher.finish()
}

fn push_fallback(out: &mut String, code: &str, err: &str) {
    out.push_str("```mermaid\n");
    out.push_str(code);
    out.push_str("\n```\n\n");
    let first = err.lines().next().unwrap_or("未知错误");
    let _ = write!(out, "> ⚠️ 图表渲染失败：{first}\n\n");
}

/// mermaid 源码 → PNG 文件
fn render_png(code: &str, dark: bool, path: &Path) -> Result<(), String> {
    let theme = theme_for(dark);
    let cfg = mermaid_rs_renderer::RenderConfig {
        background: theme.background.clone(),
        ..Default::default()
    };
    let options = mermaid_rs_renderer::RenderOptions {
        theme: theme.clone(),
        ..Default::default()
    };

    let svg =
        mermaid_rs_renderer::render_with_options(code, options).map_err(|e| e.to_string())?;
    mermaid_rs_renderer::write_output_png(&svg, path, &cfg, &theme).map_err(|e| e.to_string())
}

/// 图表主题：跟随界面深浅色，并优先使用系统中文字体，避免中文标签缺字
fn theme_for(dark: bool) -> mermaid_rs_renderer::Theme {
    let mut theme = if dark {
        mermaid_rs_renderer::Theme::dark()
    } else {
        mermaid_rs_renderer::Theme::modern()
    };
    theme.font_family = format!(
        "\"{}\", {}",
        crate::theme::CJK_FAMILY,
        theme.font_family
    );
    // 背景与预览区一致，图表不再出现突兀的白底
    let bg = crate::theme::palette(dark).preview_bg;
    theme.background = format!("#{:02x}{:02x}{:02x}", bg.r(), bg.g(), bg.b());
    theme
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(cache: &mut MermaidCache, md: &str) -> String {
        let mut out = String::new();
        cache.expand(md, false, &mut out);
        out
    }

    #[test]
    fn renders_flowchart_to_file_uri() {
        let mut c = MermaidCache::new();
        let out = expand(
            &mut c,
            "# 标题\n\n```mermaid\nflowchart LR\n  A[开始] --> B{判断}\n  B -->|是| C[结束]\n```\n\n尾注\n",
        );
        assert!(out.contains("file:///"), "未生成文件引用：{out}");
        assert!(!out.contains("```mermaid"), "代码块未被替换：{out}");
        assert!(out.starts_with("# 标题"));
        assert!(out.trim_end().ends_with("尾注"));
        assert_eq!(c.blocks.len(), 1);
    }

    #[test]
    fn second_expand_uses_cache() {
        let mut c = MermaidCache::new();
        let md = "```mermaid\nflowchart LR\n  X --> Y\n```\n";
        let a = expand(&mut c, md);
        let b = expand(&mut c, md);
        assert_eq!(a, b);
        assert_eq!(c.blocks.len(), 1);
    }

    #[test]
    fn cache_drops_diagrams_no_longer_used() {
        let mut c = MermaidCache::new();
        expand(&mut c, "```mermaid\nflowchart LR\n  A --> B\n```\n");
        assert_eq!(c.blocks.len(), 1);
        expand(&mut c, "```mermaid\nflowchart LR\n  C --> D\n```\n");
        assert_eq!(c.blocks.len(), 1, "旧图表应被回收");
    }

    #[test]
    fn invalid_diagram_falls_back_to_code_block() {
        let mut c = MermaidCache::new();
        let out = expand(&mut c, "```mermaid\n这不是任何合法的图表语法 %%&&\n```\n");
        assert!(
            out.contains("file:///") || (out.contains("```mermaid") && out.contains("渲染失败")),
            "降级输出异常：{out}"
        );
    }

    #[test]
    fn other_code_blocks_untouched() {
        let mut c = MermaidCache::new();
        let md = "```rust\nfn main() {}\n```\n\n```mermaidx\nnot mermaid\n```\n";
        assert_eq!(expand(&mut c, md), md);
    }

    #[test]
    fn unclosed_fence_is_preserved() {
        let mut c = MermaidCache::new();
        let md = "```mermaid\nflowchart LR\n  A --> B\n";
        assert_eq!(expand(&mut c, md), md);
    }
}
