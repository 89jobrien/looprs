use looprs_desktop::bootstrap;

#[tokio::main]
async fn main() {
    bootstrap::run().await;
}
