use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::node::{DisplayLine, Node};

mod node;

fn main() {
    let example = "{
        \"name\": \"John\",
        \"age\": 30,
        \"city\": \"New York\",
        \"hobbies\": [\"reading\", \"cycling\"],
        \"address\": {
            \"street\": \"123 Main St\",
            \"zip\": \"10001\"
        }
    }";

    let serde_value: serde_json::Value = serde_json::from_str(example).expect("Invalid JSON");
    let root = Node::from_value(serde_value);
    let mut state = State::new(root);

    let mut terminal = ratatui::init();
    loop {
        terminal
            .draw(|frame| draw(frame, &state))
            .expect("failed to draw frame");

        if let Event::Key(key) = event::read().expect("failed to read event") {
            if handle_key_event(&mut state, key) {
                break;
            }
        }
    }
    ratatui::restore();
}

struct State {
    root: Node,
    cursor: usize,
}

impl State {
    fn new(root: Node) -> Self {
        Self { root, cursor: 0 }
    }

    fn get_visible_lines(&self) -> Vec<DisplayLine> {
        self.root.render_lines(0)
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

    fn collapse_current(&mut self) {
        let mut line_counter = 0;
        self.root
            .collapse_at_line_if_expanded(self.cursor, &mut line_counter);
    }
}

fn handle_key_event(state: &mut State, key: KeyEvent) -> bool {
    let num_lines = state.get_visible_lines().len();

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return true,
        KeyCode::Up | KeyCode::Char('k') => state.move_cursor_up(),
        KeyCode::Down | KeyCode::Char('j') => state.move_cursor_down(num_lines),
        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
            state.toggle_current()
        }
        KeyCode::Left | KeyCode::Char('h') => {
            state.collapse_current();
        }
        _ => {}
    }
    false
}

fn draw(frame: &mut Frame, state: &State) {
    let display_lines = state.get_visible_lines();
    let mut lines: Vec<Line> = Vec::new();

    for (i, display_line) in display_lines.into_iter().enumerate() {
        let mut line_spans = display_line.spans;

        if i == state.cursor {
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

    let title = " ↑↓/jk: navigate, Enter/Space/→/l: expand, ←/h: collapse, q/Esc: quit";
    let paragraph =
        Paragraph::new(lines).block(Block::default().title(title).borders(Borders::ALL));

    frame.render_widget(paragraph, frame.area());
}
