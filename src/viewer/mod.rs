mod json_tree_view;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use jk::fold_tree::{FoldableJsonViewTree, KeyedJsonElement};
use ratatui::Frame;

use self::json_tree_view::{JsonTreeView, JsonTreeViewState};

const HELP_TEXT: &str =
    " ↑↓/jk: navigate, PgUp/PgDn: page, Enter/Space/→/l: expand, ←/h: collapse, q/Esc: quit";

pub fn start_viewer(json: &KeyedJsonElement) -> anyhow::Result<()> {
    let mut app = App::new(json);

    let mut terminal = ratatui::init();
    loop {
        terminal
            .draw(|frame| draw(frame, &mut app))
            .expect("failed to draw frame");

        if let Event::Key(key) = event::read().expect("failed to read event")
            && app.handle_key_event(key)
        {
            break;
        }
    }
    ratatui::restore();
    Ok(())
}

struct App<'a> {
    tree: FoldableJsonViewTree<'a>,
    view_state: JsonTreeViewState,
}

impl<'a> App<'a> {
    fn new(root_element: &'a KeyedJsonElement<'a>) -> Self {
        Self {
            tree: FoldableJsonViewTree::new(root_element),
            view_state: JsonTreeViewState::new(),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        let num_lines = self.tree.root_length();

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Up | KeyCode::Char('k') => self.view_state.move_cursor_up(),
            KeyCode::Down | KeyCode::Char('j') => self.view_state.move_cursor_down(num_lines),
            KeyCode::PageUp => self.view_state.page_up(),
            KeyCode::PageDown => self.view_state.page_down(num_lines),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                self.tree.toggle(self.view_state.cursor);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.tree.collapse(self.view_state.cursor);
            }
            _ => {}
        }

        self.view_state.adjust_scroll();

        false
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let tree_view = JsonTreeView::new(&app.tree, HELP_TEXT);
    frame.render_stateful_widget(tree_view, frame.area(), &mut app.view_state);
}
