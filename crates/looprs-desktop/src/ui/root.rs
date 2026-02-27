use crate::services::agent_adapter::run_turn_for_prompt;
use crate::services::mockstation::build_mockstation_runtime;
use crate::services::sqlite_store::{
    append_chat_message, append_observability_event, clear_chat_messages, load_chat_messages,
};
use freya::prelude::*;

#[derive(Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    AiChat,
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

    use_effect(move || {
        if *chat_history_loaded.read() {
            return;
        }

        chat_history_loaded.set(true);
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
