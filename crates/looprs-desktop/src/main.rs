mod bootstrap;
mod services;
mod ui;

#[tokio::main]
async fn main() {
    bootstrap::run().await;
}
