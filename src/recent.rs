//! 最近打开文件列表的持久化
//!
//! 存储位置：`%APPDATA%\ramble-markdown\recent.txt`（每行一个路径），
//! 获取不到 APPDATA 时退化为 exe 所在目录。纯文本，便于用户手工清理。

use std::path::{Path, PathBuf};

const MAX_ITEMS: usize = 10;

pub struct RecentFiles {
    file: Option<PathBuf>,
    items: Vec<PathBuf>,
}

impl RecentFiles {
    pub fn load() -> Self {
        let file = config_file();
        let items = file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|s| {
                s.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Self { file, items }
    }

    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    /// 记录一次打开：去重、置顶、截断，然后落盘
    pub fn add(&mut self, path: &Path) {
        let p = path.to_path_buf();
        self.items.retain(|x| x != &p);
        self.items.insert(0, p);
        self.items.truncate(MAX_ITEMS);
        self.save();
    }

    /// 移除某个条目（例如文件已被删除时）
    pub fn remove(&mut self, path: &Path) {
        self.items.retain(|x| x != path);
        self.save();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.save();
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let content = self
            .items
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(file, content);
    }
}

fn config_file() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join("ramble-markdown").join("recent.txt"));
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("recent.txt")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> RecentFiles {
        let file = std::env::temp_dir().join(format!("ramble-md-test-recent-{tag}.txt"));
        let _ = std::fs::remove_file(&file);
        RecentFiles {
            file: Some(file),
            items: Vec::new(),
        }
    }

    #[test]
    fn add_dedupes_and_limits() {
        let mut r = temp_store("dedupe");
        for i in 0..15 {
            r.add(Path::new(&format!("C:/docs/file{i}.md")));
        }
        assert_eq!(r.items().len(), MAX_ITEMS);
        assert_eq!(r.items()[0], PathBuf::from("C:/docs/file14.md"));

        // 重复打开会置顶而不是新增
        r.add(Path::new("C:/docs/file14.md"));
        assert_eq!(r.items().len(), MAX_ITEMS);
        assert_eq!(r.items()[0], PathBuf::from("C:/docs/file14.md"));
    }

    #[test]
    fn persists_and_reloads() {
        let mut r = temp_store("persist");
        let file = r.file.clone().unwrap();
        r.add(Path::new("C:/a.md"));
        r.add(Path::new("C:/b.md"));
        r.remove(Path::new("C:/a.md"));

        let reloaded = RecentFiles {
            file: Some(file.clone()),
            items: std::fs::read_to_string(&file)
                .unwrap()
                .lines()
                .map(PathBuf::from)
                .collect(),
        };
        assert_eq!(reloaded.items(), &[PathBuf::from("C:/b.md")]);
        let _ = std::fs::remove_file(file);
    }
}
