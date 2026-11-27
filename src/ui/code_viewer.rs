use std::path::Path;

use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLang {
    Rust,
    Python,
    JsTs,
    CStyle,
    TomlIni,
    Sql,
    Shell,
    Markup,
    Css,
    Plain,
}

pub fn detect_code_lang(path: Option<&Path>) -> CodeLang {
    let Some(ext) = path.and_then(|p| p.extension()).and_then(|e| e.to_str()) else {
        return CodeLang::Plain;
    };
    match ext.to_ascii_lowercase().as_str() {
        "rs" => CodeLang::Rust,
        "py" => CodeLang::Python,
        "js" | "jsx" | "ts" | "tsx" => CodeLang::JsTs,
        "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "go" => CodeLang::CStyle,
        "toml" | "ini" | "cfg" => CodeLang::TomlIni,
        "sql" => CodeLang::Sql,
        "sh" | "bash" | "zsh" | "ps1" | "bat" | "cmd" => CodeLang::Shell,
        "html" | "htm" | "xml" => CodeLang::Markup,
        "css" | "scss" => CodeLang::Css,
        _ => CodeLang::Plain,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Keyword,
    TypeName,
    StringLit,
    Comment,
    Number,
    MacroOrAttr,
    Punctuation,
    MatchHighlight,
}

fn is_keyword(word: &str, lang: CodeLang) -> bool {
    match lang {
        CodeLang::Rust => matches!(
            word,
            "fn" | "let"
                | "mut"
                | "pub"
                | "struct"
                | "enum"
                | "impl"
                | "trait"
                | "for"
                | "in"
                | "while"
                | "loop"
                | "if"
                | "else"
                | "match"
                | "return"
                | "use"
                | "mod"
                | "crate"
                | "super"
                | "self"
                | "Self"
                | "where"
                | "async"
                | "await"
                | "const"
                | "static"
                | "type"
                | "unsafe"
                | "ref"
                | "as"
                | "break"
                | "continue"
                | "true"
                | "false"
        ),
        CodeLang::Python => matches!(
            word,
            "def"
                | "class"
                | "import"
                | "from"
                | "as"
                | "if"
                | "elif"
                | "else"
                | "for"
                | "in"
                | "while"
                | "return"
                | "yield"
                | "with"
                | "try"
                | "except"
                | "finally"
                | "raise"
                | "pass"
                | "break"
                | "continue"
                | "and"
                | "or"
                | "not"
                | "is"
                | "None"
                | "True"
                | "False"
                | "async"
                | "await"
                | "lambda"
        ),
        CodeLang::JsTs => matches!(
            word,
            "function"
                | "const"
                | "let"
                | "var"
                | "if"
                | "else"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "break"
                | "continue"
                | "return"
                | "import"
                | "export"
                | "from"
                | "default"
                | "class"
                | "extends"
                | "interface"
                | "type"
                | "async"
                | "await"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "new"
                | "this"
                | "true"
                | "false"
                | "null"
                | "undefined"
        ),
        CodeLang::CStyle => matches!(
            word,
            "if" | "else"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "break"
                | "continue"
                | "return"
                | "struct"
                | "class"
                | "enum"
                | "public"
                | "private"
                | "protected"
                | "static"
                | "const"
                | "void"
                | "int"
                | "long"
                | "float"
                | "double"
                | "char"
                | "bool"
                | "func"
                | "package"
                | "import"
                | "true"
                | "false"
                | "null"
                | "nullptr"
                | "nil"
        ),
        CodeLang::Sql => matches!(
            word.to_ascii_uppercase().as_str(),
            "SELECT"
                | "FROM"
                | "WHERE"
                | "JOIN"
                | "LEFT"
                | "RIGHT"
                | "INNER"
                | "ON"
                | "GROUP"
                | "BY"
                | "ORDER"
                | "LIMIT"
                | "INSERT"
                | "INTO"
                | "VALUES"
                | "UPDATE"
                | "SET"
                | "DELETE"
                | "CREATE"
                | "TABLE"
                | "DROP"
                | "ALTER"
                | "AND"
                | "OR"
                | "NOT"
                | "NULL"
                | "AS"
                | "DISTINCT"
                | "COUNT"
        ),
        CodeLang::Shell => matches!(
            word.to_ascii_lowercase().as_str(),
            "if" | "then"
                | "else"
                | "elif"
                | "fi"
                | "for"
                | "in"
                | "do"
                | "done"
                | "while"
                | "case"
                | "esac"
                | "function"
                | "return"
                | "export"
                | "local"
                | "echo"
                | "exit"
                | "param"
                | "foreach"
                | "set"
        ),
        CodeLang::TomlIni => matches!(word, "true" | "false"),
        CodeLang::Css => matches!(word, "important" | "media" | "keyframes" | "import"),
        CodeLang::Markup | CodeLang::Plain => false,
    }
}

fn is_type_name(word: &str, lang: CodeLang) -> bool {
    if matches!(
        word,
        "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Path"
            | "PathBuf"
            | "HashMap"
            | "HashSet"
            | "Arc"
            | "Mutex"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "f32"
            | "f64"
            | "bool"
            | "str"
            | "char"
    ) {
        return lang != CodeLang::Plain;
    }
    lang != CodeLang::Plain
        && lang != CodeLang::Sql
        && word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_lowercase())
}

pub fn tokenize_code_line(line: &str, lang: CodeLang) -> Vec<(TokenKind, String)> {
    if lang == CodeLang::Plain {
        return if line.is_empty() {
            Vec::new()
        } else {
            vec![(TokenKind::Plain, line.to_string())]
        };
    }

    let mut out = Vec::new();
    let mut i = 0usize;
    let bytes = line.as_bytes();

    while i < bytes.len() {
        let rest = &line[i..];

        // Comments
        let is_comment = match lang {
            CodeLang::Rust | CodeLang::JsTs | CodeLang::CStyle => rest.starts_with("//"),
            CodeLang::Python | CodeLang::TomlIni | CodeLang::Shell => rest.starts_with('#'),
            CodeLang::Sql => rest.starts_with("--"),
            CodeLang::Css => rest.starts_with("/*"),
            CodeLang::Markup => rest.starts_with("<!--"),
            CodeLang::Plain => false,
        };
        if is_comment {
            out.push((TokenKind::Comment, rest.to_string()));
            break;
        }

        // Rust attributes (#[...], #![...])
        if lang == CodeLang::Rust && (rest.starts_with("#[") || rest.starts_with("#![")) {
            out.push((TokenKind::MacroOrAttr, rest.to_string()));
            break;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };

        // String literals
        if ch == '"' || ch == '\'' || (lang == CodeLang::JsTs && ch == '`') {
            if lang == CodeLang::Rust && ch == '\'' {
                // Distinguish Rust lifetime `'a` from char literal `'a'`
                let mut chars_it = rest[1..].chars();
                let first = chars_it.next();
                let second = chars_it.next();
                if first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && second != Some('\'')
                {
                    let mut len = 1;
                    for c in rest[1..].chars() {
                        if c.is_ascii_alphanumeric() || c == '_' {
                            len += c.len_utf8();
                        } else {
                            break;
                        }
                    }
                    out.push((TokenKind::TypeName, rest[..len].to_string()));
                    i += len;
                    continue;
                }
            }

            let quote = ch;
            let mut len = quote.len_utf8();
            let mut escaped = false;
            for c in rest[len..].chars() {
                len += c.len_utf8();
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    break;
                }
            }
            out.push((TokenKind::StringLit, rest[..len].to_string()));
            i += len;
            continue;
        }

        // Decorators (@...)
        if (lang == CodeLang::Python || lang == CodeLang::JsTs) && ch == '@' {
            let mut len = 1;
            for c in rest[1..].chars() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    len += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push((TokenKind::MacroOrAttr, rest[..len].to_string()));
            i += len;
            continue;
        }

        // Numbers
        if ch.is_ascii_digit() {
            let mut len = 0;
            for c in rest.chars() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    len += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push((TokenKind::Number, rest[..len].to_string()));
            i += len;
            continue;
        }

        // Identifiers / keywords
        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut len = 0;
            for c in rest.chars() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    len += c.len_utf8();
                } else {
                    break;
                }
            }
            let word = &rest[..len];
            let after_word = &rest[len..];
            if lang == CodeLang::Rust && after_word.starts_with('!') {
                len += 1;
                out.push((TokenKind::MacroOrAttr, rest[..len].to_string()));
            } else if is_keyword(word, lang) {
                out.push((TokenKind::Keyword, word.to_string()));
            } else if is_type_name(word, lang) {
                out.push((TokenKind::TypeName, word.to_string()));
            } else {
                out.push((TokenKind::Plain, word.to_string()));
            }
            i += len;
            continue;
        }

        // Whitespace batching
        if ch.is_whitespace() {
            let mut len = 0;
            for c in rest.chars() {
                if c.is_whitespace() {
                    len += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push((TokenKind::Plain, rest[..len].to_string()));
            i += len;
            continue;
        }

        // Punctuation / symbols
        let len = ch.len_utf8();
        out.push((TokenKind::Punctuation, rest[..len].to_string()));
        i += len;
    }

    out
}

fn color_for_token(kind: TokenKind) -> (Color32, Color32) {
    match kind {
        TokenKind::Plain => (Color32::from_rgb(220, 230, 245), Color32::TRANSPARENT),
        TokenKind::Keyword => (Color32::from_rgb(255, 130, 180), Color32::TRANSPARENT),
        TokenKind::TypeName => (Color32::from_rgb(115, 215, 255), Color32::TRANSPARENT),
        TokenKind::StringLit => (Color32::from_rgb(165, 228, 135), Color32::TRANSPARENT),
        TokenKind::Comment => (Color32::from_rgb(115, 132, 158), Color32::TRANSPARENT),
        TokenKind::Number => (Color32::from_rgb(255, 195, 110), Color32::TRANSPARENT),
        TokenKind::MacroOrAttr => (Color32::from_rgb(235, 175, 255), Color32::TRANSPARENT),
        TokenKind::Punctuation => (Color32::from_rgb(165, 180, 205), Color32::TRANSPARENT),
        TokenKind::MatchHighlight => (Color32::WHITE, Color32::from_rgb(155, 110, 10)),
    }
}

fn append_highlighted_piece(
    job: &mut LayoutJob,
    piece: &str,
    kind: TokenKind,
    query_lower: Option<&str>,
    font_id: &FontId,
) {
    let (fg, bg) = color_for_token(kind);
    let Some(q) = query_lower.filter(|s| !s.is_empty()) else {
        job.append(
            piece,
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: fg,
                background: bg,
                ..Default::default()
            },
        );
        return;
    };

    let piece_lower = piece.to_lowercase();
    if piece_lower.len() != piece.len() {
        // Length changed under Unicode lowercasing (e.g. ẞ): safe char-agnostic fallback
        if piece_lower.contains(q) {
            let (hfg, hbg) = color_for_token(TokenKind::MatchHighlight);
            job.append(
                piece,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: hfg,
                    background: hbg,
                    ..Default::default()
                },
            );
        } else {
            job.append(
                piece,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: fg,
                    background: bg,
                    ..Default::default()
                },
            );
        }
        return;
    }

    let mut pos = 0usize;
    while let Some(rel) = piece_lower[pos..].find(q) {
        let m_start = pos + rel;
        let m_end = m_start + q.len();
        if m_start > pos {
            job.append(
                &piece[pos..m_start],
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: fg,
                    background: bg,
                    ..Default::default()
                },
            );
        }
        let (hfg, hbg) = color_for_token(TokenKind::MatchHighlight);
        job.append(
            &piece[m_start..m_end],
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: hfg,
                background: hbg,
                ..Default::default()
            },
        );
        pos = m_end;
    }
    if pos < piece.len() {
        job.append(
            &piece[pos..],
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                color: fg,
                background: bg,
                ..Default::default()
            },
        );
    }
}

pub fn render_code_view(
    ui: &mut egui::Ui,
    text: &str,
    lang: CodeLang,
    word_wrap: bool,
    search_highlight: Option<&str>,
) {
    let font_id = FontId::monospace(13.0);
    let line_num_font = FontId::monospace(12.0);
    let q_lower = search_highlight.map(|s| s.trim().to_lowercase());
    let q_ref = q_lower.as_deref().filter(|s| !s.is_empty());

    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len().max(1);
    let num_digits = total_lines.to_string().len().max(3);
    let gutter_w = (num_digits as f32) * 8.0 + 16.0;
    let code_avail_w = (ui.available_width() - gutter_w - 16.0).max(160.0);

    for (idx, line) in lines.iter().take(4000).enumerate() {
        ui.horizontal_top(|ui| {
            ui.add_sized(
                [gutter_w, 16.0],
                egui::Label::new(
                    egui::RichText::new(format!("{:>width$} ", idx + 1, width = num_digits))
                        .font(line_num_font.clone())
                        .color(Color32::from_rgb(85, 102, 130)),
                )
                .selectable(false),
            );

            let mut job = LayoutJob::default();
            job.wrap.max_width = if word_wrap {
                code_avail_w
            } else {
                f32::INFINITY
            };

            let tokens = tokenize_code_line(line, lang);
            if tokens.is_empty() {
                job.append(
                    " ",
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: Color32::TRANSPARENT,
                        ..Default::default()
                    },
                );
            } else {
                for (kind, txt) in tokens {
                    append_highlighted_piece(&mut job, &txt, kind, q_ref, &font_id);
                }
            }

            ui.add(egui::Label::new(job).selectable(true));
        });
    }
}
