use aye::{error::Error, reader::Reader};
use aye_view::{app::App, model::sanitize, view, watch::Watcher};
use crossterm::{
    clipboard::CopyToClipboard,
    event::{self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture},
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
        DisableFocusChange,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )
}
fn native_clipboard_text(text: &str, local_macos: bool) -> bool {
    // pbcopy interprets RTF/PostScript headers instead of copying literal text.
    local_macos && !text.starts_with("{\\rtf") && !text.starts_with("%!")
}
fn copy_to_process(mut command: std::process::Command, text: &str) -> io::Result<()> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let written = child.stdin.take().unwrap().write_all(text.as_bytes());
    let status = child.wait()?;
    written?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("system clipboard command failed"))
    }
}
fn copy_selection(
    mut output: impl io::Write,
    text: &str,
    local_macos: bool,
) -> io::Result<&'static str> {
    if native_clipboard_text(text, local_macos) {
        let mut command = std::process::Command::new("/usr/bin/pbcopy");
        command.env("LC_ALL", "en_US.UTF-8");
        if copy_to_process(command, text).is_ok() {
            return Ok("Copied selection · Ctrl+C copies again · Esc Back");
        }
        execute!(output, CopyToClipboard::to_clipboard_from(text))?;
        return Ok("Copy sent to terminal (system clipboard unavailable)");
    }
    execute!(output, CopyToClipboard::to_clipboard_from(text))?;
    Ok("Copy sent to terminal · enable terminal clipboard access if needed")
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
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    let mut redraw = true;
    let local_macos = cfg!(target_os = "macos")
        && std::env::var_os("SSH_CONNECTION").is_none()
        && std::env::var_os("SSH_TTY").is_none();
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
            if let Some(text) = app.clipboard_requested.take() {
                app.copy_status = Some(match copy_selection(io::stdout(), &text, local_macos) {
                    Ok(status) => status.into(),
                    Err(error) => format!("Copy failed: {}", sanitize(&error.to_string())),
                });
                redraw = true;
            }
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
    fn terminal_clipboard_encodes_exact_utf8_and_newlines() {
        let mut output = Vec::new();
        let status = super::copy_selection(&mut output, "中文 👩‍💻 e\u{301}\n\nend", false).unwrap();
        assert_eq!(
            output,
            b"\x1b]52;c;5Lit5paHIPCfkanigI3wn5K7IGXMgQoKZW5k\x1b\\"
        );
        assert!(status.starts_with("Copy sent to terminal"));
    }
    #[test]
    fn native_copy_routes_only_local_plain_text_without_format_interpretation() {
        assert!(super::native_clipboard_text("中文 plain text", true));
        for text in ["{\\rtf1 sample}", "%!PS-Adobe-3.0", "normal"] {
            assert!(!super::native_clipboard_text(text, false));
        }
        for text in ["{\\rtf1 sample}", "%!PS-Adobe-3.0"] {
            assert!(!super::native_clipboard_text(text, true));
            let mut output = Vec::new();
            let status = super::copy_selection(&mut output, text, true).unwrap();
            assert!(output.starts_with(b"\x1b]52;c;"));
            assert!(status.starts_with("Copy sent to terminal"));
        }
    }
    #[cfg(unix)]
    #[test]
    fn clipboard_process_receives_exact_text_and_checks_failure() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test");
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join(format!("clipboard-input-{}", std::process::id()));
        let mut command = std::process::Command::new("/bin/sh");
        command
            .args(["-c", "cat > \"$1\"", "clipboard-test"])
            .arg(&file);
        let text = "中文 👩‍💻 e\u{301}\n\nend\n";
        super::copy_to_process(command, text).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), text.as_bytes());
        std::fs::remove_file(file).unwrap();
        assert!(
            super::copy_to_process(std::process::Command::new("/usr/bin/false"), "text").is_err()
        );
    }
    #[test]
    fn terminal_clipboard_write_failure_is_reported() {
        struct Failure;
        impl std::io::Write for Failure {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("clipboard output unavailable"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(super::copy_selection(Failure, "text", false).is_err());
    }
    #[test]
    fn shared_normal_error_and_panic_cleanup_disables_all_mouse_modes() {
        let mut output = Vec::new();
        super::restore_terminal(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        for mode in [1000, 1002, 1003, 1006, 1015] {
            assert!(output.contains(&format!("\x1b[?{mode}l")), "{output:?}");
        }
        assert!(output.contains("\x1b[?1004l"));
        assert!(output.contains("\x1b[?1049l"));
        assert!(output.contains("\x1b[?25h"));
    }
}
