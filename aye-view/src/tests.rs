use crate::{app::App, view};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
const NOW: &str = "2026-09-17T03:10:00.000Z";
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn frame(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::render(f, app)).unwrap();
    let b = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < width {
                let symbol = b[(x, y)].symbol();
                row.push_str(symbol);
                x += ratatui::text::Span::raw(symbol).width().max(1) as u16;
            }
            row
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn app() -> App {
    let mut s = State::empty();
    let mut t = Task::new("Unicode 中文 👩‍💻 title\x1b[2J".into(), NOW);
    t.description = format!(
        "unsafe\x1b[2J\r text {}\nEND-OF-DETAIL",
        "long 中文 ".repeat(150)
    );
    t.acceptance = vec!["acceptance content".into()];
    s.tasks.insert(t.id.clone(), t);
    App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state: s,
    })
}
#[test]
fn actual_input_render_details_scroll_and_narrow() {
    let mut app = app();
    let wide = frame(&mut app, 110, 25);
    assert!(wide.contains("Unicode 中文"));
    assert!(wide.contains("Detail"));
    assert!(wide.contains("👩‍💻"));
    assert!(!wide.contains('\x1b'));
    let narrow = frame(&mut app, 35, 15);
    assert!(narrow.contains("List"));
    assert!(!narrow.contains("Description"));
    key(&mut app, KeyCode::Enter);
    let detail = frame(&mut app, 35, 15);
    assert!(detail.contains("Detail"));
    assert!(!detail.contains('\x1b'));
    let mut found = false;
    for _ in 0..100 {
        key(&mut app, KeyCode::PageDown);
        let rendered = frame(&mut app, 35, 15);
        assert!(!rendered.chars().any(|c| c.is_control() && c != '\n'));
        if rendered.contains("END-OF-DETAIL") {
            found = true;
            break;
        }
    }
    assert!(found, "long text tail must remain reachable");
    key(&mut app, KeyCode::Esc);
    assert!(frame(&mut app, 35, 15).contains("List"));
    key(&mut app, KeyCode::Char('?'));
    assert!(frame(&mut app, 80, 25).contains("Help"));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
}
#[test]
fn empty_and_tiny_render() {
    let mut app = App::new(ReaderSnapshot {
        oid: "b".repeat(40),
        state: State::empty(),
    });
    assert!(frame(&mut app, 80, 20).contains("aye create"));
    for (w, h) in [(1, 1), (10, 3), (0, 0)] {
        frame(&mut app, w, h);
    }
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(app.selected_id.is_none());
}
#[test]
fn current_visibility_and_reverse_relations() {
    let mut s = State::empty();
    let mut a = Task::new("ancestor".into(), NOW);
    a.status = "closed".into();
    a.resolution = Some("cancelled".into());
    a.closed_at = Some(NOW.into());
    let mut b = Task::new("dependent".into(), NOW);
    b.depends_on.push(a.id.clone());
    b.parent = Some(a.id.clone());
    b.discovered_from = Some(a.id.clone());
    let mut old = a.clone();
    old.id = Task::new("old".into(), NOW).id;
    for t in [&a, &b, &old] {
        s.tasks.insert(t.id.clone(), t.clone());
    }
    let mut app = App::new(ReaderSnapshot {
        oid: "c".repeat(40),
        state: s,
    });
    assert_eq!(app.visible_ids.len(), 2);
    assert!(!app.visible_ids.contains(&old.id));
    assert_eq!(app.snapshot.state.effective(&b), "blocked");
    app.select(&a.id);
    let text = frame(&mut app, 140, 40);
    assert!(text.contains("cancelled"));
    assert!(text.contains("Blocks"));
    assert!(text.contains("dependent"));
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit);
}

#[test]
fn list_navigation_changes_actual_detail_and_ctrl_h_returns() {
    let mut s = State::empty();
    let mut first = Task::new("FIRST TASK".into(), NOW);
    first.priority = "P0".into();
    let second = Task::new("SECOND TASK".into(), NOW);
    s.tasks.insert(first.id.clone(), first.clone());
    s.tasks.insert(second.id.clone(), second.clone());
    let mut app = App::new(ReaderSnapshot {
        oid: "d".repeat(40),
        state: s,
    });
    assert_eq!(app.selected_id.as_deref(), Some(first.id.as_str()));
    key(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected_id.as_deref(), Some(second.id.as_str()));
    key(&mut app, KeyCode::Enter);
    assert!(frame(&mut app, 40, 15).contains("SECOND TASK"));
    app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
    assert_eq!(app.pane, crate::app::Pane::Main);
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.pane, crate::app::Pane::Main);
    key(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_id.as_deref(), Some(first.id.as_str()));
}
