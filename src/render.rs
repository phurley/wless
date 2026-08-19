use crate::app::AppState;
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
            Line::from(slice.to_string())
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
}

fn draw_status(frame: &mut Frame, app: &AppState, area: Rect, text_area: Rect) {
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
    let text = format!(" {}  ({})", app.filename, pos);
    let style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let line = Line::from(Span::styled(text, style));
    frame.render_widget(Paragraph::new(line).style(style), area);
}
