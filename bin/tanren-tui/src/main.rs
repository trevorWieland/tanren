//! Tanren terminal UI.
//!
//! F-0001 ships an empty buildable shell: enters raw-mode + alternate-screen,
//! renders one placeholder frame, waits for `q` to quit, then restores the
//! terminal cleanly. Live observation panels arrive with R-* slices that
//! consume read models from `tanren-app-services`.

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Alignment;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io::{Stdout, stdout};
use std::time::Duration;
use tanren_app_services::Handlers;

fn main() -> Result<()> {
    let mut terminal = setup_terminal().context("setup terminal")?;
    let run_result = run(&mut terminal);
    let teardown_result = teardown_terminal(&mut terminal).context("teardown terminal");
    // The run error is the more interesting one for the user; surface it
    // first. If only teardown failed, surface that.
    run_result.and(teardown_result)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    // Each step that mutates terminal state must roll back on failure of any
    // later step in setup; otherwise the user is left in raw / alternate
    // mode with no way to recover. Build the terminal incrementally and
    // restore on the first failure.
    enable_raw_mode().context("enable raw mode")?;
    let mut out = stdout();
    if let Err(err) = execute!(out, EnterAlternateScreen).context("enter alternate screen") {
        let _ = disable_raw_mode();
        return Err(err);
    }
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend).context("construct terminal") {
        Ok(terminal) => Ok(terminal),
        Err(err) => {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(err)
        }
    }
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leave alternate screen")?;
    terminal.show_cursor().context("show cursor")?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    let placeholder = format!(
        "Tanren TUI — placeholder shell\n\nstatus={}  version={}  contract_version={}\n\npress q to quit",
        report.status,
        report.version,
        report.contract_version.value(),
    );
    loop {
        terminal
            .draw(|frame| {
                let block = Block::default().borders(Borders::ALL).title(" tanren-tui ");
                let para = Paragraph::new(placeholder.as_str())
                    .alignment(Alignment::Center)
                    .block(block);
                frame.render_widget(para, frame.area());
            })
            .context("render frame")?;

        if event::poll(Duration::from_millis(200)).context("poll terminal events")?
            && let Event::Key(key) = event::read().context("read terminal event")?
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}
