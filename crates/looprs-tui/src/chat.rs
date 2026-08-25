//! Interactive chat TUI: a scrollback transcript pane over a single-line
//! input box. An alternate, opt-in mode to the REPL in looprs-cli.
//!
//! Unlike a naive implementation, this renders the assistant's response
//! live, inside the TUI frame, as it streams — by injecting a channel-
//! backed `UserOutput` port (`ChannelOutput`) into the `Agent` instead of
//! its default stdout adapter, then running each turn on a background
//! task while the render loop drains the channel and redraws.

use crate::output::{ChannelOutput, OutputEvent, apply_output_event};
use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt as _;
use looprs::{Agent, ChatMessage};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Restores the terminal (raw mode + alternate screen) on drop, so a
/// panic or early return anywhere in `run()` can't leave the user's
/// shell in a broken raw-mode alternate-screen state.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

fn is_quit_key(key_event: &crossterm::event::KeyEvent) -> bool {
    matches!(key_event.code, KeyCode::Esc)
        || (key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(KeyModifiers::CONTROL))
}

/// Outcome of a single key press against the input buffer, decoupled from
/// terminal I/O so it can be unit tested directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Continue,
    Submit,
    Quit,
}

/// Applies a key press to the input buffer, returning what the chat loop
/// should do next.
pub fn handle_input_key(key: KeyCode, buffer: &mut String) -> InputAction {
    match key {
        KeyCode::Char(c) => {
            buffer.push(c);
            InputAction::Continue
        }
        KeyCode::Backspace => {
            buffer.pop();
            InputAction::Continue
        }
        KeyCode::Enter => InputAction::Submit,
        KeyCode::Esc => InputAction::Quit,
        _ => InputAction::Continue,
    }
}

fn render_transcript(messages: &[ChatMessage], live_text: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for msg in messages {
        if msg.text.is_empty() {
            continue;
        }
        push_labelled(&mut lines, &msg.role, &msg.text);
    }
    if !live_text.is_empty() {
        push_labelled(&mut lines, "assistant", live_text);
    }
    lines
}

fn push_labelled(lines: &mut Vec<Line<'static>>, role: &str, text: &str) {
    let (label, color) = match role {
        "user" => ("you", Color::Cyan),
        "assistant" => ("looprs", Color::Green),
        other => (other, Color::Yellow),
    };
    lines.push(Line::from(Span::styled(
        format!("{label}:"),
        Style::default().fg(color),
    )));
    for text_line in text.lines() {
        lines.push(Line::from(text_line.to_string()));
    }
    lines.push(Line::from(""));
}

fn draw(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    transcript: &[ChatMessage],
    live_text: &str,
    input: &str,
    busy: bool,
) -> Result<()> {
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(frame.area());

        let title = if busy {
            "looprs chat (thinking...)"
        } else {
            "looprs chat"
        };
        let transcript_widget = Paragraph::new(render_transcript(transcript, live_text))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false });
        frame.render_widget(transcript_widget, chunks[0]);

        let input_title = if busy {
            "waiting for response..."
        } else {
            "message (Enter to send, Esc to quit)"
        };
        let input_widget =
            Paragraph::new(input).block(Block::default().borders(Borders::ALL).title(input_title));
        frame.render_widget(input_widget, chunks[1]);
    })?;
    Ok(())
}

type TurnResult = (Agent, Result<(), looprs::errors::AgentError>);

/// Run the chat loop against an already-bootstrapped agent. Blocks until
/// the user quits (Esc). Consumes the agent since streaming requires
/// swapping its output adapter for the lifetime of the session.
pub async fn run(agent: Agent) -> Result<()> {
    // Restore the terminal even on panic, before the default hook prints
    // its message — otherwise a raw-mode alternate-screen panic leaves
    // the shell unusable until the user reruns `reset`. Kept in an Arc so
    // the original hook can be reinstalled once this function returns.
    let previous_hook = std::sync::Arc::new(std::panic::take_hook());
    let hook_for_panic = std::sync::Arc::clone(&previous_hook);
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        hook_for_panic(info);
    }));

    let (tx, mut rx) = mpsc::unbounded_channel::<OutputEvent>();
    let mut agent_slot = Some(agent.with_output(Box::new(ChannelOutput(tx))));

    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut input = String::new();
    let mut live_text = String::new();
    let mut static_transcript: Vec<ChatMessage> = Vec::new();
    let mut turn_handle: Option<JoinHandle<TurnResult>> = None;
    let mut reader = EventStream::new();

    let result: Result<()> = loop {
        draw(
            &mut terminal,
            &static_transcript,
            &live_text,
            &input,
            turn_handle.is_some(),
        )?;

        tokio::select! {
            maybe_ev = reader.next() => {
                match maybe_ev {
                    Some(Ok(Event::Key(key_event))) if key_event.kind == KeyEventKind::Press => {
                        if is_quit_key(&key_event) {
                            if let Some(handle) = turn_handle.take() {
                                handle.abort();
                            }
                            break Ok(());
                        }
                        if turn_handle.is_some() {
                            // Busy: ignore other input until the turn completes.
                            continue;
                        }
                        match handle_input_key(key_event.code, &mut input) {
                            InputAction::Continue => {}
                            InputAction::Quit => break Ok(()),
                            InputAction::Submit => {
                                let message = input.trim().to_string();
                                input.clear();
                                if !message.is_empty() {
                                    let mut turn_agent =
                                        agent_slot.take().expect("agent present while idle");
                                    turn_agent.add_user_message(message);
                                    turn_handle = Some(tokio::spawn(async move {
                                        let result = turn_agent.run_turn_streaming().await;
                                        (turn_agent, result)
                                    }));
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => break Err(e.into()),
                    None => break Ok(()),
                }
            }
            Some(event) = rx.recv(), if turn_handle.is_some() => {
                apply_output_event(&mut live_text, event);
            }
            joined = async { turn_handle.as_mut().expect("polled only while Some").await },
                if turn_handle.is_some() =>
            {
                turn_handle = None;
                match joined {
                    Ok((returned_agent, turn_result)) => {
                        // Rebuild from the agent's own transcript first, then
                        // append a synthetic "error" entry on failure so it
                        // survives the live_text.clear() below and is still
                        // visible on the next draw (a failed turn otherwise
                        // rendered as silence: no assistant reply, no error).
                        static_transcript = returned_agent.transcript();
                        if let Err(e) = turn_result {
                            static_transcript.push(ChatMessage {
                                role: "error".to_string(),
                                text: e.to_string(),
                            });
                        }
                        live_text.clear();
                        agent_slot = Some(returned_agent);
                    }
                    Err(join_err) => break Err(join_err.into()),
                }
            }
        }
    };

    drop(_guard); // restore terminal before returning/printing any error
    std::panic::set_hook(Box::new(move |info| previous_hook(info)));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chars_append_to_buffer() {
        let mut buf = String::new();
        assert_eq!(
            handle_input_key(KeyCode::Char('h'), &mut buf),
            InputAction::Continue
        );
        assert_eq!(
            handle_input_key(KeyCode::Char('i'), &mut buf),
            InputAction::Continue
        );
        assert_eq!(buf, "hi");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut buf = String::from("hi");
        assert_eq!(
            handle_input_key(KeyCode::Backspace, &mut buf),
            InputAction::Continue
        );
        assert_eq!(buf, "h");
    }

    #[test]
    fn backspace_on_empty_buffer_is_noop() {
        let mut buf = String::new();
        assert_eq!(
            handle_input_key(KeyCode::Backspace, &mut buf),
            InputAction::Continue
        );
        assert_eq!(buf, "");
    }

    #[test]
    fn enter_submits() {
        let mut buf = String::from("hello");
        assert_eq!(
            handle_input_key(KeyCode::Enter, &mut buf),
            InputAction::Submit
        );
        assert_eq!(buf, "hello", "submit does not itself clear the buffer");
    }

    #[test]
    fn esc_quits() {
        let mut buf = String::new();
        assert_eq!(handle_input_key(KeyCode::Esc, &mut buf), InputAction::Quit);
    }

    #[test]
    fn unrelated_key_is_noop() {
        let mut buf = String::from("x");
        assert_eq!(
            handle_input_key(KeyCode::Left, &mut buf),
            InputAction::Continue
        );
        assert_eq!(buf, "x");
    }

    #[test]
    fn render_transcript_skips_empty_messages_and_appends_live_text() {
        let messages = vec![
            ChatMessage {
                role: "user".into(),
                text: "hi".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                text: String::new(),
            },
        ];
        let lines = render_transcript(&messages, "typing...");
        // "you:" + "hi" + blank, then live "looprs:" + "typing..." + blank = 6 lines.
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn render_transcript_omits_live_block_when_empty() {
        let messages = vec![ChatMessage {
            role: "user".into(),
            text: "hi".into(),
        }];
        let lines = render_transcript(&messages, "");
        assert_eq!(lines.len(), 3);
    }
}
