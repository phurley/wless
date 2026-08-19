use crate::app::{AppState, InputMode};
use crate::search;
use crate::view::visible_rows;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub fn draw(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let [text_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    draw_text(frame, app, text_area);
    draw_status(frame, app, status_area, text_area);
}

fn draw_text(frame: &mut Frame, app: &AppState, area: Rect) {
    let rows = visible_rows(&app.document, app.anchor, area.width, area.height);
    let lines: Vec<Line> = rows
        .iter()
        .map(|r| {
            let text = app.document.line_text(r.line_idx);
            let slice = &text[r.row.start..r.row.end];
            match &app.last_pattern {
                Some(re) => {
                    let matches = search::find_all_in_line(re, app.document.line_bytes(r.line_idx));
                    highlighted_line(slice, r.row.start, &matches)
                }
                None => Line::from(slice.to_string()),
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
}

/// Build a `Line` for one visual row, splitting it into styled spans
/// wherever a search match (byte range relative to the full logical line)
/// overlaps this row's byte range (`row_start..row_start + slice.len()`).
fn highlighted_line(
    slice: &str,
    row_start: usize,
    matches: &[std::ops::Range<usize>],
) -> Line<'static> {
    let row_end = row_start + slice.len();
    let match_style = Style::default().bg(Color::Yellow).fg(Color::Black);
    let mut spans = Vec::new();
    let mut pos = row_start;

    for m in matches {
        if m.end <= row_start || m.start >= row_end {
            continue;
        }
        let seg_start = m.start.max(row_start);
        let seg_end = m.end.min(row_end);
        if seg_start > pos {
            spans.push(Span::raw(
                slice[pos - row_start..seg_start - row_start].to_string(),
            ));
        }
        spans.push(Span::styled(
            slice[seg_start - row_start..seg_end - row_start].to_string(),
            match_style,
        ));
        pos = seg_end;
    }
    if pos < row_end {
        spans.push(Span::raw(slice[pos - row_start..].to_string()));
    }
    Line::from(spans)
}

fn draw_status(frame: &mut Frame, app: &AppState, area: Rect, text_area: Rect) {
    if let InputMode::Search { direction, query } = &app.input_mode {
        let text = format!("{}{}", direction.prompt_char(), query);
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
        format!(" {}  ({})", app.filename, pos)
    };
    let style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let line = Line::from(Span::styled(text, style));
    frame.render_widget(Paragraph::new(line).style(style), area);
}
