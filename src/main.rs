use std::error::Error;

mod app;
mod draw;
mod player;
mod providers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let provider = providers::Provider::new();

    let (player, chan) = player::Player::new();
    player.start_worker();

    let app = app::App::create(provider, chan)?;
    app.run().await?;

    Ok(())
}
