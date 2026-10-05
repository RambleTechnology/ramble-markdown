//! VSCode 风格的配色与主题应用

use eframe::egui;

/// 一套界面配色
pub struct Palette {
    pub panel_bg: egui::Color32,
    pub editor_bg: egui::Color32,
    pub code_bg: egui::Color32,
    pub text: egui::Color32,
    pub weak_text: egui::Color32,
    pub border: egui::Color32,
    pub hover: egui::Color32,
    pub active: egui::Color32,
    pub link: egui::Color32,
    pub accent: egui::Color32,
    pub sel_bg: egui::Color32,
    pub error: egui::Color32,
}

/// VSCode Dark+ / Light+ 配色
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            panel_bg: egui::Color32::from_rgb(0x18, 0x18, 0x18),
            editor_bg: egui::Color32::from_rgb(0x1e, 0x1e, 0x1e),
            code_bg: egui::Color32::from_rgb(0x1b, 0x1b, 0x1b),
            text: egui::Color32::from_rgb(0xcc, 0xcc, 0xcc),
            weak_text: egui::Color32::from_rgb(0x8b, 0x8b, 0x8b),
            border: egui::Color32::from_rgb(0x2b, 0x2b, 0x2b),
            hover: egui::Color32::from_rgb(0x2a, 0x2d, 0x2e),
            active: egui::Color32::from_rgb(0x04, 0x39, 0x5c),
            link: egui::Color32::from_rgb(0x37, 0x94, 0xff),
            accent: egui::Color32::from_rgb(0x00, 0x7a, 0xcc),
            sel_bg: egui::Color32::from_rgb(0x26, 0x4f, 0x78),
            error: egui::Color32::from_rgb(0xf4, 0x6a, 0x6a),
        }
    } else {
        Palette {
            panel_bg: egui::Color32::from_rgb(0xf3, 0xf3, 0xf3),
            editor_bg: egui::Color32::WHITE,
            code_bg: egui::Color32::from_rgb(0xf7, 0xf7, 0xf7),
            text: egui::Color32::from_rgb(0x3b, 0x3b, 0x3b),
            weak_text: egui::Color32::from_rgb(0x71, 0x71, 0x71),
            border: egui::Color32::from_rgb(0xe0, 0xe0, 0xe0),
            hover: egui::Color32::from_rgb(0xe8, 0xe8, 0xe8),
            active: egui::Color32::from_rgb(0xd6, 0xeb, 0xff),
            link: egui::Color32::from_rgb(0x00, 0x6a, 0xb1),
            accent: egui::Color32::from_rgb(0x00, 0x7a, 0xcc),
            sel_bg: egui::Color32::from_rgb(0xad, 0xd6, 0xff),
            error: egui::Color32::from_rgb(0xc7, 0x2e, 0x2e),
        }
    }
}

/// 应用 VSCode 风格的 visuals 与字号
pub fn apply_vscode_style(ctx: &egui::Context, dark: bool) {
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
    v.selection.stroke = egui::Stroke::NONE;
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow::NONE;

    for state in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        state.corner_radius = egui::CornerRadius::same(2);
    }
    // 扁平按钮：默认透明背景，悬停浅色高亮
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, p.text);
    v.widgets.active.bg_fill = p.active;
    v.widgets.active.fg_stroke = egui::Stroke::new(
        1.0,
        if dark {
            egui::Color32::WHITE
        } else {
            p.text
        },
    );

    ctx.set_visuals(v);

    // 字号：VSCode 风格紧凑排版
    ctx.all_styles_mut(|style| {
        let ts = &mut style.text_styles;
        ts.insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
        ts.insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
        ts.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        ts.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        ts.insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
        ts.insert(
            egui::TextStyle::Name("Heading2".into()),
            egui::FontId::proportional(17.5),
        );
        ts.insert(
            egui::TextStyle::Name("Heading3".into()),
            egui::FontId::proportional(15.5),
        );
        ts.insert(
            egui::TextStyle::Name("Heading4".into()),
            egui::FontId::proportional(14.0),
        );
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
    });
}

/// 加载系统中文字体，否则 CJK 字符全部显示为方块
pub fn load_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",                              // 微软雅黑
        "C:/Windows/Fonts/msyhl.ttc",                             // 微软雅黑 Light
        "C:/Windows/Fonts/simhei.ttf",                            // 黑体
        "C:/Windows/Fonts/simsun.ttc",                            // 宋体
        "C:/Windows/Fonts/Deng.ttf",                              // 等线
        "/System/Library/Fonts/PingFang.ttc",                     // macOS
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Linux
    ];
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
