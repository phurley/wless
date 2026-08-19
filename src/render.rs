use crate::app::{AppState, InputMode};
use crate::search;
use crate::view::visible_rows;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

const HELP_TEXT: &str = "\
wless -- word-wrapping, auto-following pager

Navigation
  Up / k              scroll up one line
  Down / j / Enter     scroll down one line
  Space / f / PgDn    page down
  b / PgUp            page up
  Ctrl-D / Ctrl-U     half page down / up
  g / Home            go to top
  G / End             go to bottom

Search
  /                   search forward (regex)
  ?                   search backward (regex)
  Enter (empty)       repeat last pattern
  Up / Down           browse search history
  n / N               repeat search (same / opposite direction)
  Esc                 cancel search / clear highlight

Follow
  F                   jump to end and follow appended content

Auto-scroll (teleprompter)
  a                   toggle auto-scroll
  + / -               speed up / slow down
  Up / Down           while auto-scrolling: also nudges speed
                      down / up (in addition to moving a line)

Other
  Ctrl-L / r          force redraw
  h / H               toggle this help
  q                   quit

Press any key to close";

pub fn draw(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let [text_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    draw_text(frame, app, text_area);
    draw_status(frame, app, status_area, text_area);

    if matches!(app.input_mode, InputMode::Help) {
        draw_help(frame, area);
    }
}

fn draw_text(frame: &mut Frame, app: &AppState, area: Rect) {
    let rows = visible_rows(&app.document, app.anchor, area.width, area.height);
    let lines: Vec<Line> = rows
        .iter()
        .map(|r| {
            let text = app.document.line_text(r.line_idx);
            match &app.last_pattern {
                Some(re) => {
                    let matches = search::find_all_in_line(re, app.document.line_bytes(r.line_idx));
                    highlighted_line(&text, r.row.start, r.row.end, &matches)
                }
                None => Line::from(text[r.row.start..r.row.end].to_string()),
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
}

/// Round `idx` outward to the nearest valid char boundary of `text`, never
/// past `[lo, hi]`. Regex matches on raw bytes aren't guaranteed to land on
/// char boundaries (e.g. an empty pattern's zero-width matches occur at
/// every byte offset, including mid-character), so match ranges must be
/// snapped before they're used to slice the `&str` used for display.
fn floor_boundary(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_boundary(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

/// Build a `Line` for one visual row (`text[row_start..row_end]`), splitting
/// it into styled spans wherever a search match (byte range relative to the
/// full logical line) overlaps this row.
fn highlighted_line(
    text: &str,
    row_start: usize,
    row_end: usize,
    matches: &[std::ops::Range<usize>],
) -> Line<'static> {
    let match_style = Style::default().bg(Color::Yellow).fg(Color::Black);
    let mut spans = Vec::new();
    let mut pos = row_start;

    for m in matches {
        let seg_start = ceil_boundary(text, m.start.max(row_start));
        let seg_end = floor_boundary(text, m.end.min(row_end));
        if seg_start >= seg_end || seg_start < pos {
            continue;
        }
        if seg_start > pos {
            spans.push(Span::raw(text[pos..seg_start].to_string()));
        }
        spans.push(Span::styled(
            text[seg_start..seg_end].to_string(),
            match_style,
        ));
        pos = seg_end;
    }
    if pos < row_end {
        spans.push(Span::raw(text[pos..row_end].to_string()));
    }
    Line::from(spans)
}

fn draw_status(frame: &mut Frame, app: &AppState, area: Rect, text_area: Rect) {
    if let InputMode::Search(state) = &app.input_mode {
        let text = format!("{}{}", state.direction.prompt_char(), state.query);
        frame.render_widget(Paragraph::new(text), area);
        return;
    }

    let text = if let Some(msg) = &app.status_message {
        format!(" {}", msg)
    } else {
        let pct = if app.document.len() == 0 {
            100
        } else {
            let offset = crate::view::anchor_offset(&app.document, app.anchor);
            ((offset as f64 / app.document.len() as f64) * 100.0) as u64
        };
        let at_end =
            crate::view::is_at_bottom(&app.document, app.anchor, text_area.width, text_area.height);
        let pos = if at_end {
            "END".to_string()
        } else {
            format!("{pct}%")
        };
        let auto = if app.auto_scroll {
            let lines_per_sec = 1000.0 / app.auto_scroll_interval.as_millis().max(1) as f64;
            format!("  [AUTO {lines_per_sec:.1}/s]")
        } else {
            String::new()
        };
        format!(" {}  ({}){}", app.filename, pos, auto)
    };
    let style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let line = Line::from(Span::styled(text, style));
    frame.render_widget(Paragraph::new(line).style(style), area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let lines = HELP_TEXT.lines().count() as u16 + 2;
    let width = HELP_TEXT.lines().map(|l| l.len()).max().unwrap_or(0) as u16 + 4;
    let [popup] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::vertical([Constraint::Length(lines.min(area.height))])
        .flex(Flex::Center)
        .areas(popup);

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" wless help ")
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let text = Paragraph::new(HELP_TEXT)
        .block(block)
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(text, popup);
}
