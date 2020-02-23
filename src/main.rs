use std::error::Error;
use std::fs::File;

use flexi_logger::Logger;

mod app;
mod config;
mod draw;
mod input;
mod key;
mod meta;
mod player;
mod providers;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    Logger::with_env_or_str("warn, rum_player = debug")
        .log_to_file()
        .directory("/tmp")
        .start()?;
    log::info!("Logging initialized");

    let config = dirs::config_dir()
        .and_then(|mut config_path| {
            config_path.push("rum-player");
            config_path.push("config");
            File::open(config_path)
                .and_then(|mut file| {
                    let mut s = String::new();
                    use std::io::Read;
                    file.read_to_string(&mut s)?;
                    Ok(s)
                })
                .ok()
        })
        .map(|x| x.parse())
        .transpose()?
        .unwrap_or_else(Config::default);

    let provider = providers::Provider::new();

    let (player, chan) = player::Player::new();
    let (state, _) = player.start_worker();

    let app = app::App::create(config, provider, chan, state)?;
    log::info!("Spinning up a fancy UI");
    app.run().await?;

    Ok(())
}
