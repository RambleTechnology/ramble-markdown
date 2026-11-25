//! 配色、字体与全局样式
//!
//! 视觉基调取自 VSCode Dark+ / Light+，但按钮、面板与对话框采用更柔和的圆角，
//! 状态栏保留 VSCode 的蓝色条作为品牌色。

use eframe::egui;

/// 各平台优先使用的中文字体族名（mermaid 栅格化同样需要）
pub const CJK_FAMILY: &str = if cfg!(target_os = "windows") {
    "Microsoft YaHei"
} else if cfg!(target_os = "macos") {
    "PingFang SC"
} else {
    "Noto Sans CJK SC"
};

/// 一套界面配色
pub struct Palette {
    pub panel_bg: egui::Color32,
    pub toolbar_bg: egui::Color32,
    pub editor_bg: egui::Color32,
    pub preview_bg: egui::Color32,
    pub code_bg: egui::Color32,
    pub text: egui::Color32,
    pub weak_text: egui::Color32,
    pub border: egui::Color32,
    pub hover: egui::Color32,
    pub active: egui::Color32,
    pub accent: egui::Color32,
    pub accent_soft: egui::Color32,
    pub accent_text: egui::Color32,
    pub status_bg: egui::Color32,
    pub link: egui::Color32,
    pub sel_bg: egui::Color32,
    pub error: egui::Color32,
    pub success: egui::Color32,
}

/// VSCode Dark+ / Light+ 配色
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            panel_bg: egui::Color32::from_rgb(0x18, 0x18, 0x18),
            toolbar_bg: egui::Color32::from_rgb(0x1f, 0x1f, 0x1f),
            editor_bg: egui::Color32::from_rgb(0x1e, 0x1e, 0x1e),
            preview_bg: egui::Color32::from_rgb(0x1b, 0x1b, 0x1b),
            code_bg: egui::Color32::from_rgb(0x16, 0x16, 0x16),
            text: egui::Color32::from_rgb(0xd4, 0xd4, 0xd4),
            weak_text: egui::Color32::from_rgb(0x8a, 0x8a, 0x8a),
            border: egui::Color32::from_rgb(0x2e, 0x2e, 0x2e),
            hover: egui::Color32::from_rgb(0x2a, 0x2d, 0x2e),
            active: egui::Color32::from_rgb(0x04, 0x39, 0x5e),
            accent: egui::Color32::from_rgb(0x37, 0x94, 0xff),
            accent_soft: egui::Color32::from_rgb(0x17, 0x3a, 0x5c),
            accent_text: egui::Color32::WHITE,
            status_bg: egui::Color32::from_rgb(0x00, 0x7a, 0xcc),
            link: egui::Color32::from_rgb(0x37, 0x94, 0xff),
            sel_bg: egui::Color32::from_rgb(0x26, 0x4f, 0x78),
            error: egui::Color32::from_rgb(0xf8, 0x51, 0x49),
            success: egui::Color32::from_rgb(0x3f, 0xb9, 0x50),
        }
    } else {
        Palette {
            panel_bg: egui::Color32::from_rgb(0xf5, 0xf5, 0xf5),
            toolbar_bg: egui::Color32::from_rgb(0xeb, 0xeb, 0xeb),
            editor_bg: egui::Color32::WHITE,
            preview_bg: egui::Color32::from_rgb(0xfb, 0xfb, 0xfb),
            code_bg: egui::Color32::from_rgb(0xf3, 0xf3, 0xf3),
            text: egui::Color32::from_rgb(0x2f, 0x2f, 0x2f),
            weak_text: egui::Color32::from_rgb(0x6e, 0x6e, 0x6e),
            border: egui::Color32::from_rgb(0xdc, 0xdc, 0xdc),
            hover: egui::Color32::from_rgb(0xe7, 0xe7, 0xe7),
            active: egui::Color32::from_rgb(0xcf, 0xe6, 0xff),
            accent: egui::Color32::from_rgb(0x09, 0x69, 0xda),
            accent_soft: egui::Color32::from_rgb(0xdd, 0xeb, 0xfb),
            accent_text: egui::Color32::WHITE,
            status_bg: egui::Color32::from_rgb(0x00, 0x7a, 0xcc),
            link: egui::Color32::from_rgb(0x09, 0x69, 0xda),
            sel_bg: egui::Color32::from_rgb(0xb6, 0xd7, 0xff),
            error: egui::Color32::from_rgb(0xcf, 0x22, 0x2e),
            success: egui::Color32::from_rgb(0x1a, 0x7f, 0x37),
        }
    }
}

/// 应用全局 style（每帧调用，代价很小）
pub fn apply_style(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    v.panel_fill = p.panel_bg;
    v.window_fill = p.panel_bg;
    v.extreme_bg_color = p.editor_bg;
    v.faint_bg_color = p.code_bg;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.link;
    v.selection.bg_fill = p.sel_bg;
    // selection.stroke 是「选中文字的颜色」，不是边框；设为 NONE 会让选中文字完全消失
    v.selection.stroke = egui::Stroke::new(1.0, p.text);
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.window_corner_radius = egui::CornerRadius::same(10);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    v.window_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: egui::Color32::from_black_alpha(if dark { 120 } else { 48 }),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: egui::Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };

    for state in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        state.corner_radius = egui::CornerRadius::same(6);
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.active.bg_fill = p.active;
    // 注意：egui 的 strong_text_color() 取自 widgets.active.fg_stroke，
    // 浅色主题下若用纯白会让加粗文字和标题「消失」在白底上，因此这里跟随文字色。
    v.widgets.active.fg_stroke = egui::Stroke::new(
        1.0,
        if dark {
            egui::Color32::WHITE
        } else {
            p.text
        },
    );

    ctx.set_visuals(v);

    ctx.all_styles_mut(|style| {
        let ts = &mut style.text_styles;
        ts.insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
        ts.insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
        ts.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        ts.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.5));
        ts.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        ts.insert(
            egui::TextStyle::Name("Heading2".into()),
            egui::FontId::proportional(18.0),
        );
        ts.insert(
            egui::TextStyle::Name("Heading3".into()),
            egui::FontId::proportional(16.0),
        );
        ts.insert(
            egui::TextStyle::Name("Heading4".into()),
            egui::FontId::proportional(14.0),
        );
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
    });
}

/// 加载系统中文字体，否则 CJK 字符全部显示为方块
pub fn load_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:/Windows/Fonts/msyhl.ttc",                             // 微软雅黑 Light（体积更小）
        "C:/Windows/Fonts/Deng.ttf",                              // 等线
        "C:/Windows/Fonts/msyh.ttc",                              // 微软雅黑
        "C:/Windows/Fonts/simhei.ttf",                            // 黑体
        "C:/Windows/Fonts/simsun.ttc",                            // 宋体
        "/System/Library/Fonts/PingFang.ttc",                     // macOS
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Linux
    ];
    // 只加载第一个可用的字体：多份 CJK 字体会白白占用十几 MB 常驻内存
    for (i, path) in candidates.iter().enumerate() {
        if let Ok(data) = std::fs::read(path) {
            let name = format!("cjk_{i}");
            fonts.font_data.insert(
                name.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            // 追加到默认字体之后作为回退：拉丁字符仍用默认字体，CJK 用系统字体
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name);
            break;
        }
    }
    ctx.set_fonts(fonts);
}
