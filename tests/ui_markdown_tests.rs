use huzar_commander::ui::markdown_viewer::collect_inlines;
use huzar_commander::ui::mermaid_layout::{parse, Diagram};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

#[test]
fn test_collect_inlines_bold_code_links() {
    let md = "Tekst ze **słowem kluczowym**, kodem `cargo build` i [linkiem](https://rust-lang.org).";
    let parser = Parser::new_ext(md, Options::empty());
    let events: Vec<Event> = parser.collect();
    let mut i = 0;

    // Skip start paragraph
    if let Event::Start(Tag::Paragraph) = &events[0] {
        i += 1;
    }

    let spans = collect_inlines(&events, &mut i, |end| matches!(end, TagEnd::Paragraph));
    assert!(!spans.is_empty());

    let bold_span = spans.iter().find(|s| s.text.contains("słowem kluczowym"));
    assert!(bold_span.is_some());
    assert!(bold_span.unwrap().bold);

    let code_span = spans.iter().find(|s| s.is_code);
    assert!(code_span.is_some());
    assert!(code_span.unwrap().text.contains("cargo build"));

    let link_span = spans.iter().find(|s| s.link_url.is_some());
    assert!(link_span.is_some());
    assert_eq!(
        link_span.unwrap().link_url.as_deref(),
        Some("https://rust-lang.org")
    );
}

#[test]
fn test_mermaid_fenced_block_parsing() {
    let md = "Wprowadzenie:\n\n```mermaid\nsequenceDiagram\n    actor User\n    participant UI\n    User->>UI: Klik\n```\n";
    let parser = Parser::new_ext(md, Options::all());
    let events: Vec<Event> = parser.collect();

    let mut found_mermaid = false;
    for ev in &events {
        if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) = ev {
            if lang.trim().eq_ignore_ascii_case("mermaid") {
                found_mermaid = true;
            }
        }
    }
    assert!(found_mermaid, "Blok mermaid powinien zostać prawidłowo wykryty");
}

#[test]
fn test_mermaid_diagram_layout_and_parse() {
    let source = "sequenceDiagram\n    actor User\n    participant App\n    User->>App: Akcja";
    let measure = |_: &str| (40.0, 16.0);
    let diag = parse(source, &measure);
    match diag {
        Diagram::Sequence(seq) => {
            assert!(!seq.lanes.is_empty());
            assert!(seq.size.0 > 0.0);
            assert!(seq.size.1 > 0.0);
        }
        other => panic!("Oczekiwano Diagram::Sequence, otrzymano {:?}", other),
    }
}
