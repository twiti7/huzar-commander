use egui::{
    text::LayoutJob, Color32, FontId, RichText, Stroke, TextFormat, TextStyle, Ui,
};

use crate::i18n::I18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructuredSubView {
    #[default]
    Code,
    Tree,
}

/// Kolorystyka podświetlania składni (Dark theme inspirowany VS Code / One Dark)
mod colors {
    use egui::Color32;

    pub const KEY: Color32 = Color32::from_rgb(120, 200, 255); // Jasny błękit/cyan
    pub const STRING: Color32 = Color32::from_rgb(150, 225, 140); // Pastelowa zieleń
    pub const NUMBER: Color32 = Color32::from_rgb(255, 185, 110); // Bursztyn / pomarańcz
    pub const BOOLEAN: Color32 = Color32::from_rgb(225, 140, 240); // Fiolet / magenta
    pub const NULL: Color32 = Color32::from_rgb(140, 150, 165); // Szary / wygaszony
    pub const PUNCTUATION: Color32 = Color32::from_rgb(180, 190, 205); // Jasnoszary
    pub const COMMENT: Color32 = Color32::from_rgb(110, 145, 110); // Ciemna zieleń/szary
    pub const LINE_NO: Color32 = Color32::from_rgb(90, 100, 120); // Numer wiersza
    pub const BADGE_BG: Color32 = Color32::from_rgb(32, 38, 52); // Tło etykiety typu
    pub const ERROR_BG: Color32 = Color32::from_rgb(55, 25, 25);
    pub const ERROR_FG: Color32 = Color32::from_rgb(255, 120, 120);
}

// =========================================================================
//  GŁÓWNY WIDOK JSON
// =========================================================================

pub fn render_json(ui: &mut Ui, content: &str, sub_view: &mut StructuredSubView, i18n: &I18n) {
    let parsed_res: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(content);

    // Belka narzędziowa widoku strukturalnego
    render_toolbar(
        ui,
        "JSON",
        sub_view,
        content,
        parsed_res.is_ok(),
        parsed_res.as_ref().err().map(|e| e.to_string()),
        i18n,
    );
    ui.add_space(4.0);

    match sub_view {
        StructuredSubView::Code => {
            // Sformatowany lub oryginalny tekst z podświetlaniem składni
            let formatted = if let Ok(ref val) = parsed_res {
                serde_json::to_string_pretty(val).unwrap_or_else(|_| content.to_string())
            } else {
                content.to_string()
            };
            render_highlighted_code(ui, &formatted, highlight_json_text, "json_code_scroll");
        }
        StructuredSubView::Tree => {
            if let Ok(ref val) = parsed_res {
                render_json_tree(ui, val);
            } else if let Err(ref err) = parsed_res {
                render_parse_error(ui, "JSON", &err.to_string());
            }
        }
    }
}

// =========================================================================
//  GŁÓWNY WIDOK YAML
// =========================================================================

pub fn render_yaml(ui: &mut Ui, content: &str, sub_view: &mut StructuredSubView, i18n: &I18n) {
    let parsed_res: Result<serde_yaml::Value, serde_yaml::Error> = serde_yaml::from_str(content);

    // Belka narzędziowa widoku strukturalnego
    render_toolbar(
        ui,
        "YAML",
        sub_view,
        content,
        parsed_res.is_ok(),
        parsed_res.as_ref().err().map(|e| e.to_string()),
        i18n,
    );
    ui.add_space(4.0);

    match sub_view {
        StructuredSubView::Code => {
            render_highlighted_code(ui, content, highlight_yaml_text, "yaml_code_scroll");
        }
        StructuredSubView::Tree => {
            if let Ok(ref val) = parsed_res {
                render_yaml_tree(ui, val);
            } else if let Err(ref err) = parsed_res {
                render_parse_error(ui, "YAML", &err.to_string());
            }
        }
    }
}

// =========================================================================
//  PASEK NARZĘDZIOWY WIDOKU
// =========================================================================

fn render_toolbar(
    ui: &mut Ui,
    format_name: &str,
    sub_view: &mut StructuredSubView,
    raw_content: &str,
    is_valid: bool,
    error_msg: Option<String>,
    i18n: &I18n,
) {
    ui.horizontal_wrapped(|ui| {
        let fmt_label = i18n.t_fmt("viewer.format_label", &[("format", format_name)]);
        ui.label(
            RichText::new(fmt_label)
                .strong()
                .color(Color32::from_rgb(180, 215, 255)),
        );

        ui.separator();

        ui.selectable_value(sub_view, StructuredSubView::Code, i18n.t("viewer.subview_code"));
        ui.selectable_value(sub_view, StructuredSubView::Tree, i18n.t("viewer.subview_tree"));

        ui.separator();

        if is_valid {
            ui.label(
                RichText::new(i18n.t("viewer.valid_syntax"))
                    .size(11.5)
                    .color(Color32::from_rgb(100, 220, 140)),
            );
        } else {
            ui.label(
                RichText::new(i18n.t("viewer.syntax_error"))
                    .size(11.5)
                    .color(Color32::from_rgb(255, 140, 100)),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(i18n.t("viewer.copy_btn")).clicked() {
                ui.ctx().copy_text(raw_content.to_string());
            }
        });
    });

    if let Some(err) = error_msg {
        ui.add_space(2.0);
        egui::Frame::NONE
            .fill(colors::ERROR_BG)
            .stroke(Stroke::new(1.0_f32, colors::ERROR_FG))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8_i8, 4_i8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("⚠ {}: {}", format_name, err))
                        .size(11.5)
                        .color(colors::ERROR_FG),
                );
            });
    }
}

fn render_parse_error(ui: &mut Ui, format_name: &str, err_msg: &str) {
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(format!("Nie można wygenerować drzewa dla pliku {}", format_name))
                .size(14.0)
                .strong()
                .color(colors::ERROR_FG),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(err_msg).size(12.0).color(Color32::LIGHT_GRAY));
        ui.add_space(8.0);
        ui.label(
            RichText::new("Przełącz na widok '📄 Kolorowy kod', aby przejrzeć i edytować treść.")
                .size(12.0)
                .italics(),
        );
    });
}

// =========================================================================
//  KOLOROWY PODGLĄD KODU Z NUMERACJĄ WIERSZY
// =========================================================================

fn render_highlighted_code(
    ui: &mut Ui,
    text: &str,
    highlighter: impl Fn(&str, &Ui) -> LayoutJob,
    scroll_id: &str,
) {
    let avail_w = ui.available_width().max(100.0);
    let job = highlighter(text, ui);

    egui::Frame::NONE
        .fill(Color32::from_rgb(18, 21, 28))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8_i8, 6_i8))
        .show(ui, |ui| {
            ui.set_max_width(avail_w);
            egui::ScrollArea::horizontal()
                .id_salt(scroll_id)
                .auto_shrink([false, true])
                .max_width(avail_w)
                .show(ui, |ui| {
                    ui.label(job);
                });
        });
}

/// Tokenizer i generator LayoutJob dla JSON
pub fn highlight_json_text(text: &str, ui: &Ui) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.break_anywhere = false;
    let font = TextStyle::Monospace.resolve(ui.style());

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();
    let num_width = if line_count > 0 {
        ((line_count as f64).log10().floor() as usize) + 1
    } else {
        1
    };

    for (idx, line) in lines.iter().enumerate() {
        // Numer wiersza
        let line_num_str = format!("{:>width$} │ ", idx + 1, width = num_width);
        job.append(
            &line_num_str,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::LINE_NO,
                ..Default::default()
            },
        );

        // Tokenizacja wiersza JSON
        tokenize_json_line(line, &font, &mut job);

        if idx + 1 < line_count {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: Color32::TRANSPARENT,
                    ..Default::default()
                },
            );
        }
    }

    job
}

fn tokenize_json_line(line: &str, font: &FontId, job: &mut LayoutJob) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(
                &s,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::PUNCTUATION,
                    ..Default::default()
                },
            );
        } else if ch == '"' {
            // Napis w cudzysłowach
            let start = i;
            i += 1; // pomiń otwierający cudzysłów
            let mut escaped = false;
            while i < chars.len() {
                if chars[i] == '\\' && !escaped {
                    escaped = true;
                    i += 1;
                } else if chars[i] == '"' && !escaped {
                    i += 1;
                    break;
                } else {
                    escaped = false;
                    i += 1;
                }
            }
            let s: String = chars[start..i].iter().collect();

            // Sprawdź czy po tym napisie występuje dwukropek ':' (oznacza klucz)
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = j < chars.len() && chars[j] == ':';

            let color = if is_key { colors::KEY } else { colors::STRING };
            job.append(
                &s,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color,
                    ..Default::default()
                },
            );
        } else if ch == '{' || ch == '}' || ch == '[' || ch == ']' || ch == ':' || ch == ',' {
            job.append(
                &ch.to_string(),
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::PUNCTUATION,
                    ..Default::default()
                },
            );
            i += 1;
        } else if ch == '-' || ch.is_ascii_digit() {
            // Liczba
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E'
                    || chars[i] == '+'
                    || chars[i] == '-')
            {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            job.append(
                &s,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::NUMBER,
                    ..Default::default()
                },
            );
        } else if chars[i..].starts_with(&['t', 'r', 'u', 'e']) {
            job.append(
                "true",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::BOOLEAN,
                    ..Default::default()
                },
            );
            i += 4;
        } else if chars[i..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            job.append(
                "false",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::BOOLEAN,
                    ..Default::default()
                },
            );
            i += 5;
        } else if chars[i..].starts_with(&['n', 'u', 'l', 'l']) {
            job.append(
                "null",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::NULL,
                    ..Default::default()
                },
            );
            i += 4;
        } else {
            job.append(
                &ch.to_string(),
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: colors::PUNCTUATION,
                    ..Default::default()
                },
            );
            i += 1;
        }
    }
}

/// Tokenizer i generator LayoutJob dla YAML
pub fn highlight_yaml_text(text: &str, ui: &Ui) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.break_anywhere = false;
    let font = TextStyle::Monospace.resolve(ui.style());

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();
    let num_width = if line_count > 0 {
        ((line_count as f64).log10().floor() as usize) + 1
    } else {
        1
    };

    for (idx, line) in lines.iter().enumerate() {
        let line_num_str = format!("{:>width$} │ ", idx + 1, width = num_width);
        job.append(
            &line_num_str,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::LINE_NO,
                ..Default::default()
            },
        );

        tokenize_yaml_line(line, &font, &mut job);

        if idx + 1 < line_count {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: Color32::TRANSPARENT,
                    ..Default::default()
                },
            );
        }
    }

    job
}

fn tokenize_yaml_line(line: &str, font: &FontId, job: &mut LayoutJob) {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();

    // Wcięcia początkowe
    if indent_len > 0 {
        job.append(
            &line[..indent_len],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::PUNCTUATION,
                ..Default::default()
            },
        );
    }

    // Komentarz na początku wiersza
    if trimmed.starts_with('#') {
        job.append(
            trimmed,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::COMMENT,
                ..Default::default()
            },
        );
        return;
    }

    // Myślnik elementu listy
    let mut rest = trimmed;
    if rest.starts_with("- ") || rest == "-" {
        job.append(
            "- ",
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::KEY,
                ..Default::default()
            },
        );
        rest = if rest.len() >= 2 { &rest[2..] } else { "" };
    }

    // Wykrywanie pary klucz: wartość
    if let Some(colon_pos) = find_yaml_colon(rest) {
        let key_part = &rest[..colon_pos];
        let after_colon = &rest[colon_pos + 1..];

        // Klucz
        job.append(
            key_part,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::KEY,
                ..Default::default()
            },
        );

        // Dwukropek
        job.append(
            ":",
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::PUNCTUATION,
                ..Default::default()
            },
        );

        // Wartość po dwukropku
        if !after_colon.is_empty() {
            tokenize_yaml_value(after_colon, font, job);
        }
    } else {
        // Linia bez dwukropka (np. sam napis lub element)
        tokenize_yaml_value(rest, font, job);
    }
}

pub fn find_yaml_colon(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for (idx, ch) in s.char_indices() {
        if (ch == '"' || ch == '\'') && (!in_quotes || quote_char == ch) {
            in_quotes = !in_quotes;
            quote_char = ch;
        } else if ch == ':' && !in_quotes {
            let next = s[idx + 1..].chars().next();
            if next.is_none() || next == Some(' ') || next == Some('\t') {
                return Some(idx);
            }
        }
    }
    None
}

fn tokenize_yaml_value(val_str: &str, font: &FontId, job: &mut LayoutJob) {
    // Sprawdź komentarz na końcu wiersza
    let (val_text, comment_text) = if let Some(hash_pos) = val_str.find(" #") {
        (&val_str[..hash_pos], Some(&val_str[hash_pos..]))
    } else {
        (val_str, None)
    };

    let trimmed_val = val_text.trim();

    let color = if trimmed_val.starts_with('"') || trimmed_val.starts_with('\'') {
        colors::STRING
    } else if trimmed_val == "true"
        || trimmed_val == "false"
        || trimmed_val == "yes"
        || trimmed_val == "no"
        || trimmed_val == "on"
        || trimmed_val == "off"
    {
        colors::BOOLEAN
    } else if trimmed_val == "null" || trimmed_val == "~" {
        colors::NULL
    } else if trimmed_val.parse::<f64>().is_ok() {
        colors::NUMBER
    } else if trimmed_val.is_empty() {
        colors::PUNCTUATION
    } else {
        colors::STRING
    };

    job.append(
        val_text,
        0.0,
        TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        },
    );

    if let Some(comment) = comment_text {
        job.append(
            comment,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors::COMMENT,
                ..Default::default()
            },
        );
    }
}

// =========================================================================
//  INTERAKTYWNE DRZEWO OBIEKTÓW JSON
// =========================================================================

fn render_json_tree(ui: &mut Ui, val: &serde_json::Value) {
    egui::Frame::NONE
        .fill(Color32::from_rgb(18, 21, 28))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10_i8, 8_i8))
        .show(ui, |ui| {
            render_json_node(ui, None, val, "root", 0);
        });
}

fn render_json_node(
    ui: &mut Ui,
    key: Option<&str>,
    val: &serde_json::Value,
    id_path: &str,
    depth: usize,
) {
    let indent = (depth as f32) * 14.0;

    match val {
        serde_json::Value::Object(map) => {
            let label = if let Some(k) = key {
                format!("{} {{ }} ({} pól)", k, map.len())
            } else {
                format!("{{ }} Obiekt główny ({} pól)", map.len())
            };

            ui.horizontal(|ui| {
                if indent > 0.0 {
                    ui.add_space(indent);
                }
                render_badge(ui, "{ }", Color32::from_rgb(80, 160, 255));
                ui.collapsing(label, |ui| {
                    for (child_k, child_v) in map {
                        let child_path = format!("{}/{}", id_path, child_k);
                        render_json_node(ui, Some(child_k), child_v, &child_path, depth + 1);
                    }
                });
            });
        }
        serde_json::Value::Array(arr) => {
            let label = if let Some(k) = key {
                format!("{} [ ] ({} el.)", k, arr.len())
            } else {
                format!("[ ] Tablica główna ({} el.)", arr.len())
            };

            ui.horizontal(|ui| {
                if indent > 0.0 {
                    ui.add_space(indent);
                }
                render_badge(ui, "[ ]", Color32::from_rgb(255, 200, 80));
                ui.collapsing(label, |ui| {
                    for (idx, item) in arr.iter().enumerate() {
                        let idx_str = format!("[{}]", idx);
                        let child_path = format!("{}/{}", id_path, idx);
                        render_json_node(ui, Some(&idx_str), item, &child_path, depth + 1);
                    }
                });
            });
        }
        serde_json::Value::String(s) => {
            render_leaf_node(ui, key, &format!("\"{}\"", s), colors::STRING, "str", indent);
        }
        serde_json::Value::Number(n) => {
            render_leaf_node(ui, key, &n.to_string(), colors::NUMBER, "num", indent);
        }
        serde_json::Value::Bool(b) => {
            render_leaf_node(ui, key, &b.to_string(), colors::BOOLEAN, "bool", indent);
        }
        serde_json::Value::Null => {
            render_leaf_node(ui, key, "null", colors::NULL, "null", indent);
        }
    }
}

// =========================================================================
//  INTERAKTYWNE DRZEWO OBIEKTÓW YAML
// =========================================================================

fn render_yaml_tree(ui: &mut Ui, val: &serde_yaml::Value) {
    egui::Frame::NONE
        .fill(Color32::from_rgb(18, 21, 28))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10_i8, 8_i8))
        .show(ui, |ui| {
            render_yaml_node(ui, None, val, "root", 0);
        });
}

fn render_yaml_node(
    ui: &mut Ui,
    key: Option<&str>,
    val: &serde_yaml::Value,
    id_path: &str,
    depth: usize,
) {
    let indent = (depth as f32) * 14.0;

    match val {
        serde_yaml::Value::Mapping(map) => {
            let label = if let Some(k) = key {
                format!("{} {{ }} ({} pól)", k, map.len())
            } else {
                format!("{{ }} Słownik YAML ({} pól)", map.len())
            };

            ui.horizontal(|ui| {
                if indent > 0.0 {
                    ui.add_space(indent);
                }
                render_badge(ui, "{ }", Color32::from_rgb(80, 160, 255));
                ui.collapsing(label, |ui| {
                    for (child_k, child_v) in map {
                        let k_str = match child_k {
                            serde_yaml::Value::String(s) => s.clone(),
                            _ => format!("{:?}", child_k),
                        };
                        let child_path = format!("{}/{}", id_path, k_str);
                        render_yaml_node(ui, Some(&k_str), child_v, &child_path, depth + 1);
                    }
                });
            });
        }
        serde_yaml::Value::Sequence(seq) => {
            let label = if let Some(k) = key {
                format!("{} [ ] ({} el.)", k, seq.len())
            } else {
                format!("[ ] Sekwencja YAML ({} el.)", seq.len())
            };

            ui.horizontal(|ui| {
                if indent > 0.0 {
                    ui.add_space(indent);
                }
                render_badge(ui, "[ ]", Color32::from_rgb(255, 200, 80));
                ui.collapsing(label, |ui| {
                    for (idx, item) in seq.iter().enumerate() {
                        let idx_str = format!("[{}]", idx);
                        let child_path = format!("{}/{}", id_path, idx);
                        render_yaml_node(ui, Some(&idx_str), item, &child_path, depth + 1);
                    }
                });
            });
        }
        serde_yaml::Value::String(s) => {
            render_leaf_node(ui, key, s, colors::STRING, "str", indent);
        }
        serde_yaml::Value::Number(n) => {
            render_leaf_node(ui, key, &n.to_string(), colors::NUMBER, "num", indent);
        }
        serde_yaml::Value::Bool(b) => {
            render_leaf_node(ui, key, &b.to_string(), colors::BOOLEAN, "bool", indent);
        }
        serde_yaml::Value::Null => {
            render_leaf_node(ui, key, "null", colors::NULL, "null", indent);
        }
        serde_yaml::Value::Tagged(t) => {
            let label = format!("!{} {:?}", t.tag, t.value);
            render_leaf_node(ui, key, &label, colors::PUNCTUATION, "tag", indent);
        }
    }
}

fn render_leaf_node(
    ui: &mut Ui,
    key: Option<&str>,
    value_text: &str,
    val_color: Color32,
    badge_type: &str,
    indent: f32,
) {
    ui.horizontal(|ui| {
        if indent > 0.0 {
            ui.add_space(indent);
        }
        render_badge(ui, badge_type, val_color);

        if let Some(k) = key {
            ui.label(
                RichText::new(k)
                    .monospace()
                    .color(colors::KEY)
                    .strong(),
            );
            ui.label(RichText::new(":").monospace().color(colors::PUNCTUATION));
        }

        ui.label(RichText::new(value_text).monospace().color(val_color));

        // Drobny przycisk kopiowania wartości
        if ui
            .small_button("📋")
            .on_hover_text("Skopiuj wartość do schowka")
            .clicked()
        {
            ui.ctx().copy_text(value_text.to_string());
        }
    });
}

fn render_badge(ui: &mut Ui, label: &str, color: Color32) {
    egui::Frame::NONE
        .fill(colors::BADGE_BG)
        .stroke(Stroke::new(1.0_f32, color))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(4_i8, 1_i8))
        .show(ui, |ui| {
            ui.label(RichText::new(label).monospace().size(10.5).color(color));
        });
}


