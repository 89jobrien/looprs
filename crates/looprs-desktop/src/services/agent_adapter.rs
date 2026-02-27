use crate::services::mockstation::{MockstationSnapshot, build_mockstation_snapshot};
use looprs::runtime::events::gui_turn_metadata;
use looprs::runtime::facade::bootstrap_runtime;
use looprs::runtime::session::run_single_turn;

#[derive(Clone, Default)]
pub struct GuiSnapshot {
    pub status: String,
    pub prompt: String,
    pub response: String,
    pub mockstation: MockstationSnapshot,
}

impl GuiSnapshot {
    pub fn fallback() -> Self {
        Self {
            status: "No run snapshot found".to_string(),
            prompt: String::new(),
            response: "Run this binary with LOOPRS_GUI_PROMPT to execute one turn.".to_string(),
            mockstation: build_mockstation_snapshot(),
        }
    }
}

pub async fn run_one_turn() -> GuiSnapshot {
    let prompt = std::env::var("LOOPRS_GUI_PROMPT")
        .unwrap_or_else(|_| "Summarize what this repository does in 2 sentences.".to_string());

    let mut bootstrapped = match bootstrap_runtime(None).await {
        Ok(runtime) => runtime,
        Err(error) => {
            return GuiSnapshot {
                status: "Provider initialization failed".to_string(),
                prompt,
                response: error.to_string(),
                mockstation: build_mockstation_snapshot(),
            };
        }
    };

    match run_single_turn(&mut bootstrapped.agent, prompt.clone(), gui_turn_metadata()).await {
        Ok(response) => GuiSnapshot {
            status: "Turn complete".to_string(),
            prompt,
            response,
            mockstation: build_mockstation_snapshot(),
        },
        Err(error) => GuiSnapshot {
            status: "Turn failed".to_string(),
            prompt,
            response: error.to_string(),
            mockstation: build_mockstation_snapshot(),
        },
    }
}

pub fn run_one_turn_blocking() -> GuiSnapshot {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(run_one_turn()),
        Err(error) => GuiSnapshot {
            status: "Failed to start runtime".to_string(),
            prompt: String::new(),
            response: error.to_string(),
            mockstation: build_mockstation_snapshot(),
        },
    }
}
