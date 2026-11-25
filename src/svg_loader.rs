//! 按 MIME / 字节内容识别 SVG 的图片加载器。
//!
//! egui_extras 自带的 SvgLoader 只认 `.svg` 后缀的 URI，而 shields.io 徽章等
//! 常见 SVG 链接无后缀，会落到位图解码器上报 FormatNotSupported。
//! 本加载器不看后缀，下载后按 Content-Type 或文件头判定 SVG 再栅格化。

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use egui::load::{BytesPoll, ImageLoadResult, ImageLoader, ImagePoll, LoadError, SizeHint};
use egui::mutex::Mutex;

struct Entry {
    last_used: u64,
    result: Result<Arc<egui::ColorImage>, String>,
}

struct State {
    pass_index: u64,
    cache: HashMap<String, HashMap<SizeHint, Entry>>,
}

pub struct SvgMimeLoader {
    options: resvg::usvg::Options<'static>,
    state: Mutex<State>,
}

impl SvgMimeLoader {
    pub fn new() -> Self {
        let mut options = resvg::usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        Self {
            options,
            state: Mutex::new(State {
                pass_index: 0,
                cache: HashMap::new(),
            }),
        }
    }
}

fn is_svg(mime: Option<&str>, bytes: &[u8]) -> bool {
    if mime.is_some_and(|m| m.starts_with("image/svg+xml")) {
        return true;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg"))
}

impl ImageLoader for SvgMimeLoader {
    fn id(&self) -> &str {
        egui::generate_loader_id!(SvgMimeLoader)
    }

    fn load(&self, ctx: &egui::Context, uri: &str, size_hint: SizeHint) -> ImageLoadResult {
        let mut state = self.state.lock();
        let pass_index = state.pass_index;
        if let Some(entry) = state
            .cache
            .get_mut(uri)
            .and_then(|bucket| bucket.get_mut(&size_hint))
        {
            entry.last_used = pass_index;
            return match entry.result.clone() {
                Ok(image) => Ok(ImagePoll::Ready { image }),
                Err(err) => Err(LoadError::Loading(err)),
            };
        }
        match ctx.try_load_bytes(uri) {
            Ok(BytesPoll::Ready { bytes, mime, .. }) => {
                if !is_svg(mime.as_deref(), &bytes) {
                    return Err(LoadError::NotSupported);
                }
                let result =
                    egui_extras::image::load_svg_bytes_with_size(&bytes, size_hint, &self.options)
                        .map(Arc::new);
                state.cache.entry(uri.to_owned()).or_default().insert(
                    size_hint,
                    Entry {
                        last_used: pass_index,
                        result: result.clone(),
                    },
                );
                match result {
                    Ok(image) => Ok(ImagePoll::Ready { image }),
                    Err(err) => Err(LoadError::Loading(err)),
                }
            }
            Ok(BytesPoll::Pending { size }) => Ok(ImagePoll::Pending { size }),
            Err(err) => Err(err),
        }
    }

    fn forget(&self, uri: &str) {
        self.state.lock().cache.remove(uri);
    }

    fn forget_all(&self) {
        self.state.lock().cache.clear();
    }

    fn byte_size(&self) -> usize {
        self.state
            .lock()
            .cache
            .values()
            .flat_map(|bucket| bucket.values())
            .map(|entry| match &entry.result {
                Ok(image) => image.pixels.len() * std::mem::size_of::<egui::Color32>(),
                Err(err) => err.len(),
            })
            .sum()
    }

    fn end_pass(&self, pass_index: u64) {
        let mut state = self.state.lock();
        state.pass_index = pass_index;
        // 同一 URI 因缩放产生多份尺寸时，淘汰连续两帧未使用的，防止缓存膨胀
        state.cache.retain(|_, bucket| {
            if 2 <= bucket.len() {
                bucket.retain(|_, entry| pass_index <= entry.last_used + 1);
            }
            !bucket.is_empty()
        });
    }
}