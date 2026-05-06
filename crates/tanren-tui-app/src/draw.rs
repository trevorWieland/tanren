//! Rendering helpers for the TUI screen router. Split out of `main.rs`
//! to keep that file under the workspace 500-line budget.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{FormState, MenuChoice, OutcomeView, SubmenuKind};

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

pub(crate) fn draw_submenu(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    kind: SubmenuKind,
    selected: usize,
) {
    let mut lines = vec![
        Line::from(kind.title()),
        Line::from(""),
        Line::from("Choose an action:"),
        Line::from(""),
    ];
    let count = kind.choice_count();
    for idx in 0..count {
        let marker = if idx == selected { "> " } else { "  " };
        let style = if idx == selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}", kind.choice_label(idx)),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("↑/↓ select   Enter confirm   Esc back"));
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", kind.title()));
    let para = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(block);
    frame.render_widget(para, area);
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
