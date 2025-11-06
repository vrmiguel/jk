use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::node::{DisplayLine, Node};

pub fn start_viewer(root: Node) -> anyhow::Result<()> {
    let mut ctx = Ctx::new(root);
    ctx.build_visible_lines();

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

struct Ctx {
    root: Node,
    cursor: usize,
    visible_lines: Vec<DisplayLine>,
    scroll_offset: usize,
    /// Viewport height as per the last frame rendered
    viewport_height: usize,
}

impl Ctx {
    fn new(root: Node) -> Self {
        Self {
            root,
            cursor: 0,
            visible_lines: Vec::new(),
            scroll_offset: 0,
            viewport_height: 0,
        }
    }

    fn build_visible_lines(&mut self) {
        self.visible_lines = self.root.render_lines();
    }

    fn get_visible_lines(&self) -> &[DisplayLine] {
        &self.visible_lines
    }

    fn toggle_current(&mut self) {
        let mut line_counter = 0;
        self.root.toggle_at_line(self.cursor, &mut line_counter);
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
        let mut line_counter = 0;
        self.root
            .collapse_at_line_if_expanded(self.cursor, &mut line_counter);
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        let num_lines = self.get_visible_lines().len();
        let mut dirty = false;

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor_down(num_lines),
            KeyCode::PageUp => self.page_up(self.viewport_height),
            KeyCode::PageDown => self.page_down(num_lines, self.viewport_height),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                self.toggle_current();
                dirty = true;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // TODO(vini): drop this, keep only toggling?
                self.collapse_current();
                dirty = true;
            }
            _ => {}
        }

        if dirty {
            self.build_visible_lines();
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
    let display_lines = ctx.get_visible_lines();

    let start = ctx.scroll_offset;
    let end = (start + ctx.viewport_height).min(display_lines.len());

    let mut lines: Vec<Line> = Vec::new();

    for (i, display_line) in display_lines[start..end].iter().enumerate() {
        let actual_line_index = start + i;
        let mut line_spans = display_line.spans.clone();

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
