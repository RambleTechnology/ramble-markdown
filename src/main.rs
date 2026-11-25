#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Ramble Markdown —— 轻量级 Markdown 编辑与实时预览桌面应用
//!
//! 模块划分：
//! - `assets`：本地图片路径解析与图片管线配置
//! - `theme`：VSCode 风格配色、字体加载
//! - `pane`：编辑区 / 预览区渲染（含滚动随动所需的返回值）
//! - `mermaid`：mermaid 图表渲染与磁盘缓存
//! - `find`：查找 / 替换
//! - `recent`：最近打开文件持久化
//! - `svg_loader`：按 MIME / 内容识别 SVG 的图片加载器

mod assets;
mod find;
mod mermaid;
mod pane;
mod recent;
mod svg_loader;
mod theme;

use eframe::egui;
use egui_commonmark::CommonMarkCache;
use find::FindState;
use mermaid::MermaidCache;
use recent::RecentFiles;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 视图模式
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    /// 分栏：编辑 + 预览
    Split,
    /// 仅编辑
    Editor,
    /// 仅预览
    Preview,
}

/// 待确认的未保存操作
#[derive(Clone, PartialEq)]
enum PendingAction {
    New,
    Open(PathBuf),
    Exit,
}

struct RambleApp {
    /// Markdown 源文本（编辑区内容，保持原始输入）
    source: String,
    /// 交给预览区渲染的文本（mermaid 代码块已替换为图片引用，复用同一缓冲区）
    prepared: String,
    /// prepared 是否需要重新生成
    needs_prepare: bool,
    /// 预览里相对图片路径的基准 URI（文档所在目录）
    uri_base: String,
    /// 当前文件路径
    path: Option<PathBuf>,
    /// 是否有未保存的修改
    dirty: bool,
    /// markdown 渲染缓存（语法高亮、图片等）
    cache: CommonMarkCache,
    /// mermaid 渲染缓存
    mermaid: MermaidCache,
    /// 查找替换状态
    find: FindState,
    /// 最近打开文件
    recent: RecentFiles,
    /// 错误提示
    error: Option<String>,
    /// 等待用户确认的操作
    pending: Option<PendingAction>,
    /// 用户已确认放弃未保存内容，真正退出中（避免重复拦截关闭事件）
    quitting: bool,
    /// 视图模式
    mode: ViewMode,
    /// 当前主题（None = 尚未初始化，跟随系统）
    dark: Option<bool>,
    /// 已应用的样式主题（避免每帧重复设置 visuals）
    styled: Option<bool>,
    /// 上次写入窗口标题的内容（避免每帧重复分配并调用系统接口）
    title_cache: String,
    // ── 缓存与内存控制 ────────────────────────────
    /// 字数统计缓存
    stats_text: String,
    stats_dirty: bool,
    /// 下一帧释放上一份文档的图片纹理
    release_images: bool,
    // ── 滚动随动状态 ──────────────────────────────
    last_ed_offset: Option<f32>,
    last_pv_offset: Option<f32>,
    ed_max: f32,
    pv_max: f32,
    apply_pv: Option<f32>,
    apply_ed: Option<f32>,
}

impl RambleApp {
    fn new() -> Self {
        // 支持命令行传入文件：ramble-markdown.exe README.md
        let initial = std::env::args().nth(1).map(PathBuf::from);
        let mut app = Self {
            source: String::new(),
            prepared: String::new(),
            needs_prepare: true,
            uri_base: assets::image_base(None),
            path: None,
            dirty: false,
            cache: CommonMarkCache::default(),
            mermaid: MermaidCache::new(),
            find: FindState::new(),
            recent: RecentFiles::load(),
            error: None,
            pending: None,
            quitting: false,
            mode: ViewMode::Split,
            dark: None,
            styled: None,
            title_cache: String::new(),
            stats_text: String::new(),
            stats_dirty: true,
            release_images: false,
            last_ed_offset: None,
            last_pv_offset: None,
            ed_max: 0.0,
            pv_max: 0.0,
            apply_pv: None,
            apply_ed: None,
        };
        if let Some(p) = initial {
            app.load(&p);
        }
        app
    }

    fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名.md".into())
    }

    fn title(&self) -> String {
        format!(
            "{}{} — Ramble Markdown",
            elide_middle(&self.file_name(), 44),
            if self.dirty { " *" } else { "" }
        )
    }

    /// 内容或主题变化后统一标记，保证预览与统计按需更新
    fn touch(&mut self) {
        self.dirty = true;
        self.needs_prepare = true;
        self.stats_dirty = true;
    }

    /// 重新生成预览文本（mermaid 图表在此渲染，未改动的图表走磁盘缓存）
    fn prepare(&mut self) {
        let dark = self.dark.unwrap_or(true);
        self.mermaid.expand(&self.source, dark, &mut self.prepared);
        self.needs_prepare = false;
    }

    fn load(&mut self, p: &Path) {
        match std::fs::read_to_string(p) {
            Ok(mut s) => {
                // 去掉 UTF-8 BOM：否则首行的 `#` 不会被视为标题标记，
                // Windows 记事本 / PowerShell 保存的文件都会带 BOM
                if s.starts_with('\u{feff}') {
                    s.drain(..3);
                }
                self.source = s;
                self.path = Some(p.to_path_buf());
                self.uri_base = assets::image_base(self.path.as_deref());
                self.dirty = false;
                self.error = None;
                self.needs_prepare = true;
                self.stats_dirty = true;
                // 切文档时释放上一篇文档的图片纹理与预览缓冲
                self.release_images = true;
                self.prepared.clear();
                self.prepared.shrink_to_fit();
                self.recent.add(p);
                self.find.close();
                self.reset_scroll_sync();
            }
            Err(e) => self.error = Some(format!("打开失败: {e}")),
        }
    }

    fn write_to(&mut self, p: &Path) {
        match std::fs::write(p, &self.source) {
            Ok(()) => {
                self.path = Some(p.to_path_buf());
                self.uri_base = assets::image_base(self.path.as_deref());
                self.dirty = false;
                self.error = None;
                self.recent.add(p);
            }
            Err(e) => self.error = Some(format!("保存失败: {e}")),
        }
    }

    /// 保存：有路径直接写，没有则走另存为对话框（返回是否完成保存动作）
    fn save(&mut self) -> bool {
        if let Some(p) = self.path.clone() {
            self.write_to(&p);
            true
        } else {
            self.save_as()
        }
    }

    fn save_as(&mut self) -> bool {
        let default_name = self.file_name();
        match rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .set_file_name(&default_name)
            .save_file()
        {
            Some(p) => {
                self.write_to(&p);
                true
            }
            None => false,
        }
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .add_filter("所有文件", &["*"])
            .pick_file()
        {
            self.open_path(p);
        }
    }

    /// 打开文件（带未保存确认）
    fn open_path(&mut self, p: PathBuf) {
        if self.dirty {
            self.pending = Some(PendingAction::Open(p));
        } else {
            self.load(&p);
        }
    }

    fn request_new(&mut self) {
        if self.dirty {
            self.pending = Some(PendingAction::New);
        } else {
            self.do_new();
        }
    }

    fn do_new(&mut self) {
        self.source.clear();
        self.prepared.clear();
        self.prepared.shrink_to_fit();
        self.path = None;
        self.uri_base = assets::image_base(None);
        self.dirty = false;
        self.error = None;
        self.needs_prepare = true;
        self.stats_dirty = true;
        self.release_images = true;
        self.find.close();
        self.reset_scroll_sync();
    }

    /// 未保存确认后真正执行挂起的动作
    fn complete_pending(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::New => self.do_new(),
            PendingAction::Open(p) => self.load(&p),
            PendingAction::Exit => {
                // 标记为正在退出，否则 Close 触发的 CloseRequested 会再次被拦截，
                // 导致"不保存"看起来没有反应
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// 切换视图模式时重置滚动同步状态
    fn set_mode(&mut self, mode: ViewMode) {
        if self.mode != mode {
            self.mode = mode;
            self.reset_scroll_sync();
        }
    }

    fn reset_scroll_sync(&mut self) {
        self.last_ed_offset = None;
        self.last_pv_offset = None;
        self.apply_ed = None;
        self.apply_pv = None;
    }

    fn toggle_theme(&mut self, ctx: &egui::Context, dark: bool) {
        self.dark = Some(!dark);
        ctx.set_theme(if dark {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
        // 图表配色随主题变化，需要重新渲染；旧纹理同时释放
        self.needs_prepare = true;
        self.release_images = true;
    }

    /// 文档统计（按需计算并缓存，避免每帧扫描全文）
    fn stats(&mut self) -> &str {
        if self.stats_dirty {
            let chars = self.source.chars().count();
            let lines = if self.source.is_empty() {
                0
            } else {
                self.source.lines().count()
            };
            let mut words = 0usize;
            let mut in_word = false;
            for ch in self.source.chars() {
                if ch.is_alphanumeric() {
                    if !in_word {
                        words += 1;
                    }
                    in_word = true;
                } else {
                    in_word = false;
                }
            }
            self.stats_text.clear();
            let _ = write!(self.stats_text, "{words} 词 · {chars} 字符 · {lines} 行");
            self.stats_dirty = false;
        }
        &self.stats_text
    }

    fn mode_hint(&self) -> &'static str {
        match self.mode {
            ViewMode::Split => "分栏",
            ViewMode::Editor => "仅编辑",
            ViewMode::Preview => "仅预览",
        }
    }
}

impl eframe::App for RambleApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 主题初始化（跟随系统）与应用全局样式
        if self.dark.is_none() {
            self.dark = Some(ctx.theme() == egui::Theme::Dark);
            assets::configure(&ctx);
        }
        let dark = self.dark.unwrap_or(true);
        if self.styled != Some(dark) {
            theme::apply_style(&ctx, dark);
            self.styled = Some(dark);
        }
        let p = theme::palette(dark);

        // 切换文档 / 主题后释放图片纹理，避免显存与像素缓存长期叠加
        if self.release_images {
            ctx.forget_all_images();
            self.release_images = false;
        }

        // 预览文本（含 mermaid 渲染）按需生成
        if self.needs_prepare {
            self.prepare();
        }

        let title = self.title();
        if title != self.title_cache {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_cache = title;
        }

        // 关闭窗口时检查未保存更改（quitting 表示用户已确认放弃，不再拦截）
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested
            && self.dirty
            && !self.quitting
            && self.pending != Some(PendingAction::Exit)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(PendingAction::Exit);
        }

        // 拖拽文件打开
        let dropped = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
        if let Some(pp) = dropped {
            self.open_path(pp);
        }

        // ── 快捷键 ──────────────────────────────────────────────
        let (key_n, key_o, key_s, key_f, key_h, key_f3, key_esc, zoom_in, zoom_out, zoom_reset) =
            ctx.input(|i| {
                (
                    i.modifiers.command && i.key_pressed(egui::Key::N),
                    i.modifiers.command && i.key_pressed(egui::Key::O),
                    i.modifiers.command && i.key_pressed(egui::Key::S),
                    i.modifiers.command && i.key_pressed(egui::Key::F),
                    i.modifiers.command && i.key_pressed(egui::Key::H),
                    i.key_pressed(egui::Key::F3),
                    i.key_pressed(egui::Key::Escape),
                    i.modifiers.command
                        && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
                    i.modifiers.command && i.key_pressed(egui::Key::Minus),
                    i.modifiers.command && i.key_pressed(egui::Key::Num0),
                )
            });
        if key_n {
            self.request_new();
        }
        if key_o {
            self.open_dialog();
        }
        if key_s {
            self.save();
        }
        if key_f {
            self.find.open = true;
            self.find.replace_visible = false;
            self.find.focus_input = true;
        }
        if key_h {
            self.find.open = true;
            self.find.replace_visible = true;
            self.find.focus_input = true;
        }
        if key_f3 {
            let shift = ctx.input(|i| i.modifiers.shift);
            if shift {
                self.find.find_prev(&self.source);
            } else {
                self.find.find_next(&self.source);
            }
        }
        // Ctrl+= / Ctrl+- / Ctrl+0 缩放界面
        if zoom_in {
            zoom_by(&ctx, 1.1);
        }
        if zoom_out {
            zoom_by(&ctx, 1.0 / 1.1);
        }
        if zoom_reset {
            ctx.set_zoom_factor(1.0);
        }
        if key_esc && self.find.open {
            self.find.close();
        }

        // 最近文件菜单的延迟动作
        let mut open_recent: Option<PathBuf> = None;
        let mut dead_recent: Option<PathBuf> = None;
        let mut clear_recent = false;
        let mut request_zoom: Option<f32> = None;

        // ── 顶部工具栏 ──────────────────────────────────────────
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(p.toolbar_bg)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .stroke(egui::Stroke::new(1.0, p.border)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 品牌
                    let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, p.accent);
                    ui.label(
                        egui::RichText::new("Ramble")
                            .size(13.5)
                            .strong()
                            .color(p.text),
                    );
                    ui.label(egui::RichText::new("Markdown").size(11.5).color(p.weak_text));
                    ui.add_space(4.0);
                    ui.separator();

                    if ui.button("新建").clicked() {
                        self.request_new();
                    }
                    if ui.button("打开").clicked() {
                        self.open_dialog();
                    }
                    if ui.button("保存").clicked() {
                        self.save();
                    }
                    if ui.button("另存为").clicked() {
                        self.save_as();
                    }

                    // 最近打开
                    let recent_count = self.recent.items().len();
                    ui.menu_button(format!("最近 {recent_count}"), |ui| {
                        ui.set_min_width(360.0);
                        if recent_count == 0 {
                            ui.label(egui::RichText::new("暂无记录").weak());
                        }
                        for rp in self.recent.items() {
                            let name = rp
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| rp.display().to_string());
                            let label = if rp.exists() {
                                egui::RichText::new(name)
                            } else {
                                egui::RichText::new(format!("{name}（已不存在）")).weak()
                            };
                            if ui
                                .button(label)
                                .on_hover_text(rp.display().to_string())
                                .clicked()
                            {
                                if rp.exists() {
                                    open_recent = Some(rp.clone());
                                } else {
                                    dead_recent = Some(rp.clone());
                                }
                                ui.close();
                            }
                        }
                        if recent_count > 0 {
                            ui.separator();
                            if ui.button("清除列表").clicked() {
                                clear_recent = true;
                                ui.close();
                            }
                        }
                    });

                    ui.separator();
                    // 视图模式切换（分段控件）
                    if mode_button(ui, &p, "分栏", self.mode == ViewMode::Split).clicked() {
                        self.set_mode(ViewMode::Split);
                    }
                    if mode_button(ui, &p, "编辑", self.mode == ViewMode::Editor).clicked() {
                        self.set_mode(ViewMode::Editor);
                    }
                    if mode_button(ui, &p, "预览", self.mode == ViewMode::Preview).clicked() {
                        self.set_mode(ViewMode::Preview);
                    }
                    ui.separator();
                    // 查找栏开关
                    let find_open = self.find.open;
                    if mode_button(ui, &p, "查找", find_open).clicked() {
                        if find_open {
                            self.find.close();
                        } else {
                            self.find.open = true;
                            self.find.focus_input = true;
                            self.find.jump_first(&self.source);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 界面缩放比例（不是正文字号）：Ctrl+= 放大、Ctrl+- 缩小，点击重置
                        let pct = (ctx.zoom_factor() * 100.0).round() as i32;
                        if ui
                            .button(format!("{pct}%"))
                            .on_hover_text("界面缩放：Ctrl+= 放大 / Ctrl+- 缩小，点击重置为 100%")
                            .clicked()
                        {
                            request_zoom = Some(1.0);
                        }
                        if ui
                            .button(if dark { "浅色" } else { "深色" })
                            .on_hover_text("切换明暗主题")
                            .clicked()
                        {
                            self.toggle_theme(&ctx, dark);
                        }
                        ui.separator();
                        if let Some(err) = &self.error {
                            ui.label(egui::RichText::new(err).color(p.error).small())
                                .on_hover_text(err.clone());
                        }
                        if let Some(pp) = &self.path {
                            // 长文件名中间省略 + 截断，避免压住右侧的缩放/主题按钮
                            let name = elide_middle(&self.file_name(), 30);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(name).color(p.weak_text).small(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(pp.display().to_string());
                        }
                    });
                });
            });

        if let Some(z) = request_zoom {
            ctx.set_zoom_factor(z.clamp(0.5, 3.0));
        }
        if let Some(pth) = open_recent {
            self.open_path(pth);
        }
        if let Some(pth) = dead_recent {
            // 文件已被移动/删除：提示并从列表中清理
            self.error = Some(format!("文件不存在: {}", pth.display()));
            self.recent.remove(&pth);
        }
        if clear_recent {
            self.recent.clear();
        }

        // ── 底部状态栏（VSCode 蓝条） ────────────────────────────
        egui::Panel::bottom("statusbar")
            .frame(
                egui::Frame::new()
                    .fill(p.status_bg)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let white = p.accent_text;
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        3.5,
                        if self.dirty {
                            egui::Color32::from_rgb(0xff, 0xd7, 0x66)
                        } else {
                            p.success
                        },
                    );
                    ui.label(
                        egui::RichText::new(if self.dirty { "未保存" } else { "已保存" })
                            .small()
                            .color(white),
                    );
                    ui.separator();
                    let stats = self.stats().to_owned();
                    ui.label(egui::RichText::new(stats).small().color(white));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 右下角版本标识
                        ui.label(
                            egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                .small()
                                .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 190)),
                        )
                        .on_hover_text("Ramble Markdown 版本");
                        ui.separator();
                        let zoom = (ctx.zoom_factor() * 100.0).round() as i32;
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {zoom}% · Ctrl+F 查找",
                                self.mode_hint()
                            ))
                            .small()
                            .color(white),
                        );
                    });
                });
            });

        // ── 查找 / 替换栏 ───────────────────────────────────────
        if self.find.open {
            egui::Panel::bottom("findbar")
                .frame(
                    egui::Frame::new()
                        .fill(p.panel_bg)
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .stroke(egui::Stroke::new(1.0, p.border)),
                )
                .show(ui, |ui| {
                    let changed = self.find.ui(ui, &mut self.source, p.error);
                    if changed {
                        self.touch();
                        ctx.request_repaint();
                    }
                });
        }

        // ── 主内容区 ────────────────────────────────────────────
        let jump = self.find.pending_jump;
        let highlight = self.find.highlight;
        match self.mode {
            ViewMode::Split => {
                egui::Panel::left("editor_panel")
                    .resizable(true)
                    .default_size(560.0)
                    .size_range(180.0..=2400.0)
                    .frame(egui::Frame::new().fill(p.editor_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_ed.take();
                        let (y, max, user, changed, jumped, scroll_to) = pane::editor_ui(
                            ui,
                            &mut self.source,
                            forced,
                            &mut self.last_ed_offset,
                            jump,
                            highlight,
                        );
                        if jumped {
                            self.find.pending_jump = None;
                        }
                        if let Some(target) = scroll_to {
                            self.apply_ed = Some(target);
                            ctx.request_repaint();
                        }
                        if changed {
                            self.touch();
                            ctx.request_repaint();
                        }
                        self.ed_max = max;
                        if user {
                            let ratio = if max > 0.0 { y / max } else { 0.0 };
                            self.apply_pv = Some(ratio * self.pv_max);
                        }
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.preview_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_pv.take();
                        let (y, max, user) = pane::preview_ui(
                            ui,
                            &mut self.cache,
                            &self.prepared,
                            &self.uri_base,
                            forced,
                            &mut self.last_pv_offset,
                        );
                        self.pv_max = max;
                        if user {
                            let ratio = if max > 0.0 { y / max } else { 0.0 };
                            self.apply_ed = Some(ratio * self.ed_max);
                        }
                    });
            }
            ViewMode::Editor => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.editor_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_ed.take();
                        let (_y, _max, _user, changed, jumped, scroll_to) = pane::editor_ui(
                            ui,
                            &mut self.source,
                            forced,
                            &mut self.last_ed_offset,
                            jump,
                            highlight,
                        );
                        if jumped {
                            self.find.pending_jump = None;
                        }
                        if let Some(target) = scroll_to {
                            self.apply_ed = Some(target);
                            ctx.request_repaint();
                        }
                        if changed {
                            self.touch();
                            ctx.request_repaint();
                        }
                    });
            }
            ViewMode::Preview => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.preview_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_pv.take();
                        let (_y, _max, _user) = pane::preview_ui(
                            ui,
                            &mut self.cache,
                            &self.prepared,
                            &self.uri_base,
                            forced,
                            &mut self.last_pv_offset,
                        );
                    });
            }
        }

        // ── 未保存更改确认对话框 ────────────────────────────────
        if let Some(action) = self.pending.clone() {
            let mut done: Option<Option<bool>> = None;
            egui::Window::new("未保存的更改")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.set_min_width(340.0);
                    ui.add_space(4.0);
                    ui.label("当前文档有未保存的更改，要先保存吗？");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let save = ui.add(
                            egui::Button::new(
                                egui::RichText::new("保存并继续").color(p.accent_text),
                            )
                            .fill(p.status_bg),
                        );
                        if save.clicked() {
                            done = Some(Some(true));
                        }
                        if ui.button("不保存").clicked() {
                            done = Some(Some(false));
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.button("取消").clicked() {
                                    done = Some(None);
                                }
                            },
                        );
                    });
                });
            match done {
                Some(Some(true)) => {
                    let saved = self.save();
                    if saved {
                        self.pending = None;
                        if !self.dirty {
                            self.complete_pending(action, &ctx);
                        }
                    }
                }
                Some(Some(false)) => {
                    self.pending = None;
                    self.complete_pending(action, &ctx);
                }
                Some(None) => {
                    self.pending = None;
                }
                None => {}
            }
        }

        // 只在预览落后或需要把滚动随动写回对侧时再重绘，空闲时不再空转
        if self.needs_prepare || self.apply_ed.is_some() || self.apply_pv.is_some() {
            ctx.request_repaint();
        }
    }
}

/// 长名称中间省略，保留头尾，避免标题栏 / 工具栏被超长文件名撑爆。
fn elide_middle(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

fn zoom_by(ctx: &egui::Context, factor: f32) {
    let z = (ctx.zoom_factor() * factor).clamp(0.5, 3.0);
    ctx.set_zoom_factor(z);
}

/// 分段控件风格的模式按钮
fn mode_button(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    label: &str,
    selected: bool,
) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label).color(if selected { p.accent } else { p.text }),
        )
        .fill(if selected {
            p.accent_soft
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(if selected {
            egui::Stroke::new(1.0, p.accent)
        } else {
            egui::Stroke::NONE
        }),
    )
}

/// 从内嵌 PNG 解码出窗口图标（任务栏 / Alt+Tab 使用）。
fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(include_bytes!("../assets/icon256.png"))
        .ok()?
        .to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([720.0, 480.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Ramble Markdown",
        options,
        Box::new(|cc| {
            theme::load_cjk_fonts(&cc.egui_ctx);
            cc.egui_ctx
                .add_image_loader(Arc::new(svg_loader::SvgMimeLoader::new()));
            Ok(Box::new(RambleApp::new()))
        }),
    )
}
