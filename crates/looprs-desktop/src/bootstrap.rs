use crate::ui::root;
use freya::prelude::{LaunchConfig, WindowConfig, launch};

pub async fn run() {
    looprs::ui::init_logging();
    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(root::app)
                .with_size(1200.0, 800.0)
                .with_title("looprs AI Workspace")
                .with_min_size(800.0, 600.0),
        ),
    );
}
