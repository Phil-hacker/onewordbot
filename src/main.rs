mod error;
mod handler;
use crate::handler::*;
use serenity::{Client, prelude::GatewayIntents};
#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();
    let token = std::env::var("DISCORD_TOKEN").unwrap();
    let mut client = Client::builder(
        token,
        GatewayIntents::GUILD_MESSAGES.union(GatewayIntents::MESSAGE_CONTENT),
    )
    .event_handler(Handler::default())
    .await
    .unwrap();
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
    println!("Hello, world!");
}
