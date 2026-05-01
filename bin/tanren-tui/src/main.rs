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
    let result = run(&mut terminal);
    teardown_terminal(&mut terminal).context("teardown terminal")?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).context("enter alternate screen")?;
    let backend = CrosstermBackend::new(out);
    Terminal::new(backend).context("construct terminal")
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
