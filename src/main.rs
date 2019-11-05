use std::error::Error;

use flexi_logger::Logger;

mod app;
mod draw;
mod input;
mod key;
mod meta;
mod player;
mod providers;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    Logger::with_env_or_str("warn, rum_player = debug")
        .log_to_file()
        .directory("/tmp")
        .start()?;
    log::info!("Logging initialized");

    let provider = providers::Provider::new();

    let (player, chan) = player::Player::new();
    let (state, _) = player.start_worker();

    let app = app::App::create(provider, chan, state)?;
    log::info!("Spinning up a fancy UI");
    app.run().await?;

    Ok(())
}
