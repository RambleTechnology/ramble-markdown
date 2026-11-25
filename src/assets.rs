//! 本地图片资源的定位与图片管线配置
//!
//! markdown 里的相对路径图片（`![](images/a.png)`）默认相对于进程工作目录解析，
//! 这里把「文档所在目录」转成 `file://` 前缀，交给 `egui_commonmark` 拼接，
//! 这样在任意目录打开文档，图片都能按文档所在位置找到。

use eframe::egui;
use std::path::Path;

/// 把本地路径转成 `file://` URI（egui_extras 的 FileLoader 只接受这种形式）。
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

/// 文档目录的 `file://` 前缀（以 `/` 结尾）。
///
/// 没有文档路径（新建未保存）时退化为当前工作目录。
pub fn image_base(doc: Option<&Path>) -> String {
    let dir = doc
        .and_then(|p| p.parent())
        // 相对路径（如 `note.md`）没有父目录，退回当前工作目录
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok());
    match dir {
        Some(dir) => {
            let mut base = file_uri(&dir);
            if !base.ends_with('/') {
                base.push('/');
            }
            base
        }
        None => "file:///".to_string(),
    }
}

/// 图片相关的上下文配置（启动时调用一次）。
pub fn configure(ctx: &egui::Context) {
    // 纹理上传后立刻释放解码得到的像素副本。
    // egui_extras 的图片加载器默认会长期缓存 ColorImage，等于同一张图
    // 同时占用「内存像素 + 显存纹理」两份空间；开启后只保留纹理。
    ctx.options_mut(|o| o.reduce_texture_memory = true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_handles_windows_and_unix_paths() {
        assert_eq!(file_uri(Path::new("E:/docs/a.png")), "file:///E:/docs/a.png");
        assert_eq!(file_uri(Path::new("/tmp/a.png")), "file:///tmp/a.png");
    }

    #[test]
    fn image_base_ends_with_slash() {
        let base = image_base(Some(Path::new("E:/docs/note.md")));
        assert_eq!(base, "file:///E:/docs/");
        assert!(image_base(Some(Path::new("/home/u/note.md"))).starts_with("file:///"));
        // 相对文件名没有父目录时回退到工作目录，而不是 file:///
        assert!(image_base(Some(Path::new("note.md"))).len() > "file:///".len());
    }
}
