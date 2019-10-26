use std::error::Error;

mod draw;
mod player;
mod providers;

use draw::Interafce;
use player::Player;
use providers::Provider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let provider = Provider::new();

    let (player, chan) = Player::new();
    player.start_worker();

    Interafce::create(provider, chan)?.run().await?;
    Ok(())
}
