use crate::services::agent_adapter::{GuiSnapshot, run_one_turn_blocking};
use crate::services::mockstation::build_mockstation_runtime;
use freya::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    AiChat,
    Mockstation,
}

pub fn app() -> Element {
    let mut screen = use_signal(|| Screen::MainMenu);
    let mut chat = use_signal(GuiSnapshot::fallback);
    let mut mockstation = use_signal(build_mockstation_runtime);

    rsx!(
        rect {
            width: "100%",
            height: "100%",
            padding: "12",
            direction: "vertical",
            match *screen.read() {
                Screen::MainMenu => rsx!(
                    label { "looprs desktop" }
                    label { "" }
                    rect {
                        width: "100%",
                        height: "40%",
                        direction: "vertical",
                        spacing: "10",
                        rect {
                            width: "100%",
                            height: "30%",
                            padding: "8",
                            background: "rgb(48, 78, 132)",
                            onclick: move |_| screen.set(Screen::AiChat),
                            label { "AI Chat" }
                            label { "Run LLM turn only when you click Run." }
                        }
                        rect {
                            width: "100%",
                            height: "30%",
                            padding: "8",
                            background: "rgb(46, 108, 76)",
                            onclick: move |_| screen.set(Screen::Mockstation),
                            label { "Mockstation" }
                            label { "Terminal/Browser mock websocket controls." }
                        }
                    }
                ),
                Screen::AiChat => rsx!(
                    rect {
                        width: "100%",
                        direction: "horizontal",
                        spacing: "8",
                        rect {
                            width: "18%",
                            padding: "6",
                            background: "rgb(90, 90, 90)",
                            onclick: move |_| screen.set(Screen::MainMenu),
                            label { "Back to menu" }
                        }
                        rect {
                            width: "18%",
                            padding: "6",
                            background: "rgb(60, 90, 132)",
                            onclick: move |_| chat.set(run_one_turn_blocking()),
                            label { "Run AI turn" }
                        }
                    }
                    label { "" }
                    label { "Status: {chat.read().status}" }
                    rect {
                        width: "100%",
                        height: "85%",
                        direction: "vertical",
                        label { "Prompt:" }
                        label { "{chat.read().prompt}" }
                        label { "" }
                        label { "Response:" }
                        label { "{chat.read().response}" }
                    }
                ),
                Screen::Mockstation => {
                    let live_mock = mockstation.read().snapshot();
                    rsx!(
                        rect {
                            width: "100%",
                            direction: "horizontal",
                            spacing: "8",
                            rect {
                                width: "18%",
                                padding: "6",
                                background: "rgb(90, 90, 90)",
                                onclick: move |_| screen.set(Screen::MainMenu),
                                label { "Back to menu" }
                            }
                        }
                        label { "Mockstation" }
                        rect {
                            width: "100%",
                            height: "20%",
                            direction: "horizontal",
                            spacing: "8",
                            rect {
                                width: "16%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(44, 90, 160)",
                                onclick: move |_| mockstation.write().connect_terminal(),
                                label { "Connect terminal" }
                            }
                            rect {
                                width: "16%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(120, 44, 44)",
                                onclick: move |_| mockstation.write().disconnect_terminal(),
                                label { "Disconnect terminal" }
                            }
                            rect {
                                width: "16%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(44, 120, 70)",
                                onclick: move |_| mockstation.write().connect_browser(),
                                label { "Connect browser" }
                            }
                            rect {
                                width: "16%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(120, 80, 44)",
                                onclick: move |_| mockstation.write().disconnect_browser(),
                                label { "Disconnect browser" }
                            }
                            rect {
                                width: "18%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(60, 60, 120)",
                                onclick: move |_| {
                                    mockstation
                                        .write()
                                        .send_from_terminal("ping from terminal control")
                                },
                                label { "Terminal send" }
                            }
                            rect {
                                width: "18%",
                                height: "fill",
                                padding: "6",
                                background: "rgb(60, 120, 120)",
                                onclick: move |_| {
                                    mockstation
                                        .write()
                                        .send_from_browser("pong from browser control")
                                },
                                label { "Browser send" }
                            }
                        }
                        rect {
                            width: "100%",
                            height: "80%",
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
                                label { "Wire log" }
                                label { "{live_mock.transport_log}" }
                            }
                        }
                    )
                }
            }
        }
    )
}
