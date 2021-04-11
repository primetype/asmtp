mod config;
mod gossips;

pub use self::config::Config;
use self::gossips::Gossips;
use anyhow::{ensure, Context as _, Result};
use asmtp_lib::PassportImporter;
use asmtp_storage::{Storage as Db, StorageOptions};
use bytes::Bytes;
use keynesis::{
    key::ed25519,
    passport::{
        block::{BlockSlice, Hash, Time},
        Passport, PassportBlocks, PassportBlocksSlice,
    },
};
use poldercast::{Gossip, Topic};
use std::collections::HashSet;

#[derive(Clone)]
pub struct Storage {
    gossips: Gossips,
    storage: Db,
    db: sled::Db,

    users: HashSet<ed25519::PublicKey>,
}

impl Storage {
    pub async fn new(config: Config, users_set: HashSet<String>) -> Result<Self> {
        let sled_config = sled::Config::new().temporary(true);

        let sled_db = sled_config.open().with_context(|| {
            format!(
                "Cannot open persistent storage file: {}",
                sled_config.path.display()
            )
        })?;

        let gossips = Gossips::new(&sled_db, config.gossip_refresh_rate)?;

        let mut users: HashSet<ed25519::PublicKey> = HashSet::new();
        for user in users_set {
            let k = user.parse().context("Invalid user")?;
            users.insert(k);
        }

        let storage = Db::new(StorageOptions::Sqlite {
            uri: config.path.display().to_string(),
        })
        .await?;

        Ok(Self {
            users,
            gossips,
            db: sled_db,
            storage,
        })
    }

    pub async fn put_passport(&self, passport_blocks: PassportBlocksSlice<'_>) -> Result<Hash> {
        self.storage.new_passport(passport_blocks).await
    }

    pub async fn get_passport_blocks(&self, id: Hash) -> Result<Option<PassportBlocks<Vec<u8>>>> {
        self.storage
            .get_passport(&id)
            .await
            .context("Failed to get passport's block from persistent storage")
    }

    pub async fn get_passport_from_topic(&self, topic: Topic) -> Result<Option<Passport>> {
        let mut id = [0; Hash::SIZE];
        if topic.as_ref()[..(Topic::SIZE - Hash::SIZE)] != [0; Topic::SIZE - Hash::SIZE] {
            return Ok(None);
        }

        let hash = &topic.as_ref()[(Topic::SIZE - Hash::SIZE)..];
        id.copy_from_slice(hash.as_ref());
        let id = Hash::from(id);

        if let Some(blocks) = self.get_passport_blocks(id).await? {
            let passport = PassportImporter::from_blocks_owned(
                blocks.as_slice().iter().map(|b| b.to_block()),
            )?;
            Ok(Some(passport))
        } else {
            Ok(None)
        }
    }

    pub async fn handle_incoming_message(&mut self, topic: Topic, message: Bytes) -> Result<()> {
        if let Some(mut passport) = self
            .get_passport_from_topic(topic)
            .await
            .context("Error while handling incoming message")?
        {
            let block = BlockSlice::try_from_slice(message.as_ref()).with_context(|| {
                format!("cannot handle new block for passport {}", passport.id())
            })?;

            passport.push(block)?;

            self.storage.update_passport(passport.blocks()).await?;
            Ok(())
        } else if self.storage.contains_tread(&topic).await? {
            let _message_id = self.storage.new_message(&topic, message).await?;
            Ok(())
        } else {
            // simply ignore the message and relay anyway
            Ok(())
        }
    }

    pub async fn handle_get_passport(&self, id: Hash) -> Result<Option<PassportBlocks<Vec<u8>>>> {
        self.storage
            .get_passport(&id)
            .await
            .context("Failed to query passport from block")
    }

    pub async fn handle_put_passport(
        &self,
        peer: ed25519::PublicKey,
        id: Hash,
        blocks: PassportBlocks<Vec<u8>>,
    ) -> Result<()> {
        ensure!(
            self.users.contains(&peer),
            "user needs to be registered in order to allow them to publish passports"
        );

        tracing::info!(id = %id, peer = %peer, "received new passport blocks");
        let resulted_id = self.put_passport(blocks.as_slice()).await?;

        ensure!(
            resulted_id == id,
            "the passport does not match the expected given hash"
        );
        Ok(())
    }

    pub async fn subscribe_message(&self, topic: Topic) -> Result<()> {
        self.storage.new_thread(&topic).await
    }

    /*
    pub fn subscribe_passport_update(&self) -> impl Stream<Item = Hash> {
        self.gen.subscribe_passport_update()
    }*/

    /// list all the subscriptions we have in the storage
    pub async fn topic_subscriptions(&self) -> Result<Vec<Topic>> {
        let mut topics: Vec<Topic> = self
            .storage
            .passports()
            .await?
            .into_iter()
            .map(|p| {
                let h = p.id;
                let mut bytes = [0; Topic::SIZE];
                let topic = &mut bytes[(Topic::SIZE - Hash::SIZE)..];
                topic.copy_from_slice(&h);
                Topic::new(bytes)
            })
            .collect();

        topics.extend(self.storage.threads().await?.into_iter().map(|t| {
            let mut topic = [0; Topic::SIZE];
            topic.copy_from_slice(&t.topic);
            Topic::new(topic)
        }));

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

    pub async fn messages(&self, topic: Topic, since: Time) -> Result<Vec<Vec<u8>>> {
        let since = chrono::DateTime::from(since.to_system_time());
        self.storage
            .messages_of_thread_since(&topic, since)
            .await
            .map(|r| r.into_iter().map(|m| m.content).collect())
    }

    pub async fn put_topic(&self, peer: ed25519::PublicKey, topic: Topic) -> Result<()> {
        ensure!(
            self.users.contains(&peer),
            "user needs to be registered in order to allow them to subscribe to topics"
        );

        self.storage
            .new_thread(&topic)
            .await
            .context("Failed to store updated information in the persistent storage")?;

        Ok(())
    }

    pub async fn remove_topic(&self, peer: ed25519::PublicKey, topic: Topic) -> Result<()> {
        ensure!(
            self.users.contains(&peer),
            "user needs to be registered in order to allow them to unsubscribe from topics"
        );

        self.storage.delete_thread(&topic).await?;

        Ok(())
    }
}
