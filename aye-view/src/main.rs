use aye::{error::Error, reader::Reader};
use aye_view::{app::App, model::sanitize, view, watch::Watcher};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
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
        let _ = restore_terminal(io::stdout());
    }
}
fn restore_terminal(mut output: impl io::Write) -> io::Result<()> {
    execute!(
        output,
        DisableMouseCapture,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )
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
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    let mut redraw = true;
    while !app.quit {
        if let Some(update) = watcher.take_update() {
            app.apply_update(update);
            redraw = true;
        }
        redraw |= app.tick_clock();
        if redraw {
            terminal.draw(|frame| view::render(frame, &mut app))?;
            redraw = false;
        }
        if event::poll(Duration::from_millis(250))? {
            redraw = app.handle_event(event::read()?);
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
        let _ = restore_terminal(io::stdout());
        previous(info);
    }));
    if let Err(error) = run() {
        eprintln!("aye-view: {}", sanitize(&error.to_string()));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_normal_error_and_panic_cleanup_disables_all_mouse_modes() {
        let mut output = Vec::new();
        super::restore_terminal(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        for mode in [1000, 1002, 1003, 1006, 1015] {
            assert!(output.contains(&format!("\x1b[?{mode}l")), "{output:?}");
        }
        assert!(output.contains("\x1b[?1049l"));
        assert!(output.contains("\x1b[?25h"));
    }
}
