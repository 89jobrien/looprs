use crate::ui::root;
use freya::prelude::launch;

pub async fn run() {
    looprs::ui::init_logging();
    launch(root::app);
}
