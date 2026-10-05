//! mermaid 图表渲染
//!
//! 渲染流程：mermaid 源码 → `mermaid-rs-renderer` 生成 SVG → resvg 栅格化为 PNG
//! → base64 编码为 data URI，嵌入 markdown 交给 `egui_commonmark` 显示。
//!
//! 渲染结果按「源码原文」缓存，编辑正文时不会重复渲染未改动的图表。
//! 渲染失败时保留原始代码块，并追加一行错误提示，便于定位语法问题。

use base64::Engine;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

enum Entry {
    /// 渲染成功的 data URI（PNG）
    Png(String),
    /// 渲染失败的错误信息
    Error(String),
}

#[derive(Default)]
pub struct MermaidCache {
    blocks: HashMap<String, Entry>,
}

impl MermaidCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 把 markdown 中的 ```mermaid 代码块替换为内嵌图片
    pub fn expand(&mut self, md: &str) -> String {
        let mut out = String::with_capacity(md.len());
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

            let code_trimmed = code.trim_end_matches(['\n', '\r']).to_string();

            // 未闭合（用户正在输入）或内容为空：原文照搬，绝不改动用户输入
            if closing.is_none() {
                out.push_str(line);
                out.push_str(&code);
                continue;
            }
            if code_trimmed.trim().is_empty() {
                out.push_str(line);
                out.push_str(&code);
                out.push_str(closing.unwrap());
                continue;
            }

            match self.render(&code_trimmed) {
                Ok(uri) => {
                    out.push_str(&format!("![mermaid]({})\n\n", uri));
                }
                Err(err) => {
                    out.push_str("```mermaid\n");
                    out.push_str(&code);
                    out.push_str("```\n\n");
                    let first = err.lines().next().unwrap_or("未知错误");
                    out.push_str(&format!("> ⚠️ 图表渲染失败：{}\n\n", first));
                }
            }
        }

        out
    }

    /// 渲染单个图表，返回 data URI；命中缓存则直接返回
    fn render(&mut self, code: &str) -> Result<String, String> {
        if let Some(entry) = self.blocks.get(code) {
            return match entry {
                Entry::Png(uri) => Ok(uri.clone()),
                Entry::Error(err) => Err(err.clone()),
            };
        }
        match render_png_data_uri(code) {
            Ok(uri) => {
                self.blocks
                    .insert(code.to_string(), Entry::Png(uri.clone()));
                Ok(uri)
            }
            Err(err) => {
                self.blocks
                    .insert(code.to_string(), Entry::Error(err.clone()));
                Err(err)
            }
        }
    }
}

/// mermaid 源码 → PNG data URI
fn render_png_data_uri(code: &str) -> Result<String, String> {
    let svg = mermaid_rs_renderer::render(code).map_err(|e| e.to_string())?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!("ramble-md-mermaid-{:x}.png", hasher.finish()));

    let theme = mermaid_rs_renderer::Theme::modern();
    let cfg = mermaid_rs_renderer::RenderConfig::default();
    mermaid_rs_renderer::write_output_png(&svg, &tmp, &cfg, &theme).map_err(|e| e.to_string())?;

    let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_flowchart_to_data_uri() {
        let mut c = MermaidCache::new();
        let out = c.expand("# 标题\n\n```mermaid\nflowchart LR\n  A[开始] --> B[结束]\n```\n\n尾注\n");
        assert!(out.contains("data:image/png;base64,"), "未生成内嵌图：{out}");
        assert!(!out.contains("```mermaid"), "代码块未被替换：{out}");
        assert!(out.starts_with("# 标题"));
        assert!(out.trim_end().ends_with("尾注"));
    }

    #[test]
    fn second_expand_uses_cache() {
        let mut c = MermaidCache::new();
        let md = "```mermaid\nflowchart LR\n  X --> Y\n```\n";
        let a = c.expand(md);
        let b = c.expand(md);
        assert_eq!(a, b);
        assert_eq!(c.blocks.len(), 1);
    }

    #[test]
    fn invalid_diagram_falls_back_to_code_block() {
        let mut c = MermaidCache::new();
        let out = c.expand("```mermaid\n这不是任何合法的图表语法 %%&&\n```\n");
        // 要么渲染成功，要么保留源码并给出提示，不能 panic 或丢失内容
        assert!(
            out.contains("data:image/png;base64,")
                || (out.contains("```mermaid") && out.contains("渲染失败")),
            "降级输出异常：{out}"
        );
    }

    #[test]
    fn other_code_blocks_untouched() {
        let mut c = MermaidCache::new();
        let md = "```rust\nfn main() {}\n```\n\n```mermaidx\nnot mermaid\n```\n";
        assert_eq!(c.expand(md), md);
    }

    #[test]
    fn unclosed_fence_is_preserved() {
        let mut c = MermaidCache::new();
        let md = "```mermaid\nflowchart LR\n  A --> B\n";
        assert_eq!(c.expand(md), md);
    }
}
