//! 编辑区与预览区的渲染
//!
//! 两个函数都返回 `(当前滚动偏移, 最大可滚动距离, 是否用户滚动)`，
//! 供 main 里的滚动随动逻辑使用。

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// 预览区最大阅读宽度（再宽的行会显得难读）
const MAX_CONTENT_WIDTH: f32 = 860.0;

/// 编辑器 TextEdit 的固定 Id（查找跳转需要用它读写光标状态）
pub fn editor_id() -> egui::Id {
    egui::Id::new("ramble_editor")
}

/// 编辑区：返回 (当前偏移, 最大偏移, 是否用户滚动, 内容是否修改, 跳转是否已应用)
pub fn editor_ui(
    ui: &mut egui::Ui,
    source: &mut String,
    forced: Option<f32>,
    last_offset: &mut Option<f32>,
    jump: Option<(usize, usize)>,
    highlight: Option<(usize, usize)>,
) -> (f32, f32, bool, bool, bool, Option<f32>) {
    let id = editor_id();

    // 应用查找跳转：只设置选区并滚动过去，不抢焦点。
    // 否则在查找框里每输入一个字符，焦点都会被抢回编辑器，导致无法继续输入中文。
    let mut jump_applied = false;
    if let Some((from, to)) = jump {
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(
            egui::text::CCursor::new(from),
            egui::text::CCursor::new(to),
        )));
        state.store(ui.ctx(), id);
        jump_applied = true;
    }

    let mut sa = egui::ScrollArea::vertical()
        .id_salt("editor_scroll")
        .auto_shrink(false);
    if let Some(y) = forced {
        sa = sa.vertical_scroll_offset(y);
    }
    // 自定义排版：把行高放大到字号的 1.5 倍，长文阅读不至于太紧凑
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
    let line_height = (font_id.size * 1.5).round();
    let mut layouter = move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = egui::text::LayoutJob::single_section(
            text.as_str().to_owned(),
            egui::text::TextFormat {
                font_id: font_id.clone(),
                line_height: Some(line_height),
                color: ui.visuals().text_color(),
                ..Default::default()
            },
        );
        job.wrap.max_width = wrap;
        // 保留行尾空白，否则输入空格时视觉反馈很怪
        job.keep_trailing_whitespace = true;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let mut jump_rect: Option<egui::Rect> = None;
    let out = sa.show(ui, |ui| {
        // 内边距让文字不贴边；同时只依赖 TextEdit 的 changed() 判断修改，
        // 不再每帧克隆整篇文档（大文档下这是最大的一笔无谓内存开销）
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                let out = egui::TextEdit::multiline(source)
                    .id(id)
                    .font(egui::TextStyle::Monospace)
                    .layouter(&mut layouter)
                    .desired_width(ui.available_width())
                    .desired_rows(200)
                    .show(ui);
                if let Some((from, to)) = highlight {
                    let start = out
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(from));
                    let end = out.galley.pos_from_cursor(egui::text::CCursor::new(to));
                    let rect = egui::Rect::from_min_max(start.min, end.max)
                        .translate(out.galley_pos.to_vec2());
                    // 编辑器没有焦点时，egui 不会画选区；这里补一个黄色高亮，
                    // 保证在查找框里输入时也能看到当前匹配。
                    if !out.response.response.has_focus() {
                        let dark = ui.visuals().dark_mode;
                        let fill = if dark {
                            egui::Color32::from_rgba_unmultiplied(255, 200, 60, 70)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(255, 214, 64, 90)
                        };
                        let line = if dark {
                            egui::Color32::from_rgb(255, 210, 90)
                        } else {
                            egui::Color32::from_rgb(196, 145, 0)
                        };
                        let r = rect.expand(1.0);
                        ui.painter().rect_filled(r, 2.0, fill);
                        ui.painter().rect_stroke(
                            r,
                            2.0,
                            egui::Stroke::new(1.0, line),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if jump_applied {
                        jump_rect = Some(rect);
                    }
                }
                out.response.response.changed()
            })
            .inner
    });

    let y = out.state.offset.y;
    let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    let user = forced.is_none() && last_offset.map(|l| (l - y).abs() > 1.0).unwrap_or(false);
    *last_offset = Some(y);
    // 下一帧直接把编辑区滚到匹配位置（滚动目标在内容布局完成后才能算出来）
    let scroll_to = jump_rect.map(|rect| {
        (y + (rect.center().y - out.inner_rect.center().y)).clamp(0.0, max)
    });
    (y, max, user, out.inner, jump_applied, scroll_to)
}

/// 预览区：返回 (当前偏移, 最大偏移, 是否用户滚动)
pub fn preview_ui(
    ui: &mut egui::Ui,
    cache: &mut CommonMarkCache,
    markdown: &str,
    uri_base: &str,
    forced: Option<f32>,
    last_offset: &mut Option<f32>,
) -> (f32, f32, bool) {
    ui.style_mut().url_in_tooltip = true;
    let mut sa = egui::ScrollArea::vertical()
        .id_salt("preview_scroll")
        .auto_shrink(false);
    if let Some(y) = forced {
        sa = sa.vertical_scroll_offset(y);
    }
    let out = sa.show(ui, |ui| {
        if markdown.trim().is_empty() {
            empty_state(ui);
            return;
        }

        // 内容居中并限制最大行宽，阅读体验接近 Typora / VSCode Markdown 预览
        // 左右至少留出 32px 缩进，避免文字/图片贴住分栏线与滚动条
        const SIDE_PADDING: f32 = 32.0;
        let avail = ui.available_width();
        let width = (avail - SIDE_PADDING * 2.0)
            .max(80.0)
            .min(MAX_CONTENT_WIDTH);
        let pad = ((avail - width) * 0.5).max(0.0);
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    CommonMarkViewer::new()
                        .max_image_width(Some(width.max(1.0) as usize))
                        // 图片相对路径以文档所在目录为基准
                        .default_implicit_uri_scheme(uri_base.to_owned())
                        .indentation_spaces(3)
                        .show(ui, cache, markdown);
                },
            );
        });
        ui.add_space(96.0);
    });
    let y = out.state.offset.y;
    let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    let user = forced.is_none() && last_offset.map(|l| (l - y).abs() > 1.0).unwrap_or(false);
    *last_offset = Some(y);
    (y, max, user)
}

fn empty_state(ui: &mut egui::Ui) {
    let weak = ui.visuals().weak_text_color();
    ui.add_space(120.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("Ramble")
                .size(26.0)
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        ui.label(egui::RichText::new("Markdown").size(13.0).color(weak));
        ui.add_space(14.0);
        ui.label(
            egui::RichText::new("Ctrl+O 打开文档，或把 .md 文件拖进窗口")
                .size(12.0)
                .color(weak),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("支持本地图片与 mermaid 流程图")
                .size(12.0)
                .color(weak),
        );
    });
    ui.add_space(120.0);
}
