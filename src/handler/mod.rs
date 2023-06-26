use chrono::{Duration, Utc};
use serenity::{
    async_trait,
    model::prelude::{
        command::Command, interaction::application_command::CommandDataOptionValue, *,
    },
    prelude::*,
    utils::MessageBuilder,
};
use sqlite::State;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const MESSAGE_DELAY: i64 = 1500;

pub struct Handler {
    connection: Arc<Mutex<sqlite::Connection>>,
    last_message: Arc<Mutex<HashMap<u64,Timestamp>>>,
}

impl Handler {
    pub fn is_on_cooldown(&self, message: &Message) -> Result<bool, crate::error::Error> {
        let last_message = self.last_message.lock()?;
        if last_message.contains_key(&message.channel_id.0) {
            return Ok(message.timestamp.timestamp_millis() - last_message.get(&message.channel_id.0).unwrap().timestamp_millis() < MESSAGE_DELAY);
        }
        Ok(false)
    }
    pub fn add_channel(&self, channelid: &ChannelId) -> Result<(), crate::error::Error> {
        let connection = self.connection.lock()?;
        Ok(connection.execute(format!(
            "INSERT OR IGNORE INTO channels(id,text) VALUES ({},'')",
            channelid.0
        ))?)
    }
    pub fn remove_channel(&self, channelid: &ChannelId) -> Result<(), crate::error::Error> {
        let connection = self.connection.lock()?;
        Ok(connection.execute(format!("DELETE FROM channels WHERE id={}", channelid.0))?)
    }
    pub fn is_channel_registered(&self, channelid: &ChannelId) -> Result<bool, crate::error::Error> {
        let connection = self.connection.lock()?;
        let is_registered = connection
            .prepare(format!("SELECT * FROM channels WHERE id={}", channelid.0))?
            .iter().count()
            > 0;
        Ok(is_registered)
    }
    pub fn is_last_message_sender(&self, channelid: &ChannelId, user: &User) -> Result<bool, crate::error::Error> {
        let connection = self.connection.lock()?;
        let is_registered = connection
            .prepare(format!("SELECT * FROM channels WHERE id={} AND last_user = {}", channelid.0, user.id.0))?
            .iter().count()
            > 0;
        Ok(is_registered)
    }
    pub fn append_text(&self, message: &Message) -> Result<(), crate::error::Error> {
        let mut last_message = self.last_message.lock()?;
        let connection = self.connection.lock()?;
        last_message.insert(message.channel_id.0, message.timestamp);
        Ok(connection.execute(format!(
            "UPDATE channels SET text = text || '{} ',last_user = {}  WHERE id={};",
            message.content, message.author.id.0, message.channel_id.0
        ))?)
    }
    pub fn pop_text(&self, channelid: &ChannelId) -> Result<String,crate::error::Error> {
        let connection = self.connection.lock()?;
        let mut state = connection
            .prepare(format!("SELECT text FROM channels WHERE id={}", channelid.0))?;
        if let Ok(State::Row) = state.next() {
            let string = state.read::<String, _>(0).unwrap();
            drop(state);
            connection.execute(format!(
                "UPDATE channels SET text = '',last_user = 0 WHERE id={};",
                channelid.0
            ))?;
            return Ok(string);
        }
        Err(crate::error::Error::UnknownError)
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            connection: Arc::new(Mutex::new(sqlite::open("oneworddb.sqlite").unwrap())),
            last_message: Arc::new(Mutex::new(HashMap::default())),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, message: Message) {
        if self
            .is_channel_registered(&message.channel_id)
            .unwrap_or(false) && !&message.author.bot
        {
            if self.is_on_cooldown(&message).unwrap() {
                ctx.http.delete_message(message.channel_id.0, message.id.0).await.unwrap();
                return;
            }
            if message.content.contains(' ') || self.is_last_message_sender(&message.channel_id, &message.author).unwrap() {
                ctx.http.delete_message(message.channel_id.0, message.id.0).await.unwrap();
                ctx.http
                    .get_guild(message.guild_id.unwrap().0)
                    .await
                    .unwrap()
                    .edit_member(ctx.http, message.author, |edit| {
                        let time = Utc::now();
                        let time = time + Duration::minutes(1);
                        edit.disable_communication_until_datetime(Timestamp::from(time))
                    })
                    .await
                    .unwrap();
            } else {
                self.append_text(&message).unwrap();
                if message.content.contains(".") {
                    let string = self.pop_text(&message.channel_id).unwrap();
                    println!("{}",string);
                    message.channel_id.send_message(&ctx.http,|msg| {
                        msg.embed(|embed| {
                            embed.description(&string)
                        })
                    }).await.unwrap();
                }
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: interaction::Interaction) {
        match interaction {
            interaction::Interaction::Ping(_) => todo!(),
            interaction::Interaction::ApplicationCommand(command) => {
                let cmd = command.data.name.as_str();
                let options = command.data.options.clone();
                match cmd {
                    "add_channel" => {
                        let is_admin = || -> Option<bool> {
                            Some(command.member.clone()?.permissions?.administrator())
                        }()
                        .unwrap_or(false);
                        if is_admin {
                            let value = || -> Option<u64> {
                                match options.get(0)?.resolved.clone()? {
                                    CommandDataOptionValue::Channel(channel) => Some(channel.id.0),
                                    _ => None,
                                }
                            }();
                            match value {
                                Some(channel) => match self.add_channel(&ChannelId(channel)) {
                                    Ok(_) => {
                                        command.create_interaction_response(&ctx.http, |response| {
                                        response.kind(interaction::InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|response| {
                                            response.content(MessageBuilder::new().push("Added ").channel(channel).push(" to the one word challenge.").build())
                                        })
                                    }).await.unwrap();
                                    }
                                    Err(err) => {
                                        println!("{:?}", err);
                                    }
                                },
                                None => {}
                            };
                        }
                    }
                    "remove_channel" => {
                        let is_admin = || -> Option<bool> {
                            Some(command.member.clone()?.permissions?.administrator())
                        }()
                        .unwrap_or(false);
                        if is_admin {
                            let value = || -> Option<u64> {
                                match options.get(0)?.resolved.clone()? {
                                    CommandDataOptionValue::Channel(channel) => Some(channel.id.0),
                                    _ => None,
                                }
                            }();
                            match value {
                                Some(channel) => {
                                    if self.remove_channel(&ChannelId(channel)).is_ok() {
                                        command.create_interaction_response(&ctx.http, |response| {
                                            response.kind(interaction::InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|response| {
                                                response.content(MessageBuilder::new().push("Removed ").channel(channel).push(" from the one word challenge.").build())
                                            })
                                        }).await.unwrap();
                                    }
                                }
                                None => {}
                            };
                        }
                    }
                    _ => {}
                }
            }
            interaction::Interaction::MessageComponent(_) => todo!(),
            interaction::Interaction::Autocomplete(_) => todo!(),
            interaction::Interaction::ModalSubmit(_) => todo!(),
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        let _commands = Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("add_channel")
                .add_option(
                    serenity::builder::CreateApplicationCommandOption(HashMap::from([]))
                        .name("channel")
                        .description("Channel to add")
                        .kind(serenity::model::prelude::command::CommandOptionType::Channel)
                        .clone(),
                )
                .description("Add a new channel to the one word challenge.")
        })
        .await;
        let _commands = Command::create_global_application_command(&ctx.http, |command| {
            command
                .name("remove_channel")
                .add_option(
                    serenity::builder::CreateApplicationCommandOption(HashMap::from([]))
                        .name("channel")
                        .description("Channel to remove")
                        .kind(serenity::model::prelude::command::CommandOptionType::Channel)
                        .clone(),
                )
                .description("Remove a channel from the one word challenge.")
        })
        .await;
    }
}
