//! 编辑区与预览区的渲染
//!
//! 两个函数都返回 `(当前滚动偏移, 最大可滚动距离, 是否用户滚动)`，
//! 供 main 里的滚动随动逻辑使用。

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

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
) -> (f32, f32, bool, bool, bool) {
    let id = editor_id();

    // 应用查找跳转：设置选区并聚焦，egui 会滚动到光标处
    let mut jump_applied = false;
    if let Some((from, to)) = jump {
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(
            egui::text::CCursor::new(from),
            egui::text::CCursor::new(to),
        )));
        state.store(ui.ctx(), id);
        ui.ctx().memory_mut(|m| m.request_focus(id));
        jump_applied = true;
    }

    let before = source.clone();
    let mut sa = egui::ScrollArea::vertical()
        .id_salt("editor_scroll")
        .auto_shrink(false);
    if let Some(y) = forced {
        sa = sa.vertical_scroll_offset(y);
    }
    let out = sa.show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(source)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .desired_width(ui.available_width())
                .desired_rows(200),
        );
        if jump_applied {
            ui.scroll_to_cursor(Some(egui::Align::Center));
        }
    });

    let y = out.state.offset.y;
    let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    let user = forced.is_none() && last_offset.map(|l| (l - y).abs() > 1.0).unwrap_or(false);
    *last_offset = Some(y);
    (y, max, user, *source != before, jump_applied)
}

/// 预览区：返回 (当前偏移, 最大偏移, 是否用户滚动)
pub fn preview_ui(
    ui: &mut egui::Ui,
    cache: &mut CommonMarkCache,
    markdown: &str,
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
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(ui.available_width());
            CommonMarkViewer::new().show(ui, cache, markdown);
        });
        ui.add_space(60.0);
    });
    let y = out.state.offset.y;
    let max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    let user = forced.is_none() && last_offset.map(|l| (l - y).abs() > 1.0).unwrap_or(false);
    *last_offset = Some(y);
    (y, max, user)
}
