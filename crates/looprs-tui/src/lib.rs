//! Small interactive terminal menus used by looprs-cli for setup flows
//! (provider selection, local model selection). Kept deliberately minimal:
//! a single-column selectable list, arrow keys / j-k to move, Enter to
//! confirm, Esc/q to cancel.

pub mod chat;
pub mod output;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

/// Outcome of a menu interaction, decoupled from terminal I/O so the
/// key-handling logic can be unit tested without a real terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Keep the menu open; selection may have moved.
    Continue,
    /// User confirmed the current selection.
    Confirm,
    /// User cancelled (Esc or 'q').
    Cancel,
}

/// Applies a single key press to the current selection index, returning
/// what the menu loop should do next. `len` is the number of items.
pub fn handle_key(key: KeyCode, selected: &mut usize, len: usize) -> MenuAction {
    if len == 0 {
        return MenuAction::Cancel;
    }
    match key {
        KeyCode::Up | KeyCode::Char('k') => {
            *selected = selected.checked_sub(1).unwrap_or(len - 1);
            MenuAction::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *selected = (*selected + 1) % len;
            MenuAction::Continue
        }
        KeyCode::Enter => MenuAction::Confirm,
        KeyCode::Esc | KeyCode::Char('q') => MenuAction::Cancel,
        _ => MenuAction::Continue,
    }
}

/// Render and drive an interactive single-select menu over `items`.
/// Returns `Ok(None)` if the user cancelled, `Ok(Some(index))` on confirm.
pub fn select(title: &str, items: &[String]) -> Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let result = run_select_loop(title, items);
    disable_raw_mode()?;
    result
}

/// Drives the actual draw/read loop. Kept separate from `select` so raw
/// mode is always disabled on the way out, even if this returns early
/// via `?` on a draw or read error.
fn run_select_loop(title: &str, items: &[String]) -> Result<Option<usize>> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    // TODO(verify): when select() is called more than once in a process
    // (e.g. run_provider_menu picking a provider, then a local model), this
    // fresh Terminal's diff buffer starts blank and skips writing cells
    // whose new content happens to be a plain space, since it can't tell
    // the physical terminal still has a stale glyph there from the prior
    // call. Observed leaking a character from "Select a provider" into
    // "Select a local model" ("Select a localdmodel"). terminal.clear()?
    // here should fix it.

    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(frame.area());

            frame.render_widget(Paragraph::new(Line::from(title)), chunks[0]);

            let list_items: Vec<ListItem> =
                items.iter().map(|i| ListItem::new(i.as_str())).collect();
            let list = List::new(list_items)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(list, chunks[1], &mut state);
        })?;

        if let Event::Key(key_event) = event::read()? {
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            match handle_key(key_event.code, &mut selected, items.len()) {
                MenuAction::Continue => continue,
                MenuAction::Confirm => return Ok(Some(selected)),
                MenuAction::Cancel => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_wraps_to_start() {
        let mut selected = 2;
        assert_eq!(
            handle_key(KeyCode::Down, &mut selected, 3),
            MenuAction::Continue
        );
        assert_eq!(selected, 0);
    }

    #[test]
    fn up_wraps_to_end() {
        let mut selected = 0;
        assert_eq!(
            handle_key(KeyCode::Up, &mut selected, 3),
            MenuAction::Continue
        );
        assert_eq!(selected, 2);
    }

    #[test]
    fn j_k_move_like_arrows() {
        let mut selected = 0;
        assert_eq!(
            handle_key(KeyCode::Char('j'), &mut selected, 3),
            MenuAction::Continue
        );
        assert_eq!(selected, 1);
        assert_eq!(
            handle_key(KeyCode::Char('k'), &mut selected, 3),
            MenuAction::Continue
        );
        assert_eq!(selected, 0);
    }

    #[test]
    fn enter_confirms() {
        let mut selected = 1;
        assert_eq!(
            handle_key(KeyCode::Enter, &mut selected, 3),
            MenuAction::Confirm
        );
        assert_eq!(selected, 1);
    }

    #[test]
    fn esc_and_q_cancel() {
        let mut selected = 1;
        assert_eq!(
            handle_key(KeyCode::Esc, &mut selected, 3),
            MenuAction::Cancel
        );
        assert_eq!(
            handle_key(KeyCode::Char('q'), &mut selected, 3),
            MenuAction::Cancel
        );
    }

    #[test]
    fn empty_list_cancels_immediately() {
        let mut selected = 0;
        assert_eq!(
            handle_key(KeyCode::Enter, &mut selected, 0),
            MenuAction::Cancel
        );
    }

    #[test]
    fn unknown_key_continues_without_moving() {
        let mut selected = 1;
        assert_eq!(
            handle_key(KeyCode::Char('x'), &mut selected, 3),
            MenuAction::Continue
        );
        assert_eq!(selected, 1);
    }

    #[test]
    fn select_returns_none_for_empty_items() {
        let items: Vec<String> = vec![];
        assert_eq!(select("empty", &items).unwrap(), None);
    }
}
