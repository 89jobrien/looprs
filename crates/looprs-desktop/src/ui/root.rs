use crate::services::agent_adapter::run_turn_for_prompt;
use crate::services::generative_ui::{
    GenerativeUiCommand, GenerativeUiUpdate, LiveGenerativeUiHandle, start_live_generative_ui,
};
use crate::services::mockstation::build_mockstation_runtime;
use crate::services::sqlite_store::{
    append_chat_message, append_observability_event, clear_chat_messages, load_chat_messages,
};
use freya::prelude::*;
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
    GenerativeUi,
    Mockstation,
}

pub fn app() -> Element {
    let mut screen = use_signal(|| Screen::AiChat);
    let mut chat_status = use_signal(|| "Ready".to_string());
    let mut chat_is_running = use_signal(|| false);
    let mut chat_input = use_signal(String::new);
    let mut chat_history = use_signal(Vec::<ChatMessage>::new);
    let mut chat_history_loaded = use_signal(|| false);
    let mut mockstation = use_signal(build_mockstation_runtime);
    let mut genui_handle = use_signal(|| None::<LiveGenerativeUiHandle>);
    let mut genui_update = use_signal(GenerativeUiUpdate::default);

    use_effect(move || {
        if *screen.read() != Screen::AiChat {
            return;
        }

        {
            let mut loaded = chat_history_loaded.write();
            if *loaded {
                return;
            }
            *loaded = true;
        }

        spawn(async move {
            let persisted = load_chat_messages(500).await;
            let hydrated = persisted
                .into_iter()
                .map(|message| ChatMessage {
                    role: message.role,
                    content: message.content,
                })
                .collect::<Vec<_>>();
            chat_history.set(hydrated);
        });
    });

    use_effect(move || {
        if *screen.read() != Screen::GenerativeUi {
            return;
        }

        let handle = {
            let mut slot = genui_handle.write();
            if slot.is_some() {
                return;
            }

            let handle = start_live_generative_ui(Duration::from_secs(3));
            *slot = Some(handle.clone());
            handle
        };

        let mut updates_rx = handle.updates.clone();
        spawn(async move {
            loop {
                if updates_rx.changed().await.is_err() {
                    return;
                }
                let update = (*updates_rx.borrow()).clone();
                genui_update.set(update);
            }
        });
    });

    use_effect(move || {
        if *screen.read() == Screen::GenerativeUi {
            return;
        }

        let handle = genui_handle.write().take();
        if let Some(handle) = handle {
            handle.stop();
        }
    });

    rsx!(
        rect {
            width: "100%",
            height: "100%",
            padding: "12",
            direction: "vertical",

            rect {
                width: "100%",
                direction: "horizontal",
                spacing: "8",
                rect {
                    width: "18%",
                    padding: "6",
                    background: "rgb(60, 90, 132)",
                    onclick: move |_| screen.set(Screen::AiChat),
                    label { "AI Workspace" }
                }
                rect {
                    width: "18%",
                    padding: "6",
                    background: "rgb(76, 52, 120)",
                    onclick: move |_| screen.set(Screen::GenerativeUi),
                    label { "Generative UI" }
                }
                rect {
                    width: "18%",
                    padding: "6",
                    background: "rgb(46, 108, 76)",
                    onclick: move |_| screen.set(Screen::Mockstation),
                    label { "Mockstation" }
                }
                rect {
                    width: "18%",
                    padding: "6",
                    background: "rgb(90, 90, 90)",
                    onclick: move |_| screen.set(Screen::MainMenu),
                    label { "Main Menu" }
                }
            }

            label { "" }

            match *screen.read() {
                Screen::MainMenu => rsx!(
                    label { "looprs desktop" }
                    label { "Editor-first shell: AI Workspace + Mockstation." }
                ),
                Screen::AiChat => {
                    let history_items = chat_history.read().clone();
                    rsx!(
                    rect {
                        width: "100%",
                        direction: "vertical",
                        spacing: "10",
                        label { "Self-contained Chatbot" }
                        rect {
                            width: "100%",
                            direction: "horizontal",
                            spacing: "8",
                            rect {
                                width: "70%",
                                Input {
                                    value: chat_input,
                                    placeholder: "Ask looprs anything...".to_string(),
                                    width: "100%".to_string(),
                                    onchange: move |value| chat_input.set(value),
                                    onvalidate: move |_| {
                                        if *chat_is_running.read() {
                                            chat_status.set("Turn already running...".to_string());
                                            return;
                                        }

                                        let prompt = {
                                            let input_value = chat_input.read();
                                            input_value.trim().to_string()
                                        };
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

                                        spawn(async move {
                                            append_observability_event("chat.send", &prompt).await;
                                            append_chat_message("You", &prompt).await;

                                            let snapshot = run_turn_for_prompt(prompt).await;
                                            let status = snapshot.status;
                                            let response = snapshot.response;

                                            append_observability_event("chat.response", &status).await;
                                            append_chat_message("Assistant", &response).await;

                                            chat_history.write().push(ChatMessage {
                                                role: "Assistant".to_string(),
                                                content: response,
                                            });
                                            chat_status.set(status);
                                            chat_is_running.set(false);
                                        });
                                    }
                                }
                            }
                            rect {
                                width: "14%",
                                padding: "6",
                                background: "rgb(60, 90, 132)",
                                onclick: move |_| {
                                    if *chat_is_running.read() {
                                        chat_status.set("Turn already running...".to_string());
                                        return;
                                    }

                                    let prompt = {
                                        let input_value = chat_input.read();
                                        input_value.trim().to_string()
                                    };
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

                                    spawn(async move {
                                        append_observability_event("chat.send", &prompt).await;
                                        append_chat_message("You", &prompt).await;

                                        let snapshot = run_turn_for_prompt(prompt).await;
                                        let status = snapshot.status;
                                        let response = snapshot.response;

                                        append_observability_event("chat.response", &status).await;
                                        append_chat_message("Assistant", &response).await;
                                        chat_history.write().push(ChatMessage {
                                            role: "Assistant".to_string(),
                                            content: response,
                                        });
                                        chat_status.set(status);
                                        chat_is_running.set(false);
                                    });
                                },
                                label { "Send" }
                            }
                        }
                        rect {
                            width: "16%",
                            padding: "6",
                            background: "rgb(90, 90, 90)",
                            onclick: move |_| {
                                if *chat_is_running.read() {
                                    chat_status.set("Wait for the current turn to finish".to_string());
                                    return;
                                }

                                chat_history.set(Vec::new());
                                chat_status.set("Ready".to_string());

                                spawn(async move {
                                    clear_chat_messages().await;
                                    append_observability_event("chat.clear", "cleared chat transcript")
                                        .await;
                                });
                            },
                            label { "Clear" }
                        }
                        rect {
                            width: "14%",
                            padding: "6",
                            background: "rgb(52, 76, 120)",
                            onclick: move |_| {
                                chat_input.set("Summarize what this repository does in 2 sentences.".to_string());
                            },
                            label { "Use prompt" }
                        }
                    }

                    rect {
                        width: "100%",
                        height: "84%",
                        direction: "horizontal",
                        spacing: "12",
                        rect {
                            width: "22%",
                            height: "fill",
                            direction: "vertical",
                            label { "Workspace" }
                            label { "Status: {chat_status.read()}" }
                            label { "Running: {chat_is_running.read()}" }
                            label { "" }
                            label { "Messages: {chat_history.read().len()}" }
                            label { "" }
                            label { "This pane is independent from Mockstation controls." }
                        }
                        rect {
                            width: "56%",
                            height: "fill",
                            direction: "vertical",
                            label { "Conversation" }
                            rect {
                                width: "100%",
                                height: "fill",
                                direction: "vertical",
                                spacing: "6",
                                {
                                    history_items
                                        .iter()
                                        .enumerate()
                                        .map(|(index, message)| {
                                            let bg = if message.role.as_str() == "You" {
                                                "rgb(50, 74, 114)"
                                            } else {
                                                "rgb(64, 64, 64)"
                                            };

                                            rsx!(
                                                rect {
                                                    key: "{index}",
                                                    width: "100%",
                                                    padding: "8",
                                                    background: "{bg}",
                                                    direction: "vertical",
                                                    label { "{message.role}" }
                                                    label { "{message.content}" }
                                                }
                                            )
                                        })
                                }
                            }
                        }
                        rect {
                            width: "22%",
                            height: "fill",
                            direction: "vertical",
                            label { "Inspector" }
                            label { "Use 'Use prompt' to prefill a starter question." }
                            label { "Each Send appends to conversation history in this session." }
                        }
                    }
                    )
                }
                Screen::GenerativeUi => {
                    let update = genui_update.read().clone();
                    let state_pretty = serde_json::to_string_pretty(&update.state)
                        .unwrap_or_else(|_| "{}".to_string());
                    let tree_pretty = serde_json::to_string_pretty(&update.component_tree_json)
                        .unwrap_or_else(|_| "null".to_string());

                    let genui_handle_interval_3 = genui_handle;
                    let genui_handle_interval_5 = genui_handle;
                    let genui_handle_regenerate = genui_handle;
                    let genui_handle_theme_dark = genui_handle;
                    let genui_handle_theme_light = genui_handle;
                    let genui_handle_accent_purple = genui_handle;
                    let genui_handle_accent_blue = genui_handle;

                    rsx!(
                        rect {
                            width: "100%",
                            height: "100%",
                            direction: "vertical",
                            spacing: "10",
                            label { "Live Generative UI" }
                            label { "Status: {update.status}" }
                            label { "Interval: {update.interval_secs}s" }

                            rect {
                                width: "100%",
                                height: "88%",
                                direction: "horizontal",
                                spacing: "12",

                                rect {
                                    width: "22%",
                                    height: "fill",
                                    direction: "vertical",
                                    spacing: "8",
                                    label { "Controls" }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(60, 90, 132)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_interval_3.read().as_ref() {
                                                h.send(GenerativeUiCommand::SetIntervalSecs { secs: 3 });
                                            }
                                        },
                                        label { "Every 3s" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(60, 132, 90)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_interval_5.read().as_ref() {
                                                h.send(GenerativeUiCommand::SetIntervalSecs { secs: 5 });
                                            }
                                        },
                                        label { "Every 5s" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(90, 90, 90)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_regenerate.read().as_ref() {
                                                h.send(GenerativeUiCommand::Regenerate);
                                            }
                                        },
                                        label { "Regenerate" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(52, 76, 120)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_theme_dark.read().as_ref() {
                                                h.send(GenerativeUiCommand::PatchState {
                                                    patch: serde_json::json!({"theme": "dark"}),
                                                });
                                            }
                                        },
                                        label { "Theme: dark" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(120, 120, 70)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_theme_light.read().as_ref() {
                                                h.send(GenerativeUiCommand::PatchState {
                                                    patch: serde_json::json!({"theme": "light"}),
                                                });
                                            }
                                        },
                                        label { "Theme: light" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(76, 52, 120)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_accent_purple.read().as_ref() {
                                                h.send(GenerativeUiCommand::PatchState {
                                                    patch: serde_json::json!({"accent": "rgb(76, 52, 120)"}),
                                                });
                                            }
                                        },
                                        label { "Accent: purple" }
                                    }
                                    rect {
                                        width: "100%",
                                        padding: "6",
                                        background: "rgb(60, 90, 132)",
                                        onclick: move |_| {
                                            if let Some(h) = genui_handle_accent_blue.read().as_ref() {
                                                h.send(GenerativeUiCommand::PatchState {
                                                    patch: serde_json::json!({"accent": "rgb(60, 90, 132)"}),
                                                });
                                            }
                                        },
                                        label { "Accent: blue" }
                                    }
                                }

                                rect {
                                    width: "52%",
                                    height: "fill",
                                    direction: "vertical",
                                    spacing: "8",
                                    label { "Generated component code" }
                                    rect {
                                        width: "100%",
                                        height: "fill",
                                        padding: "8",
                                        background: "rgb(44, 44, 44)",
                                        direction: "vertical",
                                        label { "{update.component_code}" }
                                    }
                                }

                                rect {
                                    width: "26%",
                                    height: "fill",
                                    direction: "vertical",
                                    spacing: "8",
                                    label { "State" }
                                    rect {
                                        width: "100%",
                                        height: "fill",
                                        padding: "8",
                                        background: "rgb(54, 54, 54)",
                                        direction: "vertical",
                                        label { "{state_pretty}" }
                                    }
                                    label { "Tree" }
                                    rect {
                                        width: "100%",
                                        height: "40%",
                                        padding: "8",
                                        background: "rgb(54, 54, 54)",
                                        direction: "vertical",
                                        label { "{tree_pretty}" }
                                    }
                                    if let Some(err) = update.last_error.clone() {
                                        label { "Error: {err}" }
                                    }
                                }
                            }
                        }
                    )
                }
                Screen::Mockstation => {
                    let live_mock = mockstation.read().snapshot();
                    rsx!(
                        rect {
                            width: "100%",
                            height: "32%",
                            direction: "horizontal",
                            spacing: "8",
                            rect {
                                width: "25%",
                                direction: "vertical",
                                spacing: "6",
                                label { "Terminal" }
                                rect { width: "100%", padding: "6", background: "rgb(44, 90, 160)", onclick: move |_| mockstation.write().connect_terminal(), label { "Connect" } }
                                rect { width: "100%", padding: "6", background: "rgb(120, 44, 44)", onclick: move |_| mockstation.write().disconnect_terminal(), label { "Disconnect" } }
                                rect { width: "100%", padding: "6", background: "rgb(52, 76, 120)", onclick: move |_| mockstation.write().run_terminal_command("ls -la"), label { "Run: ls -la" } }
                                rect { width: "100%", padding: "6", background: "rgb(60, 60, 120)", onclick: move |_| mockstation.write().send_from_terminal("ping from terminal"), label { "Send WS frame" } }
                            }
                            rect {
                                width: "25%",
                                direction: "vertical",
                                spacing: "6",
                                label { "WebSocket" }
                                rect { width: "100%", padding: "6", background: "rgb(60, 100, 150)", onclick: move |_| mockstation.write().start_websocket(), label { "Start WS" } }
                                rect { width: "100%", padding: "6", background: "rgb(100, 60, 120)", onclick: move |_| mockstation.write().stop_websocket(), label { "Stop WS" } }
                            }
                            rect {
                                width: "25%",
                                direction: "vertical",
                                spacing: "6",
                                label { "REST API" }
                                rect { width: "100%", padding: "6", background: "rgb(52, 120, 88)", onclick: move |_| mockstation.write().start_rest_api(), label { "Start REST" } }
                                rect { width: "100%", padding: "6", background: "rgb(120, 88, 52)", onclick: move |_| mockstation.write().stop_rest_api(), label { "Stop REST" } }
                                rect { width: "100%", padding: "6", background: "rgb(70, 120, 120)", onclick: move |_| mockstation.write().browser_rest_call("/api/health"), label { "Browser GET /api/health" } }
                            }
                            rect {
                                width: "25%",
                                direction: "vertical",
                                spacing: "6",
                                label { "Browser" }
                                rect { width: "100%", padding: "6", background: "rgb(44, 120, 70)", onclick: move |_| mockstation.write().connect_browser(), label { "Connect" } }
                                rect { width: "100%", padding: "6", background: "rgb(120, 80, 44)", onclick: move |_| mockstation.write().disconnect_browser(), label { "Disconnect" } }
                                rect { width: "100%", padding: "6", background: "rgb(60, 120, 120)", onclick: move |_| mockstation.write().send_from_browser("pong from browser"), label { "Send WS frame" } }
                            }
                        }

                        rect {
                            width: "100%",
                            height: "68%",
                            direction: "horizontal",
                            spacing: "16",
                            rect {
                                width: "33%",
                                height: "fill",
                                direction: "vertical",
                                label { "Terminal panel" }
                                label { "{live_mock.terminal_view}" }
                            }
                            rect {
                                width: "33%",
                                height: "fill",
                                direction: "vertical",
                                label { "Browser panel" }
                                label { "{live_mock.browser_view}" }
                            }
                            rect {
                                width: "34%",
                                height: "fill",
                                direction: "vertical",
                                label { "Transport + server log" }
                                label { "{live_mock.transport_log}" }
                            }
                        }
                    )
                }
            }
        }
    )
}
