use egui::{
    text::LayoutJob, Color32, FontId, RichText, Stroke, TextFormat, TextStyle, Ui,
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Default, Clone)]
pub struct InlineSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub is_code: bool,
    pub link_url: Option<String>,
}

fn build_layout_job(spans: &[InlineSpan], max_width: f32, base_size: f32, base_color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = max_width.max(60.0);
    job.wrap.break_anywhere = true;

    for span in spans {
        let (font_id, color, bg) = if span.is_code {
            (
                FontId::monospace(base_size * 0.92),
                Color32::from_rgb(245, 215, 140),
                Color32::from_rgb(45, 52, 68),
            )
        } else if span.link_url.is_some() {
            (
                FontId::proportional(base_size),
                Color32::from_rgb(100, 185, 255),
                Color32::TRANSPARENT,
            )
        } else if span.bold {
            (
                FontId::proportional(base_size),
                Color32::from_rgb(255, 255, 255),
                Color32::TRANSPARENT,
            )
        } else {
            (
                FontId::proportional(base_size),
                base_color,
                Color32::TRANSPARENT,
            )
        };

        let underline = if span.link_url.is_some() {
            Stroke::new(1.0_f32, Color32::from_rgb(100, 185, 255))
        } else {
            Stroke::NONE
        };

        let strikethrough = if span.strikethrough {
            Stroke::new(1.0_f32, base_color)
        } else {
            Stroke::NONE
        };

        job.append(
            &span.text,
            0.0,
            TextFormat {
                font_id,
                color,
                background: bg,
                italics: span.italic,
                underline,
                strikethrough,
                ..Default::default()
            },
        );
    }

    job
}

pub fn render_markdown(ui: &mut Ui, text: &str) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(text, options);
    let events: Vec<Event> = parser.collect();

    let mut i = 0;
    let mut list_depth = 0usize;
    let mut list_counters: Vec<usize> = Vec::new();
    let mut scroll_id_counter = 0usize;

    let text_color = Color32::from_rgb(215, 220, 230);

    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = *level;
                i += 1;
                let mut heading_text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::End(TagEnd::Heading(_)) => {
                            i += 1;
                            break;
                        }
                        Event::Text(t) | Event::Code(t) => {
                            heading_text.push_str(t);
                            i += 1;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }

                match lvl {
                    HeadingLevel::H1 => {
                        ui.add_space(14.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&heading_text)
                                    .size(23.0)
                                    .strong()
                                    .color(Color32::from_rgb(250, 250, 255)),
                            )
                            .wrap(),
                        );
                        ui.add_space(2.0);
                        ui.separator();
                        ui.add_space(6.0);
                    }
                    HeadingLevel::H2 => {
                        ui.add_space(12.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&heading_text)
                                    .size(18.5)
                                    .strong()
                                    .color(Color32::from_rgb(225, 235, 255)),
                            )
                            .wrap(),
                        );
                        ui.add_space(2.0);
                        ui.separator();
                        ui.add_space(5.0);
                    }
                    HeadingLevel::H3 => {
                        ui.add_space(10.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&heading_text)
                                    .size(16.0)
                                    .strong()
                                    .color(Color32::from_rgb(180, 210, 255)),
                            )
                            .wrap(),
                        );
                        ui.add_space(4.0);
                    }
                    _ => {
                        ui.add_space(8.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&heading_text)
                                    .size(14.0)
                                    .strong()
                                    .color(Color32::from_rgb(160, 195, 240)),
                            )
                            .wrap(),
                        );
                        ui.add_space(3.0);
                    }
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let lang_label = match kind {
                    CodeBlockKind::Fenced(l) => {
                        let trimmed = l.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };

                i += 1;
                let mut code_text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::End(TagEnd::CodeBlock) => {
                            i += 1;
                            break;
                        }
                        Event::Text(t) => {
                            code_text.push_str(t);
                            i += 1;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }

                scroll_id_counter += 1;
                let block_id = scroll_id_counter;

                // Jeśli blok to diagram Mermaid — dedykowany renderer graficzny
                if lang_label
                    .as_deref()
                    .map(|l| l.eq_ignore_ascii_case("mermaid"))
                    .unwrap_or(false)
                {
                    render_mermaid_block(ui, &code_text, block_id);
                } else {
                    // Bezwarunkowy odstęp przed blokiem kodu — kod ZAWSZE w nowej linii
                    let avail_w = ui.available_width();
                    let code_box_w = (avail_w - 24.0).max(60.0);
                    ui.add_space(6.0);
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(20, 23, 30))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 62, 78)))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(10_i8, 8_i8))
                        .show(ui, |ui| {
                            ui.set_max_width(code_box_w);
                            // Belka górna bloku kodu: etykieta języka + przycisk Kopiuj
                            ui.horizontal(|ui| {
                                let label = lang_label.as_deref().unwrap_or("kod");
                                ui.label(
                                    RichText::new(format!("💻 {}", label))
                                        .size(11.5)
                                        .monospace()
                                        .color(Color32::from_rgb(130, 160, 205)),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("📋 Kopiuj").clicked() {
                                        ui.ctx().copy_text(code_text.clone());
                                    }
                                });
                            });
                            ui.add_space(4.0);

                            // Zawartość kodu (monospaced, przewijanie poziome z unikalnym ID)
                            egui::ScrollArea::horizontal()
                                .id_salt(("md_code_scroll", block_id))
                                .auto_shrink([false, true])
                                .max_width(code_box_w)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut code_text.as_str())
                                            .font(TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .frame(false)
                                            .text_color(Color32::from_rgb(225, 230, 240)),
                                    );
                                });
                        });
                    ui.add_space(6.0);
                }
            }

            Event::Start(Tag::Paragraph) => {
                i += 1;
                let spans = collect_inlines(&events, &mut i, |end| matches!(end, TagEnd::Paragraph));
                let avail_w = ui.available_width();
                let job = build_layout_job(&spans, avail_w, 13.5, text_color);
                ui.label(job);
                ui.add_space(4.0);
            }

            Event::Start(Tag::BlockQuote(_)) => {
                i += 1;
                let spans = collect_inlines(&events, &mut i, |end| matches!(end, TagEnd::BlockQuote(_)));
                ui.add_space(4.0);
                let avail_w = ui.available_width();
                egui::Frame::NONE
                    .fill(Color32::from_rgba_premultiplied(70, 130, 230, 15))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 65, 95)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin {
                        left: 12_i8,
                        right: 8_i8,
                        top: 6_i8,
                        bottom: 6_i8,
                    })
                    .show(ui, |ui| {
                        let inner_w = (avail_w - 24.0).max(60.0);
                        ui.set_max_width(inner_w);
                        let job = build_layout_job(&spans, inner_w, 13.0, Color32::from_rgb(180, 205, 235));
                        ui.label(job);
                    });
                ui.add_space(4.0);
            }

            Event::Start(Tag::List(first_num)) => {
                list_depth += 1;
                list_counters.push(first_num.unwrap_or(0) as usize);
                i += 1;
            }

            Event::End(TagEnd::List(_)) => {
                if list_depth > 0 {
                    list_depth -= 1;
                    list_counters.pop();
                }
                ui.add_space(4.0);
                i += 1;
            }

            Event::Start(Tag::Item) => {
                i += 1;
                let indent = (list_depth.saturating_sub(1) as f32) * 16.0;

                // Sprawdź czy to tasklist (np. [ ] lub [x])
                let mut task_checked = None;
                if i < events.len() {
                    if let Event::TaskListMarker(checked) = events[i] {
                        task_checked = Some(checked);
                        i += 1;
                    }
                }

                let spans = collect_inlines(&events, &mut i, |end| matches!(end, TagEnd::Item));
                let avail_w = ui.available_width();
                let rem_w = (avail_w - indent - 30.0).max(60.0);
                let job = build_layout_job(&spans, rem_w, 13.5, text_color);

                ui.horizontal(|ui| {
                    if indent > 0.0 {
                        ui.add_space(indent);
                    }

                    if let Some(checked) = task_checked {
                        let (icon, col) = if checked {
                            ("☑", Color32::from_rgb(100, 220, 140))
                        } else {
                            ("☐", Color32::from_rgb(150, 155, 170))
                        };
                        ui.label(RichText::new(icon).size(14.0).color(col));
                    } else if let Some(last_cnt) = list_counters.last_mut() {
                        if *last_cnt > 0 {
                            ui.label(
                                RichText::new(format!("{}.", last_cnt))
                                    .size(13.0)
                                    .monospace()
                                    .color(Color32::from_rgb(140, 180, 255)),
                            );
                            *last_cnt += 1;
                        } else {
                            ui.label(
                                RichText::new("•")
                                    .size(15.0)
                                    .color(Color32::from_rgb(100, 160, 255)),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new("•")
                                .size(15.0)
                                .color(Color32::from_rgb(100, 160, 255)),
                        );
                    }

                    ui.label(job);
                });
                ui.add_space(2.0);
            }

            Event::Rule => {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                i += 1;
            }

            Event::Start(Tag::Table(_)) => {
                i += 1;
                scroll_id_counter += 1;
                render_table(ui, &events, &mut i, scroll_id_counter);
            }

            _ => {
                i += 1;
            }
        }
    }
}

fn render_mermaid_block(ui: &mut Ui, code_text: &str, block_id: usize) {
    ui.push_id(("mermaid_block", block_id), |ui| {
        ui.add_space(8.0);
        let avail_w = ui.available_width();
        let inner_w = (avail_w - 28.0).max(60.0);
        egui::Frame::NONE
            .fill(Color32::from_rgb(18, 22, 32))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 80, 130)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12_i8, 10_i8))
            .show(ui, |ui| {
                ui.set_max_width(inner_w);
                // Pasek nagłówkowy diagramu Mermaid
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("📊 Diagram Mermaid")
                            .size(12.5)
                            .strong()
                            .color(Color32::from_rgb(130, 185, 255)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("📋 Kopiuj kod").clicked() {
                            ui.ctx().copy_text(code_text.to_string());
                        }
                    });
                });
                ui.add_space(6.0);

                // Wyrenderuj diagram za pomocą silnika mermaid_painter
                let diagram_res = crate::ui::mermaid_painter::render_diagram(ui, code_text);
                if let Err(err) = diagram_res {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("⚠️ Podgląd diagramu: {}", err))
                            .size(11.5)
                            .color(Color32::from_rgb(240, 175, 75)),
                    );
                }

                ui.add_space(6.0);
                ui.collapsing("🔍 Pokaż kod źródłowy", |ui| {
                    ui.add_space(4.0);
                    let code_box_w = (inner_w - 8.0).max(60.0);
                    let mut text_ref = code_text;
                    egui::ScrollArea::horizontal()
                        .id_salt(("mermaid_raw_scroll", block_id))
                        .auto_shrink([false, true])
                        .max_width(code_box_w)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut text_ref)
                                    .font(TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .frame(false)
                                    .text_color(Color32::from_rgb(200, 215, 235)),
                            );
                        });
                });
            });
        ui.add_space(8.0);
    });
}

pub fn collect_inlines(
    events: &[Event],
    i: &mut usize,
    is_end: impl Fn(&TagEnd) -> bool,
) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut bold = false;
    let mut italic = false;
    let mut strikethrough = false;
    let mut link_url = None;

    while *i < events.len() {
        match &events[*i] {
            Event::End(end) if is_end(end) => {
                *i += 1;
                break;
            }
            Event::Start(Tag::Strong) => {
                bold = true;
                *i += 1;
            }
            Event::End(TagEnd::Strong) => {
                bold = false;
                *i += 1;
            }
            Event::Start(Tag::Emphasis) => {
                italic = true;
                *i += 1;
            }
            Event::End(TagEnd::Emphasis) => {
                italic = false;
                *i += 1;
            }
            Event::Start(Tag::Strikethrough) => {
                strikethrough = true;
                *i += 1;
            }
            Event::End(TagEnd::Strikethrough) => {
                strikethrough = false;
                *i += 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_url = Some(dest_url.to_string());
                *i += 1;
            }
            Event::End(TagEnd::Link) => {
                link_url = None;
                *i += 1;
            }
            Event::Text(t) => {
                spans.push(InlineSpan {
                    text: t.to_string(),
                    bold,
                    italic,
                    strikethrough,
                    is_code: false,
                    link_url: link_url.clone(),
                });
                *i += 1;
            }
            Event::Code(c) => {
                spans.push(InlineSpan {
                    text: format!(" {} ", c),
                    bold,
                    italic,
                    strikethrough,
                    is_code: true,
                    link_url: None,
                });
                *i += 1;
            }
            Event::SoftBreak => {
                spans.push(InlineSpan {
                    text: " ".to_string(),
                    bold,
                    italic,
                    strikethrough,
                    is_code: false,
                    link_url: None,
                });
                *i += 1;
            }
            Event::HardBreak => {
                spans.push(InlineSpan {
                    text: "\n".to_string(),
                    bold,
                    italic,
                    strikethrough,
                    is_code: false,
                    link_url: None,
                });
                *i += 1;
            }
            _ => {
                *i += 1;
            }
        }
    }

    spans
}

fn render_table(ui: &mut Ui, events: &[Event], i: &mut usize, table_id: usize) {
    let mut header_cells: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_head = false;

    while *i < events.len() {
        match &events[*i] {
            Event::End(TagEnd::Table) => {
                *i += 1;
                break;
            }
            Event::Start(Tag::TableHead) => {
                in_head = true;
                *i += 1;
            }
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                *i += 1;
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
                *i += 1;
            }
            Event::End(TagEnd::TableRow) => {
                rows.push(std::mem::take(&mut current_row));
                *i += 1;
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
                *i += 1;
            }
            Event::End(TagEnd::TableCell) => {
                let cell_txt = std::mem::take(&mut current_cell);
                if in_head {
                    header_cells.push(cell_txt);
                } else {
                    current_row.push(cell_txt);
                }
                *i += 1;
            }
            Event::Text(t) | Event::Code(t) => {
                current_cell.push_str(t);
                *i += 1;
            }
            _ => {
                *i += 1;
            }
        }
    }

    if header_cells.is_empty() && rows.is_empty() {
        return;
    }

    let num_cols = if !header_cells.is_empty() {
        header_cells.len()
    } else {
        rows.first().map(|r| r.len()).unwrap_or(1)
    };

    ui.add_space(6.0);
    let avail_w = ui.available_width().max(200.0);
    let inner_w = (avail_w - 24.0).max(180.0);

    let col_widths: Vec<f32> = if num_cols == 2 {
        // Tabela dwukolumnowa (np. Moduł / Odpowiedzialność): pierwsza kolumna zwięzła, druga zajmuje resztę
        let col0 = (inner_w * 0.28).clamp(180.0, 250.0);
        let col1 = (inner_w - col0 - 24.0).max(300.0);
        vec![col0, col1]
    } else if num_cols == 1 {
        vec![inner_w.max(250.0)]
    } else {
        // Wiele kolumn — proporcjonalny podział
        let w = ((inner_w - (num_cols as f32 * 14.0)) / num_cols as f32).max(140.0);
        vec![w; num_cols]
    };

    egui::Frame::NONE
        .fill(Color32::from_rgb(22, 25, 32))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(60, 68, 85)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10_i8, 8_i8))
        .show(ui, |ui| {
            ui.set_max_width(inner_w);
            egui::ScrollArea::horizontal()
                .id_salt(("md_table_scroll", table_id))
                .auto_shrink([false, true])
                .max_width(inner_w)
                .show(ui, |ui| {
                    egui::Grid::new(("md_table_grid", table_id))
                        .striped(true)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            if !header_cells.is_empty() {
                                for (col_idx, cell) in header_cells.iter().enumerate() {
                                    let col_w = col_widths.get(col_idx).copied().unwrap_or(160.0);
                                    ui.allocate_ui_with_layout(
                                        egui::Vec2::new(col_w, 0.0),
                                        egui::Layout::top_down(egui::Align::LEFT),
                                        |ui| {
                                            ui.set_min_width(col_w);
                                            ui.set_max_width(col_w);
                                            let mut job = LayoutJob::default();
                                            job.wrap.max_width = col_w;
                                            job.wrap.break_anywhere = false;
                                            job.append(
                                                cell,
                                                0.0,
                                                TextFormat {
                                                    font_id: TextStyle::Body.resolve(ui.style()),
                                                    color: Color32::from_rgb(180, 215, 255),
                                                    ..Default::default()
                                                },
                                            );
                                            ui.label(job);
                                        },
                                    );
                                }
                                ui.end_row();
                            }
                            for row in &rows {
                                for (col_idx, cell) in row.iter().enumerate() {
                                    let col_w = col_widths.get(col_idx).copied().unwrap_or(160.0);
                                    ui.allocate_ui_with_layout(
                                        egui::Vec2::new(col_w, 0.0),
                                        egui::Layout::top_down(egui::Align::LEFT),
                                        |ui| {
                                            ui.set_min_width(col_w);
                                            ui.set_max_width(col_w);
                                            let mut job = LayoutJob::default();
                                            job.wrap.max_width = col_w;
                                            job.wrap.break_anywhere = false;
                                            job.append(
                                                cell,
                                                0.0,
                                                TextFormat {
                                                    font_id: TextStyle::Body.resolve(ui.style()),
                                                    color: Color32::from_rgb(215, 225, 240),
                                                    ..Default::default()
                                                },
                                            );
                                            ui.label(job);
                                        },
                                    );
                                }
                                ui.end_row();
                            }
                        });
                });
        });
    ui.add_space(6.0);
}


