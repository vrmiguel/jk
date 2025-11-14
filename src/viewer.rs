use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use serde_json::Value;

use crate::fold_tree::{DisplayRow, DisplayRowKind, FoldableJsonViewTree};

pub fn start_viewer(json: &Value) -> anyhow::Result<()> {
    let mut ctx = Ctx::new(&json);

    let mut terminal = ratatui::init();
    loop {
        terminal
            .draw(|frame| {
                let viewport_height = viewport_height(frame.area());
                ctx.viewport_height = viewport_height;
                draw(frame, &ctx);
            })
            .expect("failed to draw frame");

        if let Event::Key(key) = event::read().expect("failed to read event")
            && ctx.handle_key_event(key)
        {
            break;
        }
    }
    ratatui::restore();
    Ok(())
}

struct Ctx<'a> {
    tree: FoldableJsonViewTree<'a>,
    cursor: usize,
    scroll_offset: usize,
    /// Viewport height as per the last frame rendered
    viewport_height: usize,
}

impl<'a> Ctx<'a> {
    fn new(json: &'a Value) -> Self {
        let tree = FoldableJsonViewTree::new(json);

        Self {
            tree,
            cursor: 0,
            scroll_offset: 0,
            viewport_height: 0,
        }
    }

    fn total_lines(&self) -> usize {
        self.tree.display_rows(0..usize::MAX).len()
    }

    fn toggle_current(&mut self) {
        self.tree.toggle(self.cursor);
    }

    fn move_cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_cursor_down(&mut self, num_lines: usize) {
        if self.cursor + 1 < num_lines {
            self.cursor += 1;
        }
    }

    fn page_up(&mut self, page_size: usize) {
        self.cursor = self.cursor.saturating_sub(page_size);
    }

    fn page_down(&mut self, num_lines: usize, page_size: usize) {
        self.cursor = (self.cursor + page_size).min(num_lines.saturating_sub(1));
    }

    /// Clamps the cursor to the viewport
    fn adjust_scroll(&mut self) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.cursor - self.viewport_height + 1;
        }
    }

    fn collapse_current(&mut self) {
        self.tree.collapse(self.cursor);
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        let num_lines = self.total_lines();

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor_down(num_lines),
            KeyCode::PageUp => self.page_up(self.viewport_height),
            KeyCode::PageDown => self.page_down(num_lines, self.viewport_height),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                self.toggle_current();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.collapse_current();
            }
            _ => {}
        }

        self.adjust_scroll();

        false
    }
}

fn viewport_height(area: Rect) -> usize {
    // subtracts 2px, one for each border for the top and bottom
    area.height.saturating_sub(2) as usize
}

fn draw(frame: &mut Frame, ctx: &Ctx) {
    let start = ctx.scroll_offset;
    let end = start + ctx.viewport_height;

    let display_rows = ctx.tree.display_rows(
        // One more than we'll actually render, so we can check the next row for whether we should add a comma or not
        start..end + 1,
    );

    let mut lines: Vec<Line> = Vec::new();

    for (i, display_row) in display_rows.iter().take(ctx.viewport_height).enumerate() {
        let actual_line_index = start + i;
        let next_row = display_rows.get(i + 1);
        let needs_comma = should_add_comma(display_row, next_row);

        let mut line_spans = render_display_row(display_row, needs_comma);

        if actual_line_index == ctx.cursor {
            line_spans.insert(
                0,
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            );

            line_spans = line_spans
                .into_iter()
                .map(|span| Span::styled(span.content, span.style.bg(Color::DarkGray)))
                .collect();
        } else {
            line_spans.insert(0, Span::raw("  "));
        }

        lines.push(Line::from(line_spans));
    }

    let title =
        " ↑↓/jk: navigate, PgUp/PgDn: page, Enter/Space/→/l: expand, ←/h: collapse, q/Esc: quit";

    let paragraph =
        Paragraph::new(lines).block(Block::default().title(title).borders(Borders::ALL));

    frame.render_widget(paragraph, frame.area());
}

fn should_add_comma(current: &DisplayRow, next: Option<&DisplayRow>) -> bool {
    next.map_or(false, |n| n.depth == current.depth)
}

fn render_display_row(row: &DisplayRow, needs_comma: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    let indent = "  ".repeat(row.depth);
    if !indent.is_empty() {
        spans.push(Span::raw(indent));
    }

    match &row.kind {
        DisplayRowKind::ClosingSymbol { symbol } => {
            spans.push(Span::styled(
                symbol.to_string(),
                Style::default().fg(Color::Gray),
            ));
        }
        DisplayRowKind::Element { line, is_collapsed } => {
            if let Some(key) = line.key {
                spans.push(Span::styled(
                    format!("\"{}\"", key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(": ", Style::default().fg(Color::Gray)));
            }

            match line.value {
                Value::Null => {
                    spans.push(Span::styled("null", Style::default().fg(Color::Red)));
                }
                Value::Bool(b) => {
                    spans.push(Span::styled(
                        b.to_string(),
                        Style::default().fg(Color::Magenta),
                    ));
                }
                Value::Number(n) => {
                    spans.push(Span::styled(
                        n.to_string(),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                Value::String(s) => {
                    spans.push(Span::styled(
                        format!("\"{}\"", s),
                        Style::default().fg(Color::Green),
                    ));
                }
                Value::Array(_) => {
                    spans.push(Span::styled("[", Style::default().fg(Color::Gray)));
                    if *is_collapsed {
                        spans.push(Span::styled(" ... ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled("]", Style::default().fg(Color::Gray)));
                    }
                }
                Value::Object(_) => {
                    spans.push(Span::styled("{", Style::default().fg(Color::Gray)));
                    if *is_collapsed {
                        spans.push(Span::styled(" ... ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled("}", Style::default().fg(Color::Gray)));
                    }
                }
            }
        }
    }

    if needs_comma {
        spans.push(Span::styled(",", Style::default().fg(Color::Gray)));
    }

    spans
}
