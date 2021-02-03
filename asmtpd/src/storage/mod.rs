mod config;
mod gossips;

pub use self::config::{Config, Mode};
use self::gossips::Gossips;
use anyhow::{Context as _, Result};
use asmtp_storage::{Message, Messages, Passport, Passports};
use bytes::Bytes;
use keynesis::{
    key::ed25519,
    passport::block::{BlockSlice, Hash},
};
use poldercast::{Gossip, Topic};

#[derive(Clone)]
pub struct Storage {
    gossips: Gossips,
    db: sled::Db,
    passports: Passports,
    messages: Messages,
}

impl Storage {
    pub fn new(config: Config) -> Result<Self> {
        let sled_config = sled::Config::new()
            .path(&config.path)
            .mode(config.mode.into())
            .print_profile_on_drop(config.profile_on_drop);

        let sled_config = if let Some(value) = config.compression_factor {
            sled_config.use_compression(true).compression_factor(value)
        } else {
            sled_config.use_compression(false)
        };

        let sled_db = sled_config.open().with_context(|| {
            format!(
                "Cannot open persistent storage file: {}",
                sled_config.path.display()
            )
        })?;

        let gossips = Gossips::new(&sled_db, config.gossip_refresh_rate)?;
        let passports = Passports::new(&sled_db)
            .context("Cannot open persistent backend storage for passports")?;
        let messages = Messages::open(&sled_db)
            .context("Cannot open the message topics persistent backend")?;

        Ok(Self {
            gossips,
            passports,
            messages,
            db: sled_db,
        })
    }

    pub fn get_passport_from_key(&self, key: ed25519::PublicKey) -> Result<Option<Passport>> {
        self.passports.get(&key)
    }

    pub fn get_passport_from_topic(&self, topic: Topic) -> Result<Option<Passport>> {
        let mut id = [0; Hash::SIZE];
        if topic.as_ref()[..(Topic::SIZE - Hash::SIZE)] != [0; Topic::SIZE - Hash::SIZE] {
            return Ok(None);
        }

        let hash = &topic.as_ref()[(Topic::SIZE - Hash::SIZE)..];
        id.copy_from_slice(hash.as_ref());
        let id = Hash::from(id);

        self.passports.get(&id)
    }

    pub fn handle_incoming_message(&mut self, topic: Topic, message: Bytes) -> Result<()> {
        if let Some(passport) = self
            .get_passport_from_topic(topic)
            .context("Error while handling incoming message")?
        {
            let block = BlockSlice::try_from_slice(message.as_ref()).with_context(|| {
                format!("cannot handle new block for passport {}", passport.id())
            })?;

            self.passports
                .create_or_update(block)
                .with_context(|| format!("cannot update passport {} with new block", passport.id()))
                .map(|_| ())
        } else if self.messages.contains(&topic)? {
            let m = Message::open(&self.db, topic)?;

            let _message_id = m.insert(message)?;
            Ok(())
        } else {
            // simply ignore the message and relay anyway
            Ok(())
        }
    }

    pub fn subscribe_message(&self, topic: Topic) -> Result<()> {
        self.messages
            .insert(topic)
            .context("Cannot subscribe to the new message")
    }

    /*
    pub fn subscribe_passport_update(&self) -> impl Stream<Item = Hash> {
        self.gen.subscribe_passport_update()
    }*/

    /// list all the subscriptions we have in the storage
    pub fn topic_subscriptions(&self) -> Result<Vec<Topic>> {
        let mut topics: Vec<Topic> = self
            .passports
            .all_passports()?
            .into_iter()
            .map(|h| {
                let mut bytes = [0; Topic::SIZE];
                let topic = &mut bytes[(Topic::SIZE - Hash::SIZE)..];
                topic.copy_from_slice(h.as_ref());
                Topic::new(bytes)
            })
            .collect();

        topics.extend(self.messages.range(..));

        Ok(topics)
    }

    /// list of the known gossips in the storage
    pub fn known_gossips(&self) -> Result<Vec<Gossip>> {
        self.gossips.gossips()
    }

    pub fn needs_update_known_gossips(&self) -> bool {
        self.gossips.needs_updated()
    }

    /// update the known gossips
    pub fn update_known_gossips(&self, gossips: Vec<Gossip>) -> Result<()> {
        self.gossips.update(gossips)
    }

    pub fn get_topic(&self, topic: &Topic) -> Result<Option<Message>> {
        if self.messages.contains(topic)? {
            Message::open(&self.db, *topic)
                .context("Cannot access topic message from the storage")
                .map(Some)
        } else {
            Ok(None)
        }
    }
}
