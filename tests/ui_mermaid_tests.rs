use huzar_commander::ui::mermaid_layout::*;

/// Eight units a character, sixteen a line.
fn measure(text: &str) -> (f32, f32) {
    let lines: Vec<&str> = text.split('\n').collect();
    (
        lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * 8.0,
        lines.len() as f32 * 16.0,
    )
}

fn gantt_of(source: &str) -> Gantt {
    match parse(source, &measure) {
        Diagram::Gantt(g) => g,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_journey_puts_tasks_under_their_sections_at_their_scores() {
    let j = match parse(
        "journey\n  title My day\n  section Morning\n    Make tea: 5: Me\n    Commute: 2: Me, Cat\n  section Evening\n    Read: 4: Me\n",
        &measure,
    ) {
        Diagram::Journey(j) => j,
        other => panic!("{other:?}"),
    };
    assert_eq!(j.title, "My day");
    assert_eq!(
        j.tasks
            .iter()
            .map(|t| (t.label.as_str(), t.score, t.section))
            .collect::<Vec<_>>(),
        [("Make tea", 5, 0), ("Commute", 2, 0), ("Read", 4, 1)]
    );
    assert_eq!(j.tasks[1].actors, "Me, Cat");
    assert!(j.tasks[0].x < j.tasks[1].x && j.tasks[1].x < j.tasks[2].x);
    // Section bands cover their tasks.
    assert!(
        j.sections[0].1 < j.tasks[0].x
            && j.tasks[1].x < j.sections[0].2
            && j.sections[0].2 <= j.sections[1].1
    );
}

#[test]
fn a_git_graph_has_a_lane_per_branch_and_lines_for_merges() {
    let g = match parse(
        "gitGraph\n  commit\n  commit id: \"A\"\n  branch develop\n  checkout develop\n  commit\n  checkout main\n  commit tag: \"v1\"\n  merge develop\n",
        &measure,
    ) {
        Diagram::Git(g) => g,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        g.lanes.iter().map(|l| l.0.as_str()).collect::<Vec<_>>(),
        ["main", "develop"]
    );
    assert_eq!(g.commits.len(), 5);
    assert_eq!(g.commits[1].label, "A");
    assert_eq!(g.commits[3].tag, "v1");
    assert_eq!(
        g.commits.iter().map(|c| c.lane).collect::<Vec<_>>(),
        [0, 0, 1, 0, 0]
    );
    assert!(g.commits[4].merge);
    // develop's first commit starts from A; the merge draws from develop's tip.
    assert!(g.lines.contains(&(g.commits[1].at, g.commits[2].at, 1)));
    assert!(g.lines.contains(&(g.commits[2].at, g.commits[4].at, 1)));
    assert!(g.commits.windows(2).all(|w| w[0].at.0 < w[1].at.0));
}

#[test]
fn tick_intervals_follow_d3_fields_and_today_is_marked() {
    let day = |y, m, d| days_from_civil(y, m, d) as f64;
    let lines = |extra: &str| -> Vec<String> {
        let text = format!("dateFormat YYYY-MM-DD\n{extra}\nA : 2026-10-01, 2026-11-20\n");
        text.lines().map(str::to_string).collect()
    };
    let chart = |extra: &str, today: f64| {
        let owned = lines(extra);
        gantt(
            &owned.iter().map(String::as_str).collect::<Vec<_>>(),
            &measure,
            today,
        )
    };
    let texts = |g: &Gantt| g.ticks.iter().map(|t| t.1.clone()).collect::<Vec<_>>();
    // Days of the month 1, 11, 21, 31; then Sundays of even weeks since 1970.
    let ten = chart("tickInterval 10day", 0.0);
    assert_eq!(
        texts(&ten),
        ["10-01", "10-11", "10-21", "10-31", "11-01", "11-11"]
    );
    let weeks = chart("tickInterval 2week\naxisFormat %a %d", 0.0);
    assert_eq!(texts(&weeks), ["Sun 04", "Sun 18", "Sun 01", "Sun 15"]);
    assert_eq!(
        texts(&chart("tickInterval 1month", 0.0)),
        ["2026-10", "2026-11"]
    );
    // Too many ticks or a unit Mermaid lacks: the automatic ones.
    let automatic = texts(&chart("", 0.0));
    assert_eq!(texts(&chart("tickInterval 1minute", 0.0)), automatic);
    assert_eq!(texts(&chart("tickInterval 1fortnight", 0.0)), automatic);
    assert_eq!(
        interval_ticks(0.0, 0.25, 3, "hour").map(|t| t.len()),
        Some(3)
    );
    // Today, inside the axis: red and 2 px unless styled; outside or off: none.
    let marked = chart("", day(2026, 10, 14));
    let x = marked.ticks[0].0
        + (marked.ticks[1].0 - marked.ticks[0].0)
            * (day(2026, 10, 14) - day(2026, 10, 1)) as f32
            / 7.0;
    let today = marked.today.unwrap();
    assert!((today.x - x).abs() < 1e-3, "{} against {x}", today.x);
    assert_eq!(
        (today.rgb, today.width, today.opacity),
        ([255, 0, 0], 2.0, 1.0)
    );
    let styled = chart(
        "todayMarker stroke-width:5px,stroke:#0f0,opacity:0.5",
        day(2026, 10, 14),
    )
    .today
    .unwrap();
    assert_eq!(
        (styled.rgb, styled.width, styled.opacity),
        ([0, 255, 0], 5.0, 0.5)
    );
    assert!(chart("todayMarker off", day(2026, 10, 14)).today.is_none());
    assert!(chart("", day(2027, 1, 1)).today.is_none());
}

#[test]
fn dates_are_days_since_1970() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(
        days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 28),
        2
    );
    assert_eq!(
        civil_from_days(days_from_civil(2026, 10, 9)),
        (2026, 10, 9)
    );
    assert_eq!(
        gantt_date("2026-10-09 12:00", "YYYY-MM-DD"),
        Some(days_from_civil(2026, 10, 9) as f64 + 0.5)
    );
    assert_eq!(gantt_duration("2w"), Some(14.0));
    assert_eq!(gantt_date("tomorrow", "YYYY-MM-DD"), None);
    assert_eq!(
        gantt_date("9.10.2026", "D.M.YYYY"),
        Some(days_from_civil(2026, 10, 9) as f64)
    );
    assert_eq!(
        gantt_date("10/09/26 06:30", "MM/DD/YY HH:mm"),
        Some(days_from_civil(2026, 10, 9) as f64 + 0.270_833_333_333_333_3)
    );
    assert_eq!(gantt_date("86400", "X"), Some(1.0));
    // A date in another format still reads.
    assert_eq!(
        gantt_date("2026-10-09", "DD.MM.YYYY"),
        Some(days_from_civil(2026, 10, 9) as f64)
    );
}

#[test]
fn date_formats_take_names_quarters_ordinals_and_twelve_hours() {
    let day = |y, m, d| days_from_civil(y, m, d) as f64;
    assert_eq!(
        gantt_date("Oct 9, 2026", "MMM D, YYYY"),
        Some(day(2026, 10, 9))
    );
    assert_eq!(
        gantt_date("9 september 2026", "D MMMM YYYY"),
        Some(day(2026, 9, 9))
    );
    assert_eq!(
        gantt_date("Fri, 9th Oct 26", "ddd, Do MMM YY"),
        Some(day(2026, 10, 9))
    );
    assert_eq!(gantt_date("Q3 2026", "[Q]Q YYYY"), Some(day(2026, 7, 1)));
    assert_eq!(
        gantt_date("2026-10-09 6:30 PM", "YYYY-MM-DD h:mm A"),
        Some(day(2026, 10, 9) + 18.5 / 24.0)
    );
    assert_eq!(
        gantt_date("2026-10-09 12:00 am", "YYYY-MM-DD hh:mm a"),
        Some(day(2026, 10, 9))
    );
    assert_eq!(
        gantt_date("2026-10-09T01:30:00+01:30", "YYYY-MM-DD[T]HH:mm:ssZ"),
        Some(day(2026, 10, 9))
    );
    assert_eq!(
        gantt_date("99-01-02", "YY-MM-DD"),
        Some(day(1999, 1, 2))
    );
    assert_eq!(gantt_date("Q5 2026", "[Q]Q YYYY"), None);
    assert_eq!(gantt_date("Okt 9, 2026", "MMM D, YYYY"), None);
    let noon = day(2026, 10, 9) + 0.5;
    assert_eq!(
        axis_date(noon, "%a %-d %b %Y, %I:%M %p (%j) %%"),
        "Fri 9 Oct 2026, 12:00 PM (282) %"
    );
    assert_eq!(
        axis_date(day(2026, 1, 5), "%e/%m/%y %B %A"),
        " 5/01/26 January Monday"
    );
    // In a chart: two quarters, the second twice as long.
    let g = gantt_of(
        "gantt\n    dateFormat [Q]Q YYYY\n    A : Q1 2026, Q2 2026\n    B : Q2 2026, Q4 2026\n",
    );
    let (a, b) = (
        g.bars[0].rect.2 - g.bars[0].rect.0,
        g.bars[1].rect.2 - g.bars[1].rect.0,
    );
    assert!((b / a - 183.0 / 90.0).abs() < 1e-3, "{a} {b}");
}

#[test]
fn gantt_tasks_follow_one_another_and_their_dependencies() {
    let g = gantt_of(
        "gantt\n    title Release\n    dateFormat YYYY-MM-DD\n    section Build\n    Design :done, a1, 2026-01-01, 10d\n    Code :active, a2, after a1, 2w\n    Ship :milestone, m1, after a2, 0d\n    section Test\n    Review :crit, 2026-01-05, until m1\n    Notes : 3d\n",
    );
    assert_eq!(g.title, "Release");
    assert_eq!(g.bars.len(), 5);
    let unit = (g.bars[0].rect.2 - g.bars[0].rect.0) / 10.0;
    // Code starts where Design ends and lasts 14 days.
    assert!((g.bars[1].rect.0 - g.bars[0].rect.2).abs() < 1e-3);
    assert!((g.bars[1].rect.2 - g.bars[1].rect.0 - 14.0 * unit).abs() < 1e-3);
    // Ship is a milestone at day 24; Review ends there; Notes follows Review.
    assert!(g.bars[2].milestone && g.bars[2].rect.0 == g.bars[2].rect.2);
    assert!((g.bars[3].rect.2 - g.bars[2].rect.0).abs() < 1e-3);
    assert!((g.bars[4].rect.0 - g.bars[3].rect.2).abs() < 1e-3);
    assert!(g.bars[0].done && g.bars[1].active && g.bars[3].crit);
    assert_eq!(
        g.sections.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(),
        ["Build", "Test"]
    );
    // Rows below one another, a section heading row each.
    assert!(g.bars[3].rect.1 > g.bars[2].rect.3 + GANTT_ROW - 8.0);
    assert!(!g.ticks.is_empty() && g.ticks.len() <= 11);
}

#[test]
fn gantt_durations_skip_excluded_days() {
    // 2026-01-02 is a Friday: three working days end on Wednesday.
    let g = gantt_of(
        "gantt\n    dateFormat DD.MM.YYYY\n    excludes weekends, 06.01.2026\n    A : a, 02.01.2026, 3d\n    B : after a, 1d\n    C : 05.01.2026, 08.01.2026\n",
    );
    assert_eq!(g.bars.len(), 3);
    let unit = (g.bars[2].rect.2 - g.bars[2].rect.0) / 3.0;
    let day = |x: f32| ((x - g.bars[0].rect.0) / unit).round() as i64;
    // Fri, Mon, Wed (Tuesday the 6th excluded): A ends on Thursday the 8th.
    assert_eq!(day(g.bars[0].rect.2), 6);
    assert_eq!((day(g.bars[1].rect.0), day(g.bars[1].rect.2)), (6, 7));
    // Saturday and Sunday shaded as one run, Tuesday as another.
    assert_eq!(
        g.excluded
            .iter()
            .map(|&(a, b)| (day(a), day(b)))
            .collect::<Vec<_>>(),
        [(1, 3), (4, 5)]
    );
}

#[test]
fn git_lanes_follow_their_order_and_commits_keep_their_type() {
    let g = match parse(
        "gitGraph\n  commit\n  branch b order: 2\n  commit type: HIGHLIGHT\n  branch c order: 1\n  commit type: REVERSE\n  branch d\n  commit\n  cherry-pick id: \"1\"\n",
        &measure,
    ) {
        Diagram::Git(g) => g,
        other => panic!("{other:?}"),
    };
    let y = |name: &str| g.lanes.iter().find(|l| l.0 == name).unwrap().1;
    // main 0, c 1, b 2, d (made fourth) 3.
    assert!(y("main") < y("c") && y("c") < y("b") && y("b") < y("d"));
    assert_eq!(
        g.commits.iter().map(|c| c.kind).collect::<Vec<_>>(),
        [
            CommitKind::Normal,
            CommitKind::Highlight,
            CommitKind::Reverse,
            CommitKind::Normal,
            CommitKind::CherryPick
        ]
    );
    assert_eq!(
        (g.commits[4].label.as_str(), g.commits[4].tag.as_str()),
        ("4", "cherry-pick:1")
    );
    assert!(g.commits.iter().all(|c| c.at.1 == g.lanes[c.lane].1));
    assert!(g.lines.iter().all(|(a, b, _)| g
        .lanes
        .iter()
        .any(|l| l.1 == a.1)
        && g.lanes.iter().any(|l| l.1 == b.1)));
}

fn flow_of(source: &str) -> Flow {
    match parse(source, &measure) {
        Diagram::Flow(flow) => flow,
        other => panic!("{other:?}"),
    }
}

fn node<'f>(flow: &'f Flow, id: &str) -> &'f Node {
    flow.nodes.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn a_flowchart_has_its_shapes_labels_and_edges() {
    let flow = flow_of("flowchart TD\n  A[Start] --> B{Is it?}\n  B -->|Yes| C(Round)\n  B -- No --> D([Stadium])\n  C & D -.-> E((Done)) %% end\n  E ==> F[\"Quoted [x]\"]; F --- A\n  classDef red fill:#f00\n");
    let shapes: Vec<(&str, &str, Shape)> = flow
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str(), n.shape))
        .collect();
    assert_eq!(
        shapes,
        [
            ("A", "Start", Shape::Rect),
            ("B", "Is it?", Shape::Diamond),
            ("C", "Round", Shape::Round),
            ("D", "Stadium", Shape::Stadium),
            ("E", "Done", Shape::Circle),
            ("F", "Quoted [x]", Shape::Rect),
        ]
    );
    let edges: Vec<(&str, Line, Head)> = flow
        .edges
        .iter()
        .map(|e| (e.label.as_str(), e.line, e.head))
        .collect();
    assert_eq!(
        edges,
        [
            ("", Line::Solid, Head::Arrow),
            ("Yes", Line::Solid, Head::Arrow),
            ("No", Line::Solid, Head::Arrow),
            ("", Line::Dotted, Head::Arrow),
            ("", Line::Dotted, Head::Arrow),
            ("", Line::Thick, Head::Arrow),
            ("", Line::Solid, Head::None),
        ]
    );
    // Top to bottom: each edge but the one back to A goes down.
    assert!(
        node(&flow, "A").y < node(&flow, "B").y
            && node(&flow, "B").y < node(&flow, "C").y
            && node(&flow, "E").y < node(&flow, "F").y
    );
    // C and D share a rank, side by side without overlapping.
    let (c, d) = (node(&flow, "C"), node(&flow, "D"));
    assert_eq!(c.y, d.y);
    assert!((c.x - d.x).abs() >= (c.w + d.w) / 2.0);
    // Every point inside the drawing.
    for p in flow.edges.iter().flat_map(|e| &e.points) {
        assert!(
            p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= flow.size.0 && p.1 <= flow.size.1,
            "{p:?} outside {:?}",
            flow.size
        );
    }
}

#[test]
fn left_to_right_puts_ranks_along_x_and_long_edges_bend_through_dummies() {
    let flow = flow_of("graph LR\nA --> B --> C --> D\nA --> D\nD --> A");
    let xs: Vec<f32> = ["A", "B", "C", "D"]
        .iter()
        .map(|id| node(&flow, id).x)
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    // A to D spans three ranks: its ends and two dummies, each crossed
    // straight through its rank.
    let long = &flow.edges[3].points;
    assert_eq!(long.len(), 6);
    assert!(long[1].1 == long[2].1 && long[3].1 == long[4].1 && long[1].0 < long[2].0);
    // D to A was a back edge: it is drawn from D to A all the same.
    let back = &flow.edges[4];
    assert!(back.points.first().unwrap().0 > back.points.last().unwrap().0);
}

/// Edges whose segments pass through a node other than their own ends.
fn crossings(flow: &Flow, ends: &[(&str, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    for (edge, &(from, to)) in flow.edges.iter().zip(ends) {
        for n in flow.nodes.iter().filter(|n| n.id != from && n.id != to) {
            let (x0, y0, x1, y1) = (
                n.x - n.w / 2.0,
                n.y - n.h / 2.0,
                n.x + n.w / 2.0,
                n.y + n.h / 2.0,
            );
            let hit = edge.points.windows(2).any(|s| {
                (0..=64).any(|i| {
                    let t = i as f32 / 64.0;
                    let (x, y) = (
                        s[0].0 + (s[1].0 - s[0].0) * t,
                        s[0].1 + (s[1].1 - s[0].1) * t,
                    );
                    x > x0 && x < x1 && y > y0 && y < y1
                })
            });
            if hit {
                out.push(format!("{from}->{to} through {}", n.id));
            }
        }
    }
    out
}

#[test]
fn long_edges_pass_beside_the_nodes_of_the_ranks_they_skip() {
    let source = "graph TD\nA --> B --> C --> D\nA --> D\nA --> X[A very wide node in the middle] --> D\nB --> Y[Another wide one] --> D\nA --> Z --> D";
    let flow = flow_of(source);
    let ends = [
        ("A", "B"),
        ("B", "C"),
        ("C", "D"),
        ("A", "D"),
        ("A", "X"),
        ("X", "D"),
        ("B", "Y"),
        ("Y", "D"),
        ("A", "Z"),
        ("Z", "D"),
    ];
    assert_eq!(crossings(&flow, &ends), Vec::<String>::new());
    let flow = flow_of(&source.replace("TD", "LR"));
    assert_eq!(crossings(&flow, &ends), Vec::<String>::new());
}

#[test]
fn class_diagrams_box_members_and_point_relations_at_their_marks() {
    let flow = flow_of("classDiagram\nclass Animal {\n  +String name\n  +eat()\n}\nAnimal <|-- Duck\nDuck : +swim()\nDuck \"1\" *-- \"2\" Wing : has\nDuck ..> Pond");
    assert_eq!(
        node(&flow, "Animal").label,
        "Animal\n+String name\n+eat()"
    );
    assert_eq!(node(&flow, "Duck").label, "Duck\n+swim()");
    assert_eq!(
        (node(&flow, "Animal").rule, node(&flow, "Pond").rule),
        (1, 0)
    );
    // Duck inherits Animal: Animal above, and the arrow ends there.
    let (animal, duck) = (node(&flow, "Animal"), node(&flow, "Duck"));
    assert!(animal.y < duck.y);
    assert!(flow.edges[0].points.last().unwrap().1 < flow.edges[0].points[0].1);
    assert_eq!(
        (flow.edges[0].line, flow.edges[0].head),
        (Line::Solid, Head::Arrow)
    );
    assert_eq!(
        (flow.edges[1].label.as_str(), flow.edges[1].head),
        ("has 1 : 2", Head::Circle)
    );
    assert_eq!(
        (flow.edges[2].line, flow.edges[2].head),
        (Line::Dotted, Head::Arrow)
    );
}

#[test]
fn er_diagrams_label_relations_with_cardinalities() {
    let flow = flow_of("erDiagram\nCUSTOMER ||--o{ ORDER : places\nORDER {\n  int id PK \"the key\"\n  string note\n}\nORDER }|..|| PRODUCT : lists");
    assert_eq!(
        node(&flow, "ORDER").label,
        "ORDER\nint id PK\nstring note"
    );
    assert_eq!(
        (node(&flow, "ORDER").rule, node(&flow, "CUSTOMER").rule),
        (1, 0)
    );
    assert_eq!(flow.edges[0].label, "places\n1 : 0..*");
    assert_eq!(
        (flow.edges[1].label.as_str(), flow.edges[1].line),
        ("lists\n1..* : 1", Line::Dotted)
    );
}

#[test]
fn a_mindmap_is_its_tree() {
    let flow = flow_of("mindmap\n  root((Plan))\n    Build\n      Kernels\n    Test\n  ::icon(fa fa-book)");
    let labels: Vec<&str> = flow.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["Plan", "Build", "Kernels", "Test"]);
    assert_eq!(flow.nodes[0].shape, Shape::Circle);
    assert_eq!(flow.edges.len(), 3);
    // Left to right: each level further along x.
    assert!(flow.nodes[0].x < flow.nodes[1].x && flow.nodes[1].x < flow.nodes[2].x);
    assert_eq!(flow.nodes[1].x, flow.nodes[3].x);
}

#[test]
fn a_subgraph_boxes_its_members() {
    let flow =
        flow_of("flowchart TB\nsubgraph one [First group]\n  a1 --> a2\nend\nc --> a1");
    assert_eq!(flow.groups.len(), 1);
    assert_eq!(flow.groups[0].title, "First group");
    let (x0, y0, x1, y1) = flow.groups[0].rect;
    for id in ["a1", "a2"] {
        let n = node(&flow, id);
        assert!(
            n.x - n.w / 2.0 >= x0
                && n.x + n.w / 2.0 <= x1
                && n.y - n.h / 2.0 >= y0
                && n.y + n.h / 2.0 <= y1
        );
    }
    let c = node(&flow, "c");
    assert!(!(c.x >= x0 && c.x <= x1 && c.y >= y0 && c.y <= y1));
}

#[test]
fn a_state_diagram_becomes_a_flowchart_with_start_and_end() {
    let flow = flow_of("stateDiagram-v2\n[*] --> Still\nstate \"Moving fast\" as Moving\nStill --> Moving : push\nMoving --> Still\nMoving --> [*]");
    let ids: Vec<&str> = flow.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["[*]start1", "Still", "Moving", "[*]end1"]);
    assert_eq!(node(&flow, "Moving").label, "Moving fast");
    assert_eq!(flow.nodes[0].shape, Shape::Dot);
    assert_eq!(flow.edges[1].label, "push");
}

#[test]
fn a_sequence_has_lanes_messages_notes_and_frames() {
    let Diagram::Sequence(seq) = parse(
        "sequenceDiagram\nautonumber\nparticipant A as Alice\nactor B as Bob\nA->>B: Hello Bob\nB-->>A: Hi\nloop Every minute\n  A->>A: think\nend\nalt yes\n  A-xB: no\nelse no\n  Note over A,B: both\nend",
        &measure,
    ) else {
        panic!()
    };
    assert_eq!(
        seq.lanes
            .iter()
            .map(|l| (l.label.as_str(), l.actor))
            .collect::<Vec<_>>(),
        [("Alice", false), ("Bob", true)]
    );
    let kinds: Vec<String> = seq
        .steps
        .iter()
        .map(|s| match s {
            Step::Message { text, dashed, .. } => format!("message {text} {dashed}"),
            Step::Loop { text, .. } => format!("loop {text}"),
            Step::Note { text, .. } => format!("note {text}"),
            Step::Frame {
                kind,
                label,
                sections,
                ..
            } => format!("frame {kind} {label} {}", sections.len()),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "message 1. Hello Bob false",
            "message 2. Hi true",
            "frame loop Every minute 0",
            "loop 3. think",
            "frame alt yes 1",
            "message 4. no false",
            "note both"
        ]
    );
    let ys: Vec<f32> = seq
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::Message { y, .. } | Step::Loop { y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]));
    assert!(seq.lanes[1].x - seq.lanes[0].x >= 120.0);
}

#[test]
fn a_pie_has_slices_and_others_are_unsupported() {
    let Diagram::Pie(pie) = parse(
        "pie title Pets\n\"Dogs\" : 386\n\"Cats\" : 85.5\n\"None\" : 0",
        &measure,
    ) else {
        panic!()
    };
    assert_eq!(pie.title, "Pets");
    assert_eq!(
        pie.slices,
        [("Dogs".to_string(), 386.0), ("Cats".to_string(), 85.5)]
    );
    assert!(matches!(
        parse("quadrantChart\ntitle x", &measure),
        Diagram::Unsupported(kind) if kind == "quadrantChart"
    ));
}

#[test]
fn answers_are_cut_at_closed_mermaid_fences() {
    let text = "Here:\n\n```mermaid\ngraph TD\nA-->B\n```\nAfter.\n```mermaid\ngraph TD\nC";
    let parts = split(text);
    assert_eq!(
        parts,
        [
            (false, "Here:\n\n"),
            (true, "graph TD\nA-->B\n"),
            (false, "After.\n```mermaid\ngraph TD\nC")
        ]
    );
    assert_eq!(split("plain"), [(false, "plain")]);
}
