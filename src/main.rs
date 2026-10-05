#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Ramble Markdown —— 轻量级 Markdown 编辑与实时预览桌面应用
//!
//! 模块划分：
//! - `theme`：VSCode 风格配色、字体加载
//! - `pane`：编辑区 / 预览区渲染（含滚动随动所需的返回值）
//! - `mermaid`：mermaid 图表渲染与缓存
//! - `find`：查找 / 替换
//! - `recent`：最近打开文件持久化

mod find;
mod mermaid;
mod pane;
mod recent;
mod theme;

use eframe::egui;
use egui_commonmark::CommonMarkCache;
use find::FindState;
use mermaid::MermaidCache;
use recent::RecentFiles;
use std::path::{Path, PathBuf};

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
    /// 交给预览区渲染的文本（mermaid 代码块已替换为渲染结果）
    prepared: String,
    /// prepared 是否需要重新生成
    needs_prepare: bool,
    /// 当前文件路径
    path: Option<PathBuf>,
    /// 是否有未保存的修改
    dirty: bool,
    /// markdown 渲染缓存（图片等）
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

    fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名.md".into());
        format!(
            "{}{} — Ramble Markdown",
            name,
            if self.dirty { " *" } else { "" }
        )
    }

    /// 重新生成预览文本（mermaid 图表在此渲染，未改动的图表走缓存）
    fn prepare(&mut self) {
        self.prepared = self.mermaid.expand(&self.source);
        self.needs_prepare = false;
    }

    fn load(&mut self, p: &Path) {
        match std::fs::read_to_string(p) {
            Ok(s) => {
                self.source = s;
                self.path = Some(p.to_path_buf());
                self.dirty = false;
                self.error = None;
                self.needs_prepare = true;
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
        let default_name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名.md".into());
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
        self.path = None;
        self.dirty = false;
        self.error = None;
        self.needs_prepare = true;
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

    /// 文档统计
    fn stats(&self) -> String {
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
        format!("{words} 词 · {chars} 字符 · {lines} 行")
    }
}

impl eframe::App for RambleApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 主题初始化（跟随系统）与应用 VSCode 风格
        if self.dark.is_none() {
            self.dark = Some(ctx.theme() == egui::Theme::Dark);
        }
        let dark = self.dark.unwrap_or(true);
        theme::apply_vscode_style(&ctx, dark);
        let p = theme::palette(dark);

        // 预览文本（含 mermaid 渲染）按需生成
        if self.needs_prepare {
            self.prepare();
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title()));

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
        let (key_n, key_o, key_s, key_f, key_h, key_f3) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::N),
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.modifiers.command && i.key_pressed(egui::Key::S),
                i.modifiers.command && i.key_pressed(egui::Key::F),
                i.modifiers.command && i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::F3),
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
        }
        if key_h {
            self.find.open = true;
            self.find.replace_visible = true;
        }
        if key_f3 {
            let shift = ctx.input(|i| i.modifiers.shift);
            if shift {
                self.find.find_prev(&self.source);
            } else {
                self.find.find_next(&self.source);
            }
        }

        // 最近文件菜单的延迟动作
        let mut open_recent: Option<PathBuf> = None;
        let mut dead_recent: Option<PathBuf> = None;
        let mut clear_recent = false;

        // ── 顶部工具栏 ──────────────────────────────────────────
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(p.panel_bg)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .stroke(egui::Stroke::new(1.0, p.border)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("RAMBLE MARKDOWN")
                            .size(11.0)
                            .strong()
                            .color(p.accent),
                    );
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
                    ui.menu_button(format!("最近 ({recent_count})"), |ui| {
                        ui.set_min_width(320.0);
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
                    // 视图模式切换
                    let mut mode_btn = |ui: &mut egui::Ui, label: &str, mode: ViewMode| {
                        let selected = self.mode == mode;
                        if ui
                            .add(egui::Button::new(label).selected(selected).fill(
                                if selected {
                                    p.active
                                } else {
                                    egui::Color32::TRANSPARENT
                                },
                            ))
                            .clicked()
                        {
                            self.set_mode(mode);
                        }
                    };
                    mode_btn(ui, "分栏", ViewMode::Split);
                    mode_btn(ui, "编辑", ViewMode::Editor);
                    mode_btn(ui, "预览", ViewMode::Preview);
                    ui.separator();
                    // 查找栏开关
                    let find_open = self.find.open;
                    if ui
                        .add(
                            egui::Button::new("查找").selected(find_open).fill(
                                if find_open {
                                    p.active
                                } else {
                                    egui::Color32::TRANSPARENT
                                },
                            ),
                        )
                        .clicked()
                    {
                        if find_open {
                            self.find.close();
                        } else {
                            self.find.open = true;
                            self.find.jump_first(&self.source);
                        }
                    }
                    ui.separator();
                    // 主题切换
                    if ui.button(if dark { "浅色模式" } else { "深色模式" }).clicked() {
                        self.dark = Some(!dark);
                        ctx.set_theme(if dark {
                            egui::Theme::Light
                        } else {
                            egui::Theme::Dark
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(err) = &self.error {
                            ui.label(egui::RichText::new(err).color(p.error).small());
                        }
                        if let Some(pp) = &self.path {
                            ui.label(
                                egui::RichText::new(pp.display().to_string())
                                    .color(p.weak_text)
                                    .small(),
                            );
                        }
                    });
                });
            });

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
                    .fill(p.accent)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let white = egui::Color32::WHITE;
                    ui.label(
                        egui::RichText::new(if self.dirty { "● 未保存" } else { "✓ 已保存" })
                            .small()
                            .color(white),
                    );
                    ui.separator();
                    ui.label(egui::RichText::new(self.stats()).small().color(white));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(match self.mode {
                                ViewMode::Split => "分栏 · Ctrl+F 查找 · Ctrl+H 替换",
                                ViewMode::Editor => "仅编辑 · Ctrl+F 查找 · Ctrl+H 替换",
                                ViewMode::Preview => "仅预览",
                            })
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
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .stroke(egui::Stroke::new(1.0, p.border)),
                )
                .show(ui, |ui| {
                    let changed = self.find.ui(ui, &mut self.source, p.error);
                    if changed {
                        self.dirty = true;
                        self.needs_prepare = true;
                    }
                });
        }

        // ── 主内容区 ────────────────────────────────────────────
        let jump = self.find.pending_jump;
        match self.mode {
            ViewMode::Split => {
                egui::Panel::left("editor_panel")
                    .resizable(true)
                    .default_size(560.0)
                    .size_range(180.0..=2400.0)
                    .frame(egui::Frame::new().fill(p.editor_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_ed.take();
                        let (y, max, user, changed, jumped) =
                            pane::editor_ui(ui, &mut self.source, forced, &mut self.last_ed_offset, jump);
                        if jumped {
                            self.find.pending_jump = None;
                        }
                        if changed {
                            self.dirty = true;
                            self.needs_prepare = true;
                        }
                        self.ed_max = max;
                        if user {
                            let ratio = if max > 0.0 { y / max } else { 0.0 };
                            self.apply_pv = Some(ratio * self.pv_max);
                        }
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.editor_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_pv.take();
                        let (y, max, user) = pane::preview_ui(
                            ui,
                            &mut self.cache,
                            &self.prepared,
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
                        let (_y, _max, _user, changed, jumped) =
                            pane::editor_ui(ui, &mut self.source, forced, &mut self.last_ed_offset, jump);
                        if jumped {
                            self.find.pending_jump = None;
                        }
                        if changed {
                            self.dirty = true;
                            self.needs_prepare = true;
                        }
                    });
            }
            ViewMode::Preview => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(p.editor_bg))
                    .show(ui, |ui| {
                        let forced = self.apply_pv.take();
                        let (_y, _max, _user) = pane::preview_ui(
                            ui,
                            &mut self.cache,
                            &self.prepared,
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
                    ui.label("当前文档有未保存的更改，要先保存吗？");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("保存并继续").clicked() {
                            done = Some(Some(true));
                        }
                        if ui.button("不保存").clicked() {
                            done = Some(Some(false));
                        }
                        if ui.button("取消").clicked() {
                            done = Some(None);
                        }
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

        // 持续重绘：预览需要跟随编辑
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Ramble Markdown",
        options,
        Box::new(|cc| {
            theme::load_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(RambleApp::new()))
        }),
    )
}
