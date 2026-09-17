use aye::{error::Error, reader::Reader};
use aye_view::{app::App, model::sanitize, view, watch::Watcher};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let reader = Reader::open(std::env::current_dir()?)?;
    let oid = reader
        .current_oid()?
        .ok_or_else(|| Error::new("NOT_INITIALIZED", "No aye task state found. Run aye init."))?;
    let mut app = App::new(reader.load(&oid)?);
    let watcher = Watcher::start(reader, app.snapshot.oid.clone())?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("aye-view requires an interactive terminal".into());
    }
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    while !app.quit {
        if let Some(update) = watcher.take_update() {
            app.apply_update(update);
        }
        terminal.draw(|frame| view::render(frame, &mut app))?;
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key);
            if app.refresh_requested {
                app.refresh_requested = false;
                watcher.refresh();
            }
        }
    }
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "aye-view — read-only local aye task explorer\n\nUsage: aye-view\n\nRun inside a Git repository with aye initialized. Press ? for keys."
        );
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("aye-view {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if !args.is_empty() {
        eprintln!("Usage: aye-view (see --help)");
        std::process::exit(2);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        previous(info);
    }));
    if let Err(error) = run() {
        eprintln!("aye-view: {}", sanitize(&error.to_string()));
        std::process::exit(1);
    }
}
