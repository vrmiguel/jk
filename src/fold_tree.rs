use std::{fmt, mem, ops::Range};

use crate::Value;

#[derive(Debug, Clone, Copy)]
pub struct JsonLine<'a> {
    pub key: Option<&'a str>,
    pub value: &'a Value<'a>,
}

#[derive(Debug)]
pub struct FoldableJsonViewTree<'a> {
    root: Node<'a>,
}

impl<'a> FoldableJsonViewTree<'a> {
    pub fn new(value: &'a Value) -> Self {
        FoldableJsonViewTree {
            root: Node::new(None, value, 0),
        }
    }

    pub fn display_rows(&self, range: Range<usize>) -> Vec<DisplayRow<'a>> {
        let mut rows = Vec::new();
        self.root.display_rows(0, range, &mut rows, 0);
        rows
    }

    #[cfg(test)]
    pub fn to_string(&self, range: Range<usize>) -> String {
        use std::fmt::Write as _;

        let mut string = String::new();
        for row in self.display_rows(range) {
            let _ = write!(string, "{row}");
        }
        string
    }

    pub fn collapse(&mut self, index: usize) {
        self.root
            .update_is_collapsed(index, CollapseCommand::Collapse);
    }

    #[allow(dead_code)]
    pub fn expand(&mut self, index: usize) {
        self.root
            .update_is_collapsed(index, CollapseCommand::Expand);
    }

    pub fn toggle(&mut self, index: usize) {
        self.root
            .update_is_collapsed(index, CollapseCommand::Toggle);
    }

    pub fn root_length(&self) -> usize {
        self.root.length
    }
}

/// A node in this tree can represent multiple lines, check [`NodeKind`].
#[derive(Debug, Clone)]
pub struct Node<'a> {
    length: usize,
    original_range: Range<usize>,
    kind: NodeKind<'a>,
}

impl<'a> Node<'a> {
    fn new(key: Option<&'a str>, value: &'a Value, line_offset: usize) -> Self {
        let collapsable_node = match value {
            Value::Array(array) => {
                let iter = array.iter().map(|v| (None, v));
                Some(Self::new_from_contents(iter, line_offset + 1))
            }
            Value::Object(object) => {
                let iter = object.iter().map(|(k, v)| (Some(*k), v));
                Some(Self::new_from_contents(iter, line_offset + 1))
            }
            _ => None,
        };

        match collapsable_node {
            Some(inner_contents) => {
                let contents_size = inner_contents.as_ref().map_or(0, |node| node.length);
                let original_range = line_offset..line_offset + contents_size + 2; // add 2 for the opening and closing lines
                Self {
                    length: original_range.len(),
                    original_range,
                    kind: NodeKind::Collapsible {
                        is_collapsed: false,
                        nested_contents: inner_contents.map(Box::new),
                        line: JsonLine { key, value },
                    },
                }
            }
            None => Self {
                length: 1,
                original_range: line_offset..line_offset + 1,
                kind: NodeKind::NonCollapsible {
                    lines: Vec::from([JsonLine { key, value }]),
                },
            },
        }
    }

    fn new_from_contents<I>(values_iter: I, offset: usize) -> Option<Self>
    where
        I: Iterator<Item = (Option<&'a str>, &'a Value<'a>)> + 'a,
    {
        let mut values_iter = values_iter.peekable();
        values_iter.peek()?; // early return

        let mut values = Vec::<Self>::new();
        let mut next_element_offset = offset;

        for (key, value) in values_iter {
            let node = Node::new(key, value, next_element_offset);
            next_element_offset += node.length;

            // merge any two consecutive NonCollapsable regions
            if let Some(last) = values.last_mut()
                && let NodeKind::NonCollapsible {
                    lines: last_node_lines,
                } = &mut last.kind
                && let NodeKind::NonCollapsible { lines: new_lines } = node.kind
            {
                last.length += node.length;
                last.original_range.end = node.original_range.end;
                last_node_lines.extend(new_lines);
            } else {
                values.push(node);
            }
        }

        node_array_into_tree(values)
    }

    fn update_is_collapsed(
        &mut self,
        target_remaining_offset: usize,
        command: CollapseCommand,
    ) -> Option<CollapseLineDiff> {
        let collapse_line_diff = match &mut self.kind {
            NodeKind::NonCollapsible { .. } => None,
            NodeKind::Collapsible {
                is_collapsed,
                nested_contents: contents,
                line: _,
            } => {
                // Check if current node is the target one
                if target_remaining_offset == 0 {
                    let saved_length = self.length as isize;

                    let diff = match (command, *is_collapsed) {
                        (CollapseCommand::Collapse, _) | (CollapseCommand::Toggle, false) => {
                            *is_collapsed = true;
                            1 - saved_length // collapsed element has length 1 now
                        }
                        (CollapseCommand::Expand, _) | (CollapseCommand::Toggle, true) => {
                            *is_collapsed = false;
                            // length = 1 (opening) + contents.length (current, accounting for collapsed children) + 1 (closing)
                            let new_length = contents.as_ref().map_or(2, |c| c.length + 2);
                            new_length as isize - saved_length
                        }
                    };

                    if diff == 0 {
                        None // skip updating parents
                    } else {
                        Some(CollapseLineDiff(diff))
                    }
                } else if let Some(contents) = contents
                    && !*is_collapsed
                {
                    // Call recursively to next node till we find the target
                    //
                    // Arithmetic Safety:
                    //   We just checked that offset > 0
                    //
                    // Hopefully this is tail-call optimized and doesn't overflow the process stack for JSONs with a lot of nestedness
                    contents.update_is_collapsed(target_remaining_offset - 1, command) // pass the call recursively to its children, '-1' cause we moving down
                } else {
                    None
                }
            }
            NodeKind::SubTree { left, right } => {
                if target_remaining_offset < left.length {
                    left.update_is_collapsed(target_remaining_offset, command)
                } else if target_remaining_offset <= left.length + right.length {
                    right.update_is_collapsed(target_remaining_offset - left.length, command)
                } else {
                    None
                }
            }
        };

        if let Some(CollapseLineDiff(diff)) = collapse_line_diff {
            self.length = (self.length as isize + diff) as usize;
        }

        collapse_line_diff
    }

    fn display_rows(
        &self,
        current_offset: usize,
        range: Range<usize>,
        rows: &mut Vec<DisplayRow<'a>>,
        depth: usize,
    ) {
        if current_offset >= range.end {
            return;
        }

        match &self.kind {
            NodeKind::NonCollapsible { lines } => {
                let end_idx = range.end.saturating_sub(current_offset).min(lines.len());
                let start_idx = range.start.saturating_sub(current_offset).min(end_idx);

                // Iterate through the intersection of NonCollapsible lines with the target range
                for &line in &lines[start_idx..end_idx] {
                    rows.push(DisplayRow {
                        depth,
                        kind: DisplayRowKind::Element {
                            line,
                            is_collapsed: false,
                        },
                    });
                }
            }
            NodeKind::Collapsible {
                line,
                is_collapsed,
                nested_contents: contents,
            } => {
                if range.contains(&current_offset) {
                    rows.push(DisplayRow {
                        depth,
                        kind: DisplayRowKind::Element {
                            line: *line,
                            is_collapsed: *is_collapsed,
                        },
                    });
                }

                if !is_collapsed {
                    if let Some(contents) = contents {
                        contents.display_rows(current_offset + 1, range.clone(), rows, depth + 1);
                    }
                    // Add the closing brace/bracket if it's in the current viewport's range
                    let closing_offset = current_offset + self.length - 1;
                    if range.contains(&closing_offset) {
                        rows.push(DisplayRow {
                            depth,
                            kind: DisplayRowKind::ClosingSymbol {
                                symbol: closing_symbol_of_collapsable_element(line.value),
                            },
                        });
                    }
                }
            }
            NodeKind::SubTree { left, right } => {
                left.display_rows(current_offset, range.clone(), rows, depth);
                right.display_rows(current_offset + left.length, range, rows, depth);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeKind<'a> {
    /// One or multiple lines of non-collapsable json elements.
    NonCollapsible { lines: Vec<JsonLine<'a>> },
    /// A single collapsable element (array or object) and it's contents.
    Collapsible {
        line: JsonLine<'a>,
        is_collapsed: bool,
        nested_contents: Option<Box<Node<'a>>>,
    },
    /// A sequence of nodes that can be searched in logarithmic time.
    ///
    /// Can only appear as the `nested_contents` field of a NodeKind::Collapsible.
    SubTree {
        left: Box<Node<'a>>,
        right: Box<Node<'a>>,
    },
}

#[allow(dead_code)]
impl NodeKind<'_> {
    pub fn is_collapsable(&self) -> bool {
        matches!(self, NodeKind::Collapsible { .. })
    }

    pub fn is_path(&self) -> bool {
        matches!(self, NodeKind::SubTree { .. })
    }

    pub fn is_non_collapsable(&self) -> bool {
        matches!(self, NodeKind::NonCollapsible { .. })
    }
}

#[derive(Debug)]
pub struct DisplayRow<'a> {
    pub depth: usize,
    pub kind: DisplayRowKind<'a>,
}

#[derive(Debug)]
pub enum DisplayRowKind<'a> {
    Element {
        line: JsonLine<'a>,
        is_collapsed: bool,
    },
    ClosingSymbol {
        symbol: char,
    },
}

impl fmt::Display for DisplayRow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        '_lol_: {
            let indent = self.depth * 2;
            const INDENT: &str = unsafe { <str>::from_utf8_unchecked(&[b' '; 64]) };

            let full = indent / INDENT.len();
            let rem = indent % INDENT.len();

            for _ in 0..full {
                write!(f, "{INDENT}")?;
            }
            if rem != 0 {
                write!(f, "{}", &INDENT[..rem])?;
            }
        }

        match &self.kind {
            DisplayRowKind::ClosingSymbol { symbol } => {
                writeln!(f, "{symbol}")?;
            }
            DisplayRowKind::Element { line, is_collapsed } => {
                if let Some(key) = line.key {
                    write!(f, "\"{}\": ", key)?;
                }

                match line.value {
                    Value::Null => writeln!(f, "null")?,
                    Value::Bool(boo) => writeln!(f, "{boo}")?,
                    Value::Number(n) => writeln!(f, "{n}")?,
                    Value::String(s) => writeln!(f, "\"{s}\"")?,
                    Value::Array(_) => {
                        writeln!(f, "[{}", if *is_collapsed { " ]" } else { "" })?;
                    }
                    Value::Object(_) => {
                        writeln!(f, "{{{}", if *is_collapsed { " }" } else { "" })?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn node_array_into_tree(mut nodes: Vec<Node>) -> Option<Node> {
    assert!(!nodes.is_empty());

    while nodes.len() > 1 {
        let mut taken = mem::take(&mut nodes).into_iter();

        nodes = Vec::new();

        while let Some(left) = taken.next() {
            let Some(right) = taken.next() else {
                nodes.push(left);
                break;
            };
            let original_range = left.original_range.start..right.original_range.end;
            nodes.push(Node {
                length: original_range.len(),
                original_range,
                kind: NodeKind::SubTree {
                    left: Box::new(left),
                    right: Box::new(right),
                },
            });
        }
    }

    nodes.pop()
}

fn closing_symbol_of_collapsable_element(value: &Value) -> char {
    match value {
        Value::Array(_) => ']',
        Value::Object(_) => '}',
        _ => unreachable!(),
    }
}

enum CollapseCommand {
    Collapse,
    Expand,
    Toggle,
}

/// How many lines changed after collapsing or expanding a node.
///
/// Positive indicates that length has increased, and negative indicates that it decreased.
struct CollapseLineDiff(isize);

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::borrowed_value;

    #[test]
    fn test_fold_tree() {
        let json_str = r#"{
            "hobbies": [
                [
                    "reading",
                    "cycling"
                ],
                [
                    "swimming",
                    "dancing"
                ]
            ]
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();

        let mut tree = FoldableJsonViewTree::new(&json);

        tree.collapse(2);
        let expected = indoc! {r#"{
            "hobbies": [
              [ ]
              [
                "swimming"
                "dancing"
              ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.expand(1); // no-op
        tree.collapse(2); // no-op
        assert_eq!(expected, tree.to_string(0..20));

        tree.collapse(3);
        let expected = indoc! {r#"{
            "hobbies": [
              [ ]
              [ ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.collapse(1);
        let expected = indoc! {r#"{
            "hobbies": [ ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.expand(1);
        let expected = indoc! {r#"{
            "hobbies": [
              [ ]
              [ ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.expand(2);
        let expected = indoc! {r#"{
            "hobbies": [
              [
                "reading"
                "cycling"
              ]
              [ ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.collapse(0);
        let expected = "{ }\n";
        assert_eq!(expected, tree.to_string(0..20));

        for i in 1..10 {
            tree.expand(i); // no-op
        }
        assert_eq!(expected, tree.to_string(0..20));

        tree.expand(0);
        let expected = indoc! {r#"{
            "hobbies": [
              [
                "reading"
                "cycling"
              ]
              [ ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));

        tree.expand(6);
        let expected = indoc! {r#"{
            "hobbies": [
              [
                "reading"
                "cycling"
              ]
              [
                "swimming"
                "dancing"
              ]
            ]
          }
        "#};
        assert_eq!(expected, tree.to_string(0..20));
    }

    fn assert_root_length_matches_display_rows(tree: &FoldableJsonViewTree) {
        let actual_rows = tree.display_rows(0..usize::MAX);
        let root_length = tree.root_length();
        assert_eq!(
            root_length,
            actual_rows.len(),
            "root.length ({}) should equal display_rows count ({})",
            root_length,
            actual_rows.len()
        );
    }

    #[test]
    fn test_root_length_invariant_simple() {
        let json_str = r#"{
            "name": "Alice",
            "age": 30,
            "items": [1, 2, 3]
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();
        let mut tree = FoldableJsonViewTree::new(&json);

        // Initial state: fully expanded
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);

        // collapse the array
        tree.collapse(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 5);

        // expand it back
        tree.expand(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);

        // collapse the root object
        tree.collapse(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 1);

        // expand the root object
        tree.expand(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);
    }

    #[test]
    fn test_root_length_invariant_nested() {
        let json_str = r#"{
            "hobbies": [
                [
                    "reading",
                    "cycling"
                ],
                [
                    "swimming",
                    "dancing"
                ]
            ]
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();
        let mut tree = FoldableJsonViewTree::new(&json);

        // Initial state: fully expanded
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 12);

        // collapse first inner array
        tree.collapse(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);

        // collapse second inner array
        tree.collapse(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 6);

        // collapse outer array
        tree.collapse(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 3);

        // expand outer array (inner arrays still collapsed)
        tree.expand(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 6);

        // expand first inner array
        tree.expand(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);

        // collapse root (before expanding second array)
        tree.collapse(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 1);

        // expand root (first inner array still expanded, second still collapsed)
        tree.expand(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 9);

        // Now expand second inner array (which is at index 6 after first array expanded)
        tree.expand(6);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 12);
    }

    #[test]
    fn test_root_length_invariant_toggle() {
        let json_str = r#"{
            "data": {
                "nested": {
                    "value": 42
                }
            }
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();
        let mut tree = FoldableJsonViewTree::new(&json);

        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 7);

        // toggle the most nested object
        tree.toggle(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 5);

        // toggle it back
        tree.toggle(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 7);

        // toggle middle object
        tree.toggle(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 3);

        // toggle root
        tree.toggle(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 1);

        // toggle root back
        tree.toggle(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 3);

        // toggle middle back
        tree.toggle(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 7);
    }

    #[test]
    fn test_root_length_invariant() {
        let json_str = r#"{
            "users": [
                {
                    "name": "Alice",
                    "hobbies": ["reading", "cycling"]
                },
                {
                    "name": "Bob",
                    "hobbies": ["swimming"]
                }
            ],
            "count": 2
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();
        let mut tree = FoldableJsonViewTree::new(&json);

        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);

        // collapse first user's hobbies array (no change, already at name/hobbies level which is merged)
        tree.collapse(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);

        // collapse first user object
        tree.collapse(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 12);

        // collapse second user's hobbies array (no change - user object already collapsed)
        tree.collapse(4); // Index shifts after previous collapse
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 12);

        // collapse users array
        tree.collapse(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 4);

        // expand users array
        tree.expand(1);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 12);

        // expand first user object
        tree.expand(2);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);

        // multiple toggles
        tree.toggle(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);
        tree.toggle(3);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);
        tree.toggle(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 1);
        tree.toggle(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 18);
    }

    #[test]
    fn test_root_length_with_non_collapsible_values() {
        let json_str = r#"{
            "string": "hello",
            "number": 42,
            "bool": true,
            "null": null,
            "array": []
        }"#;
        let json = borrowed_value::parse_value(json_str).unwrap();
        let mut tree = FoldableJsonViewTree::new(&json);

        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 8);

        // collapse empty array
        tree.collapse(5);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 7);

        // collapse root
        tree.collapse(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 1);

        // expand root (array still collapsed)
        tree.expand(0);
        assert_root_length_matches_display_rows(&tree);
        assert_eq!(tree.root_length(), 7);
    }
}
