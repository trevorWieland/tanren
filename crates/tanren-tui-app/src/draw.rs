//! Rendering helpers for the TUI screen router. Split out of `main.rs`
//! to keep that file under the workspace 500-line budget.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{FormState, MenuChoice, OutcomeView, ProjectDetailState, ProjectListState};

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
    lines.push(Line::from("up/down select   Enter confirm   q/Esc quit"));
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
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Length(1));
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
        Paragraph::new("Tab/up/down move   Enter submit   Esc back to menu"),
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
    lines.push(Line::from("Enter/Esc continue   q back to menu"));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(crate) fn draw_project_list(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &ProjectListState,
) {
    let block = Block::default().borders(Borders::ALL).title(" Projects ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();
    if state.projects.is_empty() {
        lines.push(Line::from("No projects found."));
        if let Some(err) = state.error.as_deref() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Error: {err}"),
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
    } else {
        for (idx, project) in state.projects.iter().enumerate() {
            let marker = if idx == state.selected { "> " } else { "  " };
            let attention = if project.needs_attention { " [!]" } else { "" };
            let style = if idx == state.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let state_label = match project.state {
                tanren_contract::ProjectStateSummary::Active => "active",
                tanren_contract::ProjectStateSummary::Paused => "paused",
                tanren_contract::ProjectStateSummary::Completed => "done",
                tanren_contract::ProjectStateSummary::Archived => "archived",
            };
            lines.push(Line::from(Span::styled(
                format!("{marker}{} [{state_label}]{attention}", project.name),
                style,
            )));
            if project.needs_attention && idx == state.selected {
                for spec in &project.attention_specs {
                    lines.push(Line::from(format!("    ! {}", spec.name)));
                }
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("up/down select   Enter switch   Esc menu"));
    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}

pub(crate) fn draw_project_detail(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    state: &ProjectDetailState,
) {
    let title = format!(" {} ", state.project.name);
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();
    let state_label = match state.project.state {
        tanren_contract::ProjectStateSummary::Active => "active",
        tanren_contract::ProjectStateSummary::Paused => "paused",
        tanren_contract::ProjectStateSummary::Completed => "done",
        tanren_contract::ProjectStateSummary::Archived => "archived",
    };
    lines.push(Line::from(format!("State: {state_label}")));

    if !state.project.attention_specs.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Attention specs:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for (i, spec) in state.project.attention_specs.iter().enumerate() {
            let selected = state.selected == i;
            let style = if selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("  ! {}", spec.name),
                style,
            )));
            if state.detail_spec.as_ref().is_some_and(|s| s.id == spec.id) {
                lines.push(Line::from(format!("    Reason: {}", spec.reason)));
            }
        }
    }

    if let Some(scoped) = state.scoped.as_ref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Scoped views:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let offset = state.project.attention_specs.len();
        let spec_style = if state.selected == offset {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  Specs: {}", scoped.specs.len()),
            spec_style,
        )));
        let loop_style = if state.selected == offset + 1 {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  Loops: {}", scoped.loops.len()),
            loop_style,
        )));
        let ms_style = if state.selected == offset + 2 {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("  Milestones: {}", scoped.milestones.len()),
            ms_style,
        )));
    }

    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("up/down navigate   Enter detail   Esc back"));
    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}
