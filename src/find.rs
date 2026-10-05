//! 查找 / 替换
//!
//! 匹配基于字符而非字节，支持中英文混合文本；跳转时把编辑器光标设置到匹配区间，
//! 由 egui 自行滚动到光标位置。

use eframe::egui;

/// 待跳转的匹配区间（字符索引）
pub type CharRange = (usize, usize);

#[derive(Default)]
pub struct FindState {
    /// 查找栏是否可见
    pub open: bool,
    /// 是否展开替换输入框
    pub replace_visible: bool,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    /// 当前选中的匹配序号（None = 尚未选中任何匹配）
    pub selected: Option<usize>,
    /// 尚未应用的跳转请求（等编辑器状态就绪后应用）
    pub pending_jump: Option<CharRange>,
}

/// 一次匹配（同时给出字节与字符索引）
struct Match {
    byte: (usize, usize),
    char: (usize, usize),
}

impl FindState {
    pub fn new() -> Self {
        Self::default()
    }

    fn matches(&self, text: &str) -> Vec<Match> {
        let q = self.query.as_str();
        let mut out = Vec::new();
        if q.is_empty() {
            return out;
        }

        // 字符序号 → 字节偏移
        let mut byte_of_char: Vec<usize> = Vec::new();
        for (bi, _) in text.char_indices() {
            byte_of_char.push(bi);
        }
        byte_of_char.push(text.len());

        // 大小写不敏感时，把原文展开成小写字符序列，并记录每个小写字符来自哪个原字符，
        // 这样匹配结果可以准确映射回原文字节区间（不会切坏多字节字符）
        let (hay, idx_map, needle): (Vec<char>, Vec<usize>, Vec<char>) = if self.case_sensitive {
            let hay: Vec<char> = text.chars().collect();
            let map: Vec<usize> = (0..hay.len()).collect();
            (hay, map, q.chars().collect())
        } else {
            let mut hay = Vec::new();
            let mut map = Vec::new();
            for (ci, ch) in text.chars().enumerate() {
                for lc in ch.to_lowercase() {
                    hay.push(lc);
                    map.push(ci);
                }
            }
            let needle: Vec<char> = q.chars().flat_map(|c| c.to_lowercase()).collect();
            (hay, map, needle)
        };

        if needle.is_empty() || hay.len() < needle.len() {
            return out;
        }

        for start in 0..=(hay.len() - needle.len()) {
            if hay[start..start + needle.len()] == needle[..] {
                let c0 = idx_map[start];
                let c1 = idx_map[start + needle.len() - 1] + 1;
                out.push(Match {
                    byte: (byte_of_char[c0], byte_of_char[c1]),
                    char: (c0, c1),
                });
            }
        }
        out
    }

    pub fn match_count(&self, text: &str) -> usize {
        self.matches(text).len()
    }

    /// 匹配进度文案，如 "3 / 12"；无匹配时为 "0 / 0"
    pub fn label(&self, text: &str) -> String {
        let n = self.match_count(text);
        if n == 0 {
            return "0 / 0".to_string();
        }
        let cur = self.selected.map(|i| i + 1).unwrap_or(1).min(n);
        format!("{cur} / {n}")
    }

    /// 跳到下一个匹配
    pub fn find_next(&mut self, text: &str) {
        let ms = self.matches(text);
        if ms.is_empty() {
            self.selected = None;
            return;
        }
        // 尚未选中时从第一个匹配开始（避免首次「下一个」跳过第一个）
        let i = match self.selected {
            Some(i) => (i + 1) % ms.len(),
            None => 0,
        };
        self.selected = Some(i);
        self.pending_jump = Some(ms[i].char);
    }

    /// 跳到上一个匹配
    pub fn find_prev(&mut self, text: &str) {
        let ms = self.matches(text);
        if ms.is_empty() {
            self.selected = None;
            return;
        }
        let i = match self.selected {
            Some(i) => (i + ms.len() - 1) % ms.len(),
            None => 0,
        };
        self.selected = Some(i);
        self.pending_jump = Some(ms[i].char);
    }

    /// 第一次打开查找栏（或查询词变化）时定位到第一个匹配
    pub fn jump_first(&mut self, text: &str) {
        let ms = self.matches(text);
        self.selected = if ms.is_empty() { None } else { Some(0) };
        if let Some(m) = ms.first() {
            self.pending_jump = Some(m.char);
        }
    }

    /// 替换当前匹配（并跳到下一个）
    pub fn replace_current(&mut self, text: &mut String) {
        let ms = self.matches(text);
        if ms.is_empty() {
            return;
        }
        let i = self.selected.unwrap_or(0).min(ms.len() - 1);
        let (b0, b1) = ms[i].byte;
        text.replace_range(b0..b1, &self.replacement);
        let after = text.clone();
        self.find_next(&after);
    }

    /// 全部替换，返回替换次数
    pub fn replace_all(&mut self, text: &mut String) -> usize {
        let ms = self.matches(text);
        if ms.is_empty() {
            return 0;
        }
        // 从后往前替换，避免字节偏移失效
        for m in ms.iter().rev() {
            text.replace_range(m.byte.0..m.byte.1, &self.replacement);
        }
        self.selected = None;
        self.pending_jump = None;
        ms.len()
    }

    /// 查找/替换栏 UI，返回文本是否被修改
    pub fn ui(&mut self, ui: &mut egui::Ui, text: &mut String, error: egui::Color32) -> bool {
        let mut changed = false;
        let mut do_next = false;
        let mut do_prev = false;
        let mut do_replace = false;
        let mut do_replace_all = false;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("查找").small());

            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(200.0)
                    .hint_text("输入要查找的内容"),
            );
            if resp.changed() {
                self.jump_first(text);
            }
            // 回车 = 下一个，Shift+回车 = 上一个；Esc 关闭
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if ui.input(|i| i.modifiers.shift) {
                    do_prev = true;
                } else {
                    do_next = true;
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.close();
            }

            ui.label(
                egui::RichText::new(self.label(text))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );

            if ui.button("上一个").clicked() {
                do_prev = true;
            }
            if ui.button("下一个").clicked() {
                do_next = true;
            }

            if self.replace_visible {
                ui.separator();
                ui.label(egui::RichText::new("替换为").small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.replacement)
                        .desired_width(160.0)
                        .hint_text("替换内容"),
                );
                if ui.button("替换").clicked() {
                    do_replace = true;
                }
                if ui.button("全部替换").clicked() {
                    do_replace_all = true;
                }
            } else if ui.button("显示替换").clicked() {
                self.replace_visible = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").clicked() {
                    self.close();
                }
                let mut cs = self.case_sensitive;
                if ui.checkbox(&mut cs, "区分大小写").changed() {
                    self.case_sensitive = cs;
                    self.jump_first(text);
                }
            });
        });

        if do_next {
            self.find_next(text);
        }
        if do_prev {
            self.find_prev(text);
        }
        if do_replace {
            self.replace_current(text);
            changed = true;
        }
        if do_replace_all {
            let n = self.replace_all(text);
            if n > 0 {
                changed = true;
            }
        }

        // 无匹配时提示
        if !self.query.is_empty() && self.match_count(text) == 0 {
            ui.label(
                egui::RichText::new("没有找到匹配项")
                    .small()
                    .color(error),
            );
        }

        changed
    }

    pub fn close(&mut self) {
        self.open = false;
        self.replace_visible = false;
        self.pending_jump = None;
        self.selected = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(query: &str, case_sensitive: bool) -> FindState {
        FindState {
            query: query.to_string(),
            case_sensitive,
            ..Default::default()
        }
    }

    #[test]
    fn case_insensitive_ascii() {
        let f = st("apple", false);
        assert_eq!(f.match_count("Apple apple APPLE"), 3);
    }

    #[test]
    fn case_sensitive_ascii() {
        let f = st("apple", true);
        assert_eq!(f.match_count("Apple apple APPLE"), 1);
    }

    #[test]
    fn chinese_matches_have_correct_byte_ranges() {
        let text = "重庆的洪崖洞，重庆的解放碑";
        let f = st("重庆", false);
        let ms = f.matches(text);
        assert_eq!(ms.len(), 2);
        for m in &ms {
            assert_eq!(&text[m.byte.0..m.byte.1], "重庆");
        }
        // 字符区间也应对应（每个中文字符 1 个字符）
        assert_eq!(ms[0].char, (0, 2));
    }

    #[test]
    fn replace_all_works() {
        let mut text = "a-b-a-c-a".to_string();
        let mut f = st("a", false);
        f.replacement = "X".into();
        assert_eq!(f.replace_all(&mut text), 3);
        assert_eq!(text, "X-b-X-c-X");
    }

    #[test]
    fn replace_all_chinese() {
        let mut text = "重庆很好，重庆很美".to_string();
        let mut f = st("重庆", false);
        f.replacement = "成都".into();
        assert_eq!(f.replace_all(&mut text), 2);
        assert_eq!(text, "成都很好，成都很美");
    }

    #[test]
    fn replace_current_then_advance() {
        let mut text = "one two one".to_string();
        let mut f = st("one", false);
        f.replacement = "1".into();
        f.replace_current(&mut text);
        assert_eq!(text, "1 two one");
        f.replace_current(&mut text);
        assert_eq!(text, "1 two 1");
    }

    #[test]
    fn navigation_cycles() {
        let text = "x y x y x";
        let mut f = st("x", false);
        // 首次「下一个」应落在第一个匹配上，而不是跳过它
        f.find_next(text);
        assert_eq!(f.selected, Some(0));
        f.find_next(text);
        assert_eq!(f.selected, Some(1));
        f.find_prev(text);
        assert_eq!(f.selected, Some(0));
        f.find_prev(text);
        assert_eq!(f.selected, Some(2)); // 循环到最后一个
    }

    #[test]
    fn empty_query_matches_nothing() {
        let f = st("", false);
        assert_eq!(f.match_count("anything"), 0);
    }

    #[test]
    fn label_shows_progress() {
        let mut f = st("x", false);
        // 未选中时默认显示第一个匹配
        assert_eq!(f.label("x x x"), "1 / 3");
        // 首次「下一个」落在第一个匹配上
        f.find_next("x x x");
        assert_eq!(f.label("x x x"), "1 / 3");
        // 再按一次才前进到第二个
        f.find_next("x x x");
        assert_eq!(f.label("x x x"), "2 / 3");
    }
}
