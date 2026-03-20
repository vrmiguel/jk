use jk::fold_tree::{DisplayRow, DisplayRowKind, FoldableJsonViewTree, JsonElement};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};

pub struct JsonTreeViewState {
    pub cursor: usize,
    pub scroll_offset: usize,
    pub viewport_height: usize,
}

impl JsonTreeViewState {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            scroll_offset: 0,
            viewport_height: 0,
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_down(&mut self, num_lines: usize) {
        if self.cursor + 1 < num_lines {
            self.cursor += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(self.viewport_height);
    }

    pub fn page_down(&mut self, num_lines: usize) {
        self.cursor = (self.cursor + self.viewport_height).min(num_lines.saturating_sub(1));
    }

    pub fn adjust_scroll(&mut self) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = self.cursor - self.viewport_height + 1;
        }
    }
}

pub struct JsonTreeView<'a> {
    tree: &'a FoldableJsonViewTree<'a>,
    title: &'a str,
}

impl<'a> JsonTreeView<'a> {
    pub fn new(tree: &'a FoldableJsonViewTree<'a>, title: &'a str) -> Self {
        Self { tree, title }
    }
}

impl StatefulWidget for JsonTreeView<'_> {
    type State = JsonTreeViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let viewport_height = area.height.saturating_sub(2) as usize;
        state.viewport_height = viewport_height;

        let start = state.scroll_offset;
        let end = start + viewport_height;

        let display_rows = self.tree.display_rows(start..end + 1);

        let lines: Vec<Line> = display_rows
            .iter()
            .take(viewport_height)
            .enumerate()
            .map(|(i, display_row)| {
                let next_row = display_rows.get(i + 1);
                let needs_comma = next_row.is_some_and(|n| n.depth == display_row.depth);
                let mut spans = render_display_row(display_row, needs_comma);
                apply_cursor_style(&mut spans, start + i == state.cursor);
                Line::from(spans)
            })
            .collect();

        let paragraph =
            Paragraph::new(lines).block(Block::default().title(self.title).borders(Borders::ALL));
        paragraph.render(area, buf);
    }
}

fn apply_cursor_style(spans: &mut Vec<Span<'static>>, is_cursor: bool) {
    if is_cursor {
        spans.insert(
            0,
            Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        for span in spans.iter_mut() {
            span.style = span.style.bg(Color::DarkGray);
        }
    } else {
        spans.insert(0, Span::raw("  "));
    }
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

            match line.inner {
                JsonElement::Null => {
                    spans.push(Span::styled("null", Style::default().fg(Color::Red)));
                }
                JsonElement::Bool(b) => {
                    spans.push(Span::styled(
                        b.to_string(),
                        Style::default().fg(Color::Magenta),
                    ));
                }
                JsonElement::Number(n) => {
                    spans.push(Span::styled(
                        n.to_string(),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                JsonElement::String(s) => {
                    spans.push(Span::styled(
                        format!("\"{}\"", s),
                        Style::default().fg(Color::Green),
                    ));
                }
                JsonElement::Array(_) => {
                    spans.push(Span::styled("[", Style::default().fg(Color::Gray)));
                    if *is_collapsed {
                        spans.push(Span::styled(" ... ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled("]", Style::default().fg(Color::Gray)));
                    }
                }
                JsonElement::Object(_) => {
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
