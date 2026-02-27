use crate::services::agent_adapter;
use crate::ui::root;
use freya::prelude::launch;

pub async fn run() {
    looprs::ui::init_logging();

    let snapshot = agent_adapter::run_one_turn().await;
    root::set_snapshot(snapshot);

    launch(root::app);
}
