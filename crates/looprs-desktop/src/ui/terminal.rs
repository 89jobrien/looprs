use freya::prelude::*;
use freya::terminal::*;

pub fn terminal_screen() -> Element {
    Element::from(TerminalScreen)
}

#[derive(Clone, Copy, PartialEq)]
pub struct TerminalScreen;

impl Component for TerminalScreen {
    fn render(&self) -> impl IntoElement {
        let focus = use_focus();
        let terminal_handle = use_state(|| {
            let mut cmd = CommandBuilder::new("bash");
            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");
            TerminalHandle::new(TerminalId::new(), cmd, None).ok()
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .vertical()
            .spacing(8.0)
            .child(label().text("Terminal"))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .background((10, 10, 10))
                    .padding(Gaps::new_all(6.0))
                    .child(if let Some(handle) = terminal_handle.read().clone() {
                        let handle_for_events = handle.clone();
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .a11y_id(focus.a11y_id())
                            .on_mouse_down(move |_| focus.request_focus())
                            .on_key_down(move |e: Event<KeyboardEventData>| {
                                e.stop_propagation();
                                if e.modifiers.contains(Modifiers::CONTROL)
                                    && matches!(&e.key, Key::Character(ch) if ch.len() == 1)
                                {
                                    if let Key::Character(ch) = &e.key {
                                        let _ = handle_for_events.write(&[ch.as_bytes()[0] & 0x1f]);
                                    }
                                    return;
                                }

                                if let Some(ch) = e.try_as_str() {
                                    let _ = handle_for_events.write(ch.as_bytes());
                                    return;
                                }

                                let bytes: &[u8] = match &e.key {
                                    Key::Named(NamedKey::Enter) => b"\r",
                                    Key::Named(NamedKey::Backspace) => &[0x7f],
                                    Key::Named(NamedKey::Delete) => b"\x1b[3~",
                                    Key::Named(NamedKey::Tab) => b"\t",
                                    Key::Named(NamedKey::Escape) => &[0x1b],
                                    Key::Named(NamedKey::ArrowUp) => b"\x1b[A",
                                    Key::Named(NamedKey::ArrowDown) => b"\x1b[B",
                                    Key::Named(NamedKey::ArrowLeft) => b"\x1b[D",
                                    Key::Named(NamedKey::ArrowRight) => b"\x1b[C",
                                    _ => return,
                                };

                                let _ = handle_for_events.write(bytes);
                            })
                            .child(Terminal::new(handle))
                            .into_element()
                    } else {
                        label().text("Terminal exited").into_element()
                    }),
            )
    }
}
