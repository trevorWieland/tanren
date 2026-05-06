//! Rendering helpers for the TUI screen router. Split out of `main.rs`
//! to keep that file under the workspace 500-line budget.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use tanren_contract::PreserveReason;

use crate::{FormState, MenuChoice, OutcomeView, UninstallPreviewState};

pub(crate) fn draw_menu(frame: &mut ratatui::Frame<'_>, area: Rect, selected: usize) {
    let mut lines = vec![
        Line::from("Tanren TUI"),
        Line::from(""),
        Line::from("Choose an action:"),
        Line::from(""),
    ];
    for (idx, choice) in MenuChoice::ALL.iter().enumerate() {
        let marker = if idx == selected { "> " } else { "  " };
        let style = if idx == selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}", choice.label()),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("↑/↓ select   Enter confirm   q/Esc quit"));
    let block = Block::default().borders(Borders::ALL).title(" tanren-tui ");
    let para = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(block);
    frame.render_widget(para, area);
}

pub(crate) fn draw_form(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    title: &str,
    state: &FormState,
) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let mut constraints: Vec<Constraint> =
        state.fields.iter().map(|_| Constraint::Length(2)).collect();
    constraints.push(Constraint::Length(1)); // hint line
    constraints.push(Constraint::Length(1)); // error line
    constraints.push(Constraint::Min(0));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (idx, field) in state.fields.iter().enumerate() {
        let focused = idx == state.focus;
        let marker = if focused { ">" } else { " " };
        let display = if field.secret {
            "*".repeat(field.value.chars().count())
        } else {
            field.value.clone()
        };
        let cursor = if focused { "_" } else { "" };
        let style = if focused {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let line = Line::from(Span::styled(
            format!("{marker} {}: {display}{cursor}", field.label),
            style,
        ));
        frame.render_widget(Paragraph::new(line), chunks[idx]);
    }
    let hint_idx = state.fields.len();
    frame.render_widget(
        Paragraph::new("Tab/↑↓ move   Enter submit   Esc back to menu"),
        chunks[hint_idx],
    );
    let error_idx = hint_idx + 1;
    if let Some(message) = state.error.as_deref() {
        let style = Style::default().add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Span::styled(message.to_owned(), style)).wrap(Wrap { trim: true }),
            chunks[error_idx],
        );
    }
}

pub(crate) fn draw_outcome(frame: &mut ratatui::Frame<'_>, area: Rect, view: &OutcomeView) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", view.title));
    let mut lines: Vec<Line<'_>> = view.lines.iter().map(|s| Line::from(s.as_str())).collect();
    lines.push(Line::from(""));
    lines.push(Line::from("Enter/Esc back to menu   q quit-to-menu"));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn preserve_reason_label(reason: PreserveReason) -> &'static str {
    match reason {
        PreserveReason::UserOwned => "user-owned",
        PreserveReason::ModifiedSinceInstall => "modified since install",
        PreserveReason::AlreadyRemoved => "already removed",
    }
}

pub(crate) fn draw_uninstall_preview(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &UninstallPreviewState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Uninstall preview ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    lines.push(Line::from(Span::styled(
        "Files to remove:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if state.preview.to_remove.is_empty() {
        lines.push(Line::from("  (none)"));
    }
    for path in &state.preview.to_remove {
        lines.push(Line::from(format!("  - {path}")));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Files to preserve:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if state.preview.preserved.is_empty() {
        lines.push(Line::from("  (none)"));
    }
    for file in &state.preview.preserved {
        let reason = preserve_reason_label(file.reason);
        lines.push(Line::from(format!("  - {} ({})", file.path, reason)));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Manifest: {}",
        state.preview.manifest_path
    )));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Note: hosted account/project history is NOT changed.",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    if let Some(err) = &state.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            err.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Enter confirm uninstall   Esc cancel"));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}
