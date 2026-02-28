use crate::services::agent_adapter::run_turn_for_prompt;
use crate::services::generative_ui::{
    GenerativeUiCommand, GenerativeUiUpdate, LiveGenerativeUiHandle, start_live_generative_ui,
};
use crate::services::mockstation::{MockstationRuntime, build_mockstation_runtime};
use crate::services::sqlite_store::{
    append_chat_message, append_observability_event, clear_chat_messages, load_chat_messages,
};
use crate::ui::context_demo::context_demo_screen;
use crate::ui::editor::editor_screen;
use crate::ui::terminal::terminal_screen;
use freya::prelude::*;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    AiChat,
    Editor,
    Terminal,
    GenerativeUi,
    Mockstation,
    ContextDemo,
}

fn action_button(
    bg: (u8, u8, u8),
    text: &'static str,
    on_press: impl Into<EventHandler<Event<PressEventData>>>,
) -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new_all(6.0))
        .background(bg)
        .center()
        .on_press(on_press)
        .child(label().text(text))
        .into_element()
}

pub fn app() -> impl IntoElement {
    let screen = use_state(
        || match std::env::var("LOOPRS_DESKTOP_START_SCREEN").ok().as_deref() {
            Some("main") | Some("menu") | Some("mainmenu") | Some("MainMenu") => Screen::MainMenu,
            Some("chat") | Some("ai") | Some("aichat") | Some("AiChat") => Screen::AiChat,
            Some("editor") | Some("Editor") => Screen::Editor,
            Some("terminal") | Some("Terminal") => Screen::Terminal,
            Some("genui") | Some("generativeui") | Some("GenerativeUi") => Screen::GenerativeUi,
            Some("mock") | Some("mockstation") | Some("Mockstation") => Screen::Mockstation,
            Some("context") | Some("contextdemo") | Some("ContextDemo") => Screen::ContextDemo,
            _ => Screen::MainMenu,
        },
    );

    let chat_status = use_state(|| "Ready".to_string());
    let chat_is_running = use_state(|| false);
    let chat_input = use_state(String::new);
    let chat_history = use_state(Vec::<ChatMessage>::new);
    let chat_history_loaded = use_state(|| false);

    let mockstation = use_state(|| Option::<MockstationRuntime>::None);
    let genui_handle = use_state(|| Option::<LiveGenerativeUiHandle>::None);
    let genui_update = use_state(|| Arc::new(GenerativeUiUpdate::default()));

    {
        let screen_state = screen;
        let mut chat_history_loaded_state = chat_history_loaded;
        let mut chat_history_state = chat_history;
        use_side_effect(move || {
            if *screen_state.read() != Screen::AiChat {
                return;
            }

            if *chat_history_loaded_state.read() {
                return;
            }

            chat_history_loaded_state.set(true);
            spawn(async move {
                let persisted = load_chat_messages(500).await;
                let hydrated = persisted
                    .into_iter()
                    .map(|message| ChatMessage {
                        role: message.role,
                        content: message.content,
                    })
                    .collect::<Vec<_>>();
                chat_history_state.set(hydrated);
            });
        });
    }

    {
        let screen_state = screen;
        let mut genui_handle_state = genui_handle;
        let mut genui_update_state = genui_update;
        use_side_effect(move || {
            if *screen_state.read() != Screen::GenerativeUi {
                return;
            }

            if genui_handle_state.read().is_some() {
                return;
            }

            let handle = start_live_generative_ui(Duration::from_secs(3));
            genui_handle_state.set(Some(handle.clone()));

            let mut updates_rx = handle.updates.clone();
            spawn(async move {
                loop {
                    if updates_rx.changed().await.is_err() {
                        return;
                    }
                    let update = updates_rx.borrow().clone();
                    genui_update_state.set(update);
                }
            });
        });
    }

    {
        let screen_state = screen;
        let mut genui_handle_state = genui_handle;
        use_side_effect(move || {
            if *screen_state.read() == Screen::GenerativeUi {
                return;
            }

            if let Some(handle) = genui_handle_state.write().take() {
                handle.stop();
            }
        });
    }

    {
        let screen_state = screen;
        let mut mockstation_state = mockstation;
        use_side_effect(move || {
            if *screen_state.read() != Screen::Mockstation {
                return;
            }
            if mockstation_state.read().is_some() {
                return;
            }
            mockstation_state.set(Some(build_mockstation_runtime()));
        });
    }

    let screen_content: Element = match *screen.read() {
        Screen::MainMenu => rect()
            .vertical()
            .spacing(6.0)
            .child(label().text("looprs desktop"))
            .child(label().text("Editor-first shell: AI Workspace + Mockstation."))
            .into_element(),
        Screen::AiChat => {
            let history_items = chat_history.read().clone();

            let on_submit = {
                let mut chat_is_running = chat_is_running;
                let mut chat_status = chat_status;
                let mut chat_input = chat_input;
                let mut chat_history = chat_history;
                move |value: String| {
                    if *chat_is_running.read() {
                        chat_status.set("Turn already running...".to_string());
                        return;
                    }

                    let prompt = value.trim().to_string();
                    if prompt.is_empty() {
                        chat_status.set("Enter a message first".to_string());
                        return;
                    }

                    chat_is_running.set(true);
                    chat_status.set("Running turn...".to_string());
                    chat_history.write().push(ChatMessage {
                        role: "You".to_string(),
                        content: prompt.clone(),
                    });
                    chat_input.set(String::new());

                    let mut chat_is_running_done = chat_is_running;
                    let mut chat_status_done = chat_status;
                    let mut chat_history_done = chat_history;
                    spawn(async move {
                        append_observability_event("chat.send", &prompt).await;
                        append_chat_message("You", &prompt).await;

                        let snapshot = run_turn_for_prompt(prompt.clone()).await;
                        let status = snapshot.status;
                        let response = snapshot.response;

                        append_observability_event("chat.response", &status).await;
                        append_chat_message("Assistant", &response).await;

                        chat_history_done.write().push(ChatMessage {
                            role: "Assistant".to_string(),
                            content: response,
                        });
                        chat_status_done.set(status);
                        chat_is_running_done.set(false);
                    });
                }
            };

            let on_send_click = {
                let mut chat_is_running = chat_is_running;
                let mut chat_status = chat_status;
                let mut chat_input = chat_input;
                let mut chat_history = chat_history;
                move |_| {
                    if *chat_is_running.read() {
                        chat_status.set("Turn already running...".to_string());
                        return;
                    }

                    let prompt = chat_input.read().trim().to_string();
                    if prompt.is_empty() {
                        chat_status.set("Enter a message first".to_string());
                        return;
                    }

                    chat_is_running.set(true);
                    chat_status.set("Running turn...".to_string());
                    chat_history.write().push(ChatMessage {
                        role: "You".to_string(),
                        content: prompt.clone(),
                    });
                    chat_input.set(String::new());

                    let mut chat_is_running_done = chat_is_running;
                    let mut chat_status_done = chat_status;
                    let mut chat_history_done = chat_history;
                    spawn(async move {
                        append_observability_event("chat.send", &prompt).await;
                        append_chat_message("You", &prompt).await;

                        let snapshot = run_turn_for_prompt(prompt.clone()).await;
                        let status = snapshot.status;
                        let response = snapshot.response;

                        append_observability_event("chat.response", &status).await;
                        append_chat_message("Assistant", &response).await;

                        chat_history_done.write().push(ChatMessage {
                            role: "Assistant".to_string(),
                            content: response,
                        });
                        chat_status_done.set(status);
                        chat_is_running_done.set(false);
                    });
                }
            };

            let on_clear_click = {
                let mut chat_status = chat_status;
                let mut chat_history = chat_history;
                move |_| {
                    if *chat_is_running.read() {
                        chat_status.set("Wait for the current turn to finish".to_string());
                        return;
                    }

                    chat_history.set(Vec::new());
                    chat_status.set("Ready".to_string());

                    spawn(async move {
                        clear_chat_messages().await;
                        append_observability_event("chat.clear", "cleared chat transcript").await;
                    });
                }
            };

            let on_use_prompt_click = {
                let mut chat_input = chat_input;
                move |_| {
                    chat_input
                        .set("Summarize what this repository does in 2 sentences.".to_string());
                }
            };

            let history_elements = history_items
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    let bg = if message.role.as_str() == "You" {
                        (50, 74, 114)
                    } else {
                        (64, 64, 64)
                    };
                    rect()
                        .key(index)
                        .width(Size::fill())
                        .padding(Gaps::new_all(8.0))
                        .background(bg)
                        .vertical()
                        .child(label().text(message.role.clone()))
                        .child(label().text(message.content.clone()))
                        .into_element()
                })
                .collect::<Vec<_>>();

            rect()
                .vertical()
                .spacing(12.0)
                .width(Size::fill())
                .height(Size::fill())
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .spacing(8.0)
                        .child(
                            rect().width(Size::percent(70.0)).child(
                                Input::new(chat_input)
                                    .placeholder("Ask looprs anything...")
                                    .width(Size::fill())
                                    .on_submit(on_submit),
                            ),
                        )
                        .child(
                            rect()
                                .width(Size::percent(14.0))
                                .padding(Gaps::new_all(6.0))
                                .background((60, 90, 132))
                                .center()
                                .on_press(on_send_click)
                                .child(label().text("Send")),
                        )
                        .child(
                            rect()
                                .width(Size::percent(16.0))
                                .padding(Gaps::new_all(6.0))
                                .background((90, 90, 90))
                                .center()
                                .on_press(on_clear_click)
                                .child(label().text("Clear")),
                        )
                        .child(
                            rect()
                                .width(Size::percent(14.0))
                                .padding(Gaps::new_all(6.0))
                                .background((52, 76, 120))
                                .center()
                                .on_press(on_use_prompt_click)
                                .child(label().text("Use prompt")),
                        ),
                )
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .height(Size::fill())
                        .spacing(12.0)
                        .child(
                            rect()
                                .width(Size::percent(22.0))
                                .height(Size::fill())
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("Workspace"))
                                .child(label().text(format!("Status: {}", chat_status.read())))
                                .child(label().text(format!("Running: {}", chat_is_running.read())))
                                .child(label().text(""))
                                .child(
                                    label()
                                        .text(format!("Messages: {}", chat_history.read().len())),
                                )
                                .child(label().text(""))
                                .child(
                                    label().text(
                                        "This pane is independent from Mockstation controls.",
                                    ),
                                ),
                        )
                        .child(
                            rect()
                                .width(Size::percent(56.0))
                                .height(Size::fill())
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("Conversation"))
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::fill())
                                        .vertical()
                                        .spacing(6.0)
                                        .children(history_elements),
                                ),
                        )
                        .child(
                            rect()
                                .width(Size::percent(22.0))
                                .height(Size::fill())
                                .vertical()
                                .child(label().text("Inspector"))
                                .child(
                                    label().text("Use 'Use prompt' to prefill a starter question."),
                                )
                                .child(label().text(
                                    "Each Send appends to conversation history in this session.",
                                )),
                        ),
                )
                .into_element()
        }
        Screen::Editor => rect()
            .width(Size::fill())
            .height(Size::fill())
            .vertical()
            .spacing(10.0)
            .child(label().text("Editor"))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .background((28, 28, 28))
                    .child(editor_screen()),
            )
            .into_element(),
        Screen::Terminal => terminal_screen().into_element(),
        Screen::GenerativeUi => {
            let update = genui_update.read().clone();
            let state_pretty = update.state_pretty.clone();
            let tree_pretty = update.component_tree_pretty.clone();

            let on_interval_3 = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::SetIntervalSecs { secs: 3 });
                    }
                }
            };
            let on_interval_5 = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::SetIntervalSecs { secs: 5 });
                    }
                }
            };
            let on_regenerate = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::Regenerate);
                    }
                }
            };
            let on_theme_dark = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::PatchState {
                            patch: serde_json::json!({"theme": "dark"}),
                        });
                    }
                }
            };
            let on_theme_light = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::PatchState {
                            patch: serde_json::json!({"theme": "light"}),
                        });
                    }
                }
            };
            let on_accent_purple = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::PatchState {
                            patch: serde_json::json!({"accent": "rgb(76, 52, 120)"}),
                        });
                    }
                }
            };
            let on_accent_blue = {
                let genui_handle_state = genui_handle;
                move |_| {
                    if let Some(h) = genui_handle_state.read().as_ref() {
                        h.send(GenerativeUiCommand::PatchState {
                            patch: serde_json::json!({"accent": "rgb(60, 90, 132)"}),
                        });
                    }
                }
            };

            rect()
                .width(Size::fill())
                .height(Size::fill())
                .vertical()
                .spacing(10.0)
                .child(label().text("Live Generative UI"))
                .child(label().text(format!("Status: {}", update.status)))
                .child(label().text(format!("Interval: {}s", update.interval_secs)))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .horizontal()
                        .spacing(12.0)
                        .child(
                            rect()
                                .width(Size::percent(22.0))
                                .height(Size::fill())
                                .vertical()
                                .spacing(8.0)
                                .child(label().text("Controls"))
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((60, 90, 132))
                                        .on_press(on_interval_3)
                                        .center()
                                        .child(label().text("Every 3s")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((60, 132, 90))
                                        .on_press(on_interval_5)
                                        .center()
                                        .child(label().text("Every 5s")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((90, 90, 90))
                                        .on_press(on_regenerate)
                                        .center()
                                        .child(label().text("Regenerate")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((52, 76, 120))
                                        .on_press(on_theme_dark)
                                        .center()
                                        .child(label().text("Theme: dark")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((120, 120, 70))
                                        .on_press(on_theme_light)
                                        .center()
                                        .child(label().text("Theme: light")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((76, 52, 120))
                                        .on_press(on_accent_purple)
                                        .center()
                                        .child(label().text("Accent: purple")),
                                )
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .padding(Gaps::new_all(6.0))
                                        .background((60, 90, 132))
                                        .on_press(on_accent_blue)
                                        .center()
                                        .child(label().text("Accent: blue")),
                                ),
                        )
                        .child(
                            rect()
                                .width(Size::percent(52.0))
                                .height(Size::fill())
                                .vertical()
                                .spacing(8.0)
                                .child(label().text("Generated component code"))
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::fill())
                                        .padding(Gaps::new_all(8.0))
                                        .background((44, 44, 44))
                                        .vertical()
                                        .child(SelectableText::new(
                                            update.component_code_preview.clone(),
                                        ))
                                        .maybe_child(
                                            update
                                                .component_code_truncated
                                                .then(|| label().text("(truncated)")),
                                        ),
                                ),
                        )
                        .child(
                            rect()
                                .width(Size::percent(26.0))
                                .height(Size::fill())
                                .vertical()
                                .spacing(8.0)
                                .child(label().text("State"))
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::fill())
                                        .padding(Gaps::new_all(8.0))
                                        .background((54, 54, 54))
                                        .vertical()
                                        .child(label().text(state_pretty)),
                                )
                                .child(label().text("Tree"))
                                .child(
                                    rect()
                                        .width(Size::fill())
                                        .height(Size::percent(40.0))
                                        .padding(Gaps::new_all(8.0))
                                        .background((54, 54, 54))
                                        .vertical()
                                        .child(label().text(tree_pretty)),
                                )
                                .maybe_child(
                                    update
                                        .last_error
                                        .clone()
                                        .map(|err| label().text(format!("Error: {err}"))),
                                ),
                        ),
                )
                .into_element()
        }
        Screen::Mockstation => {
            let live_mock = mockstation
                .read()
                .as_ref()
                .map(|runtime| runtime.snapshot())
                .unwrap_or_default();

            let on_connect_terminal = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.connect_terminal();
                    }
                }
            };
            let on_disconnect_terminal = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.disconnect_terminal();
                    }
                }
            };
            let on_ls = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.run_terminal_command("ls -la");
                    }
                }
            };
            let on_term_frame = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.send_from_terminal("ping from terminal");
                    }
                }
            };

            let on_ws_start = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.start_websocket();
                    }
                }
            };
            let on_ws_stop = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.stop_websocket();
                    }
                }
            };

            let on_rest_start = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.start_rest_api();
                    }
                }
            };
            let on_rest_stop = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.stop_rest_api();
                    }
                }
            };
            let on_rest_health = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.browser_rest_call("/api/health");
                    }
                }
            };

            let on_browser_connect = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.connect_browser();
                    }
                }
            };
            let on_browser_disconnect = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.disconnect_browser();
                    }
                }
            };
            let on_browser_frame = {
                let mut mockstation = mockstation;
                move |_| {
                    if let Some(runtime) = mockstation.write().as_mut() {
                        runtime.send_from_browser("pong from browser");
                    }
                }
            };

            rect()
                .width(Size::fill())
                .height(Size::fill())
                .vertical()
                .spacing(16.0)
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::percent(32.0))
                        .horizontal()
                        .spacing(8.0)
                        .child(
                            rect()
                                .width(Size::percent(25.0))
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("Terminal"))
                                .children([
                                    action_button((44, 90, 160), "Connect", on_connect_terminal),
                                    action_button(
                                        (120, 44, 44),
                                        "Disconnect",
                                        on_disconnect_terminal,
                                    ),
                                    action_button((52, 76, 120), "Run: ls -la", on_ls),
                                    action_button((60, 60, 120), "Send WS frame", on_term_frame),
                                ]),
                        )
                        .child(
                            rect()
                                .width(Size::percent(25.0))
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("WebSocket"))
                                .children([
                                    action_button((60, 100, 150), "Start WS", on_ws_start),
                                    action_button((100, 60, 120), "Stop WS", on_ws_stop),
                                ]),
                        )
                        .child(
                            rect()
                                .width(Size::percent(25.0))
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("REST API"))
                                .children([
                                    action_button((52, 120, 88), "Start REST", on_rest_start),
                                    action_button((120, 88, 52), "Stop REST", on_rest_stop),
                                    action_button(
                                        (70, 120, 120),
                                        "Browser GET /api/health",
                                        on_rest_health,
                                    ),
                                ]),
                        )
                        .child(
                            rect()
                                .width(Size::percent(25.0))
                                .vertical()
                                .spacing(6.0)
                                .child(label().text("Browser"))
                                .children([
                                    action_button((44, 120, 70), "Connect", on_browser_connect),
                                    action_button(
                                        (120, 80, 44),
                                        "Disconnect",
                                        on_browser_disconnect,
                                    ),
                                    action_button(
                                        (60, 120, 120),
                                        "Send WS frame",
                                        on_browser_frame,
                                    ),
                                ]),
                        ),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .horizontal()
                        .spacing(16.0)
                        .child(
                            rect()
                                .width(Size::percent(33.0))
                                .height(Size::fill())
                                .vertical()
                                .child(label().text("Terminal panel"))
                                .child(label().text(live_mock.terminal_view)),
                        )
                        .child(
                            rect()
                                .width(Size::percent(33.0))
                                .height(Size::fill())
                                .vertical()
                                .child(label().text("Browser panel"))
                                .child(label().text(live_mock.browser_view)),
                        )
                        .child(
                            rect()
                                .width(Size::percent(34.0))
                                .height(Size::fill())
                                .vertical()
                                .child(label().text("Transport + server log"))
                                .child(label().text(live_mock.transport_log)),
                        ),
                )
                .into_element()
        }
        Screen::ContextDemo => context_demo_screen().into_element(),
    };

    rect()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new_all(12.0))
        .vertical()
        .spacing(12.0)
        .child(
            rect()
                .width(Size::fill())
                .horizontal()
                .spacing(8.0)
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((60, 90, 132))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::AiChat)
                        })
                        .child(label().text("AI Workspace")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((76, 52, 120))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::Editor)
                        })
                        .child(label().text("Editor")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((46, 108, 76))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::Terminal)
                        })
                        .child(label().text("Terminal")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((52, 76, 120))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::GenerativeUi)
                        })
                        .child(label().text("Generative UI")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((44, 120, 70))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::Mockstation)
                        })
                        .child(label().text("Mockstation")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((76, 175, 80))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::ContextDemo)
                        })
                        .child(label().text("Context Demo")),
                )
                .child(
                    rect()
                        .width(Size::flex(1.0))
                        .padding(Gaps::new_all(6.0))
                        .background((90, 90, 90))
                        .center()
                        .on_press({
                            let mut screen = screen;
                            move |_| screen.set(Screen::MainMenu)
                        })
                        .child(label().text("Main Menu")),
                ),
        )
        .child(screen_content)
}
