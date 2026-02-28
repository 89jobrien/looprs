use crate::ui::root;
use freya::prelude::{LaunchConfig, WindowConfig, launch};

pub async fn run() {
    looprs::ui::init_logging();
    launch(LaunchConfig::new().with_window(WindowConfig::new(root::app)));
}
