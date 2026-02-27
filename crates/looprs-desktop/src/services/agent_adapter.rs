use looprs::runtime::events::gui_turn_metadata;
use looprs::runtime::facade::bootstrap_runtime;
use looprs::runtime::session::run_single_turn;

#[derive(Clone, Default)]
pub struct GuiSnapshot {
    pub status: String,
    pub response: String,
}

pub async fn run_turn_for_prompt(prompt: String) -> GuiSnapshot {
    let mut bootstrapped = match bootstrap_runtime(None).await {
        Ok(runtime) => runtime,
        Err(error) => {
            return GuiSnapshot {
                status: "Provider initialization failed".to_string(),
                response: error.to_string(),
            };
        }
    };

    match run_single_turn(&mut bootstrapped.agent, prompt, gui_turn_metadata()).await {
        Ok(response) => GuiSnapshot {
            status: "Turn complete".to_string(),
            response,
        },
        Err(error) => GuiSnapshot {
            status: "Turn failed".to_string(),
            response: error.to_string(),
        },
    }
}
