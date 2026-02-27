use crate::services::agent_adapter::GuiSnapshot;
use freya::prelude::*;
use std::sync::OnceLock;

static SNAPSHOT: OnceLock<GuiSnapshot> = OnceLock::new();

pub fn set_snapshot(snapshot: GuiSnapshot) {
    let _ = SNAPSHOT.set(snapshot);
}

pub fn app() -> Element {
    let snapshot = SNAPSHOT
        .get()
        .cloned()
        .unwrap_or_else(GuiSnapshot::fallback);

    rsx!(
        rect {
            width: "100%",
            height: "100%",
            padding: "12",
            direction: "vertical",
            label { "Status: {snapshot.status}" }
            rect {
                width: "100%",
                height: "fill",
                direction: "vertical",
                label { "Prompt:" }
                label { "{snapshot.prompt}" }
                label { "" }
                label { "Response:" }
                label { "{snapshot.response}" }
            }
        }
    )
}
