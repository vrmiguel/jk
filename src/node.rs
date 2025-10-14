use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};
use serde_json::Number;

#[derive(Clone)]
pub struct DisplayLine {
    pub spans: Vec<Span<'static>>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Object {
        is_collapsed: bool,
        entries: Vec<(String, Node)>,
    },
    Array {
        is_collapsed: bool,
        items: Vec<Node>,
    },
    Primitive(Primitive),
}

#[derive(Debug, Clone)]
pub enum Primitive {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
}

impl Node {
    pub fn from_value(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Node::Primitive(Primitive::Null),
            serde_json::Value::Bool(b) => Node::Primitive(Primitive::Bool(b)),
            serde_json::Value::Number(n) => Node::Primitive(Primitive::Number(n)),
            serde_json::Value::String(s) => Node::Primitive(Primitive::String(s)),
            serde_json::Value::Array(arr) => Node::Array {
                is_collapsed: false,
                items: arr.into_iter().map(Node::from_value).collect(),
            },
            serde_json::Value::Object(map) => Node::Object {
                is_collapsed: false,
                entries: map
                    .into_iter()
                    .map(|(k, v)| (k, Node::from_value(v)))
                    .collect(),
            },
        }
    }

    fn is_collapsed(&self) -> bool {
        match self {
            Node::Array { is_collapsed, .. } => *is_collapsed,
            Node::Object { is_collapsed, .. } => *is_collapsed,
            Node::Primitive(_) => false,
        }
    }

    fn toggle_collapse(&mut self) {
        match self {
            Node::Array { is_collapsed, .. } => *is_collapsed = !*is_collapsed,
            Node::Object { is_collapsed, .. } => *is_collapsed = !*is_collapsed,
            Node::Primitive(_) => {}
        }
    }

    pub fn render_lines(&self) -> Vec<DisplayLine> {
        let mut lines = Vec::new();
        let indent = 0;

        match self {
            Node::Object { is_collapsed, .. } => {
                if *is_collapsed {
                    lines.push(DisplayLine {
                        spans: vec![
                            Span::styled("{", Style::default().fg(Color::Gray)),
                            Span::styled(" ... ", Style::default().fg(Color::DarkGray)),
                            Span::styled("}", Style::default().fg(Color::Gray)),
                        ],
                    });
                } else {
                    lines.push(DisplayLine {
                        spans: vec![Span::styled("{", Style::default().fg(Color::Gray))],
                    });
                    lines.extend(self.render_contents(indent + 1));
                    lines.push(DisplayLine {
                        spans: vec![Span::styled("}", Style::default().fg(Color::Gray))],
                    });
                }
            }
            Node::Array { is_collapsed, .. } => {
                if *is_collapsed {
                    lines.push(DisplayLine {
                        spans: vec![
                            Span::styled("[", Style::default().fg(Color::Gray)),
                            Span::styled(" ... ", Style::default().fg(Color::DarkGray)),
                            Span::styled("]", Style::default().fg(Color::Gray)),
                        ],
                    });
                } else {
                    lines.push(DisplayLine {
                        spans: vec![Span::styled("[", Style::default().fg(Color::Gray))],
                    });
                    lines.extend(self.render_contents(indent + 1));
                    lines.push(DisplayLine {
                        spans: vec![Span::styled("]", Style::default().fg(Color::Gray))],
                    });
                }
            }
            Node::Primitive(prim) => {
                lines.push(DisplayLine {
                    spans: format_primitive(prim),
                });
            }
        }

        lines
    }

    fn render_contents(&self, indent: usize) -> Vec<DisplayLine> {
        let mut lines = Vec::new();
        let indent_str = "  ".repeat(indent);

        match self {
            Node::Object { entries, .. } => {
                for (i, (key, child)) in entries.iter().enumerate() {
                    let is_last = i == entries.len() - 1;
                    match child {
                        Node::Object { is_collapsed, .. } | Node::Array { is_collapsed, .. } => {
                            let (open_br, close_br) = match child {
                                Node::Object { .. } => ("{", "}"),
                                Node::Array { .. } => ("[", "]"),
                                _ => unreachable!(),
                            };

                            if *is_collapsed {
                                let mut spans = vec![
                                    Span::raw(format!("{}", indent_str)),
                                    Span::styled(
                                        format!("\"{}\"", key),
                                        Style::default()
                                            .fg(Color::Cyan)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::styled(": ", Style::default().fg(Color::Gray)),
                                    Span::styled(open_br, Style::default().fg(Color::Gray)),
                                    Span::styled(" ... ", Style::default().fg(Color::DarkGray)),
                                    Span::styled(close_br, Style::default().fg(Color::Gray)),
                                ];
                                if !is_last {
                                    spans.push(Span::styled(",", Style::default().fg(Color::Gray)));
                                }
                                lines.push(DisplayLine { spans });
                            } else {
                                lines.push(DisplayLine {
                                    spans: vec![
                                        Span::raw(format!("{}", indent_str)),
                                        Span::styled(
                                            format!("\"{}\"", key),
                                            Style::default()
                                                .fg(Color::Cyan)
                                                .add_modifier(Modifier::BOLD),
                                        ),
                                        Span::styled(": ", Style::default().fg(Color::Gray)),
                                        Span::styled(open_br, Style::default().fg(Color::Gray)),
                                    ],
                                });
                                lines.extend(child.render_contents(indent + 1));
                                let mut close_spans = vec![
                                    Span::raw(format!("{}", indent_str)),
                                    Span::styled(close_br, Style::default().fg(Color::Gray)),
                                ];
                                if !is_last {
                                    close_spans
                                        .push(Span::styled(",", Style::default().fg(Color::Gray)));
                                }
                                lines.push(DisplayLine { spans: close_spans });
                            }
                        }
                        Node::Primitive(prim) => {
                            let mut spans = vec![
                                Span::raw(format!("{}", indent_str)),
                                Span::styled(
                                    format!("\"{}\"", key),
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(": ", Style::default().fg(Color::Gray)),
                            ];
                            spans.extend(format_primitive(prim));
                            if !is_last {
                                spans.push(Span::styled(",", Style::default().fg(Color::Gray)));
                            }
                            lines.push(DisplayLine { spans });
                        }
                    }
                }
            }
            Node::Array { items, .. } => {
                for (i, child) in items.iter().enumerate() {
                    let is_last = i == items.len() - 1;
                    match child {
                        Node::Object { is_collapsed, .. } | Node::Array { is_collapsed, .. } => {
                            let (open_br, close_br) = match child {
                                Node::Object { .. } => ("{", "}"),
                                Node::Array { .. } => ("[", "]"),
                                _ => unreachable!(),
                            };

                            if *is_collapsed {
                                let mut spans = vec![
                                    Span::raw(format!("{}", indent_str)),
                                    Span::styled(open_br, Style::default().fg(Color::Gray)),
                                    Span::styled(" ... ", Style::default().fg(Color::DarkGray)),
                                    Span::styled(close_br, Style::default().fg(Color::Gray)),
                                ];
                                if !is_last {
                                    spans.push(Span::styled(",", Style::default().fg(Color::Gray)));
                                }
                                lines.push(DisplayLine { spans });
                            } else {
                                lines.push(DisplayLine {
                                    spans: vec![
                                        Span::raw(format!("{}", indent_str)),
                                        Span::styled(open_br, Style::default().fg(Color::Gray)),
                                    ],
                                });
                                lines.extend(child.render_contents(indent + 1));
                                let mut close_spans = vec![
                                    Span::raw(format!("{}", indent_str)),
                                    Span::styled(close_br, Style::default().fg(Color::Gray)),
                                ];
                                if !is_last {
                                    close_spans
                                        .push(Span::styled(",", Style::default().fg(Color::Gray)));
                                }
                                lines.push(DisplayLine { spans: close_spans });
                            }
                        }
                        Node::Primitive(prim) => {
                            let mut spans = vec![Span::raw(format!("{}", indent_str))];
                            spans.extend(format_primitive(prim));
                            if !is_last {
                                spans.push(Span::styled(",", Style::default().fg(Color::Gray)));
                            }
                            lines.push(DisplayLine { spans });
                        }
                    }
                }
            }
            Node::Primitive(_) => {}
        }

        lines
    }

    pub fn toggle_at_line(&mut self, target_line: usize, current_line: &mut usize) -> bool {
        self.apply_at_line(target_line, current_line, |node| {
            node.toggle_collapse();
        })
    }

    /// Finds the node at the target line and applies the action to it
    fn apply_at_line<F>(&mut self, target_line: usize, current_line: &mut usize, action: F) -> bool
    where
        F: Fn(&mut Node) + Copy,
    {
        if *current_line == target_line {
            action(self);
            return true;
        }
        *current_line += 1;

        if self.is_collapsed() {
            return false;
        }

        // Traverse children
        match self {
            Node::Object { entries, .. } => {
                for (_, child) in entries.iter_mut() {
                    if child.apply_at_line(target_line, current_line, action) {
                        return true;
                    }
                }
                *current_line += 1; // Closing bracket
            }
            Node::Array { items, .. } => {
                for child in items.iter_mut() {
                    if child.apply_at_line(target_line, current_line, action) {
                        return true;
                    }
                }
                *current_line += 1; // Closing bracket
            }
            Node::Primitive(_) => {}
        }

        false
    }

    pub fn collapse_at_line_if_expanded(
        &mut self,
        target_line: usize,
        current_line: &mut usize,
    ) -> bool {
        self.apply_at_line(target_line, current_line, |node| {
            if !node.is_collapsed() {
                node.toggle_collapse();
            }
        })
    }
}

fn format_primitive(prim: &Primitive) -> Vec<Span<'static>> {
    match prim {
        Primitive::String(s) => vec![Span::styled(
            format!("\"{}\"", s),
            Style::default().fg(Color::Green),
        )],
        Primitive::Number(n) => vec![Span::styled(
            n.to_string(),
            Style::default().fg(Color::Yellow),
        )],
        Primitive::Bool(b) => vec![Span::styled(
            b.to_string(),
            Style::default().fg(Color::Magenta),
        )],
        Primitive::Null => vec![Span::styled("null", Style::default().fg(Color::Red))],
    }
}
