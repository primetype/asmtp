mod key;
mod keys;
mod network;
mod passport;
mod passports;

pub use self::{
    key::{Key, KeyFile},
    keys::Keys,
    network::{Network, NetworkStats},
    passport::Passport,
    passports::Passports,
};
use anyhow::{anyhow, ensure, Context as _, Result};
use asmtp_lib::PassportImporter;
use asmtp_network::Message;
use asmtp_storage::{Storage, StorageOptions};
use directories::ProjectDirs;
use keynesis::{
    key::ed25519::PublicKey,
    passport::{
        block::{Hash, Time},
        PassportBlocksSlice,
    },
    Seed,
};
use poldercast::{GossipSlice, Topic};
use rand_chacha::ChaChaRng;
use std::{net::SocketAddr, path::PathBuf};

/// Application settings
///
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    /// directory in which the data are stored
    ///
    /// the default is to use the system's directory settings:
    ///
    /// |Platform | Example                                                               |
    /// | ------- | --------------------------------------------------------------------- |
    /// | Linux   | /home/alice/.local/share/asmtp-client                                 |
    /// | macOS   | /Users/Alice/Library/Application Support/uk.co.primetype.asmtp-client |
    /// | Windows | C:\Users\Alice\AppData\Local\primetype\asmtp-client\data              |
    ///
    /// However it is possible to set a specific path that will be used instead
    pub directory: Option<PathBuf>,

    pub remote_address: SocketAddr,

    pub remote_id: PublicKey,
}

pub struct App {
    /// PRNG, by default the [`ChaChaRng`] will be randomly seeded.
    /// so it is going to re-generated.
    pub rng: ChaChaRng,

    /// the list of keys handled by this device
    pub keys: Keys,
    pub current_key: Option<usize>,

    /// the list of all the handled passports
    pub passports: Passports,
    pub current_passport: Option<Hash>,

    pub config: Config,
    pub network: Network,
    pub storage: Storage,
}

impl Config {
    fn default_dir() -> Result<ProjectDirs> {
        ProjectDirs::from("uk.co", "primetype", "asmtp-client").ok_or_else(|| {
            anyhow!("Failed to find a valid HOME directory from the operating system")
        })
    }

    pub fn config_directory(&self) -> Result<PathBuf> {
        if let Some(directory) = &self.directory {
            Ok(directory.clone())
        } else {
            Self::default_dir().map(|dirs| dirs.config_dir().to_path_buf())
        }
    }

    pub fn data_local_directory(&self) -> Result<PathBuf> {
        if let Some(directory) = &self.directory {
            Ok(directory.clone())
        } else {
            Self::default_dir().map(|dirs| dirs.data_local_dir().to_path_buf())
        }
    }
}

impl App {
    /// create an [`App`] instance with the given [`Config`]
    ///
    /// This function is similar to [`with_seed`] but generate a [`Seed`]
    /// from [`rand::rngs::OsRng`]. If you want to use a different random
    /// generator to seed the [`App`]'s RNG you should use [`with_seed`].
    pub async fn new(config: Config) -> Result<Self> {
        let seed = Seed::generate(&mut rand::rngs::OsRng);
        Self::with_seed(config, seed).await
    }

    /// create an [`App`] instance with the given [`Config`] and the given [`Seed`]
    /// for the [`App`]'s RNG (Random Number Generator).
    ///
    /// The [`App`] uses [`ChaChaRng`] as random number generator. The [`Seed`] will be
    /// used to initialize the RNG.
    ///
    pub async fn with_seed(config: Config, rng_seed: Seed) -> Result<Self> {
        let data_local_directory = config.data_local_directory()?;

        let rng = rng_seed.into_rand_chacha();

        let mut keys = Keys::new(data_local_directory.join("keys"))?;
        let storage = Storage::new(StorageOptions::Sqlite {
            uri: format!("file:{}", data_local_directory.join("db.sqlite").display()),
        })
        .await?;

        let mut passports = Passports::new()?;

        for key in keys.keys_mut() {
            if let Some(pk) = key.public_key().cloned() {
                let info = storage
                    .key(&pk)
                    .await
                    .with_context(|| format!("Failed to query information about the key {}", pk))?;

                if let Some(info) = info {
                    if let Some(alias) = info.alias {
                        key.set_alias(alias);
                    }
                }
            }
        }

        for passport in storage.passports().await? {
            let blocks = passport.blocks;
            let blocks = PassportBlocksSlice::try_from_slice(&blocks)
                .context("Failed to parse a passport blockchain from the storage")?;
            let passport =
                PassportImporter::from_blocks_owned(blocks.into_iter().map(|b| b.to_block()))
                    .context("Persistent storage contains an invalid passport")?;

            let passport = Passport::new(passport);
            passports.insert(passport);
        }

        let network = Network::default();

        Ok(Self {
            rng,
            keys,
            current_key: None,
            passports,
            current_passport: None,

            config,
            network,
            storage,
        })
    }

    pub async fn create_new_key(&mut self, alias: &str) -> Result<PublicKey> {
        let seed = Seed::generate(&mut self.rng);
        let index = self.keys.add_key(KeyFile::Seed { seed })?;
        let key = self
            .keys
            .keys_mut()
            .get_mut(index)
            .ok_or_else(|| {
                anyhow!(
                    "Cannot retrieve the key ({index}) from the memory access",
                    index = index
                )
            })
            .expect("Key should be present");

        // we already know the public key is going to be there. However as
        // we start implementing more complex type of keys this may not come
        // cheaply.
        let public_key = *key
            .public_key()
            .expect("We assume we used a single seed key");

        // store the alias given to the key. this will be needed/used when we
        // link that key to a passport
        self.storage.new_key(&public_key).await.with_context(|| {
            format!(
                "Failed to store the public key ({}) on the persistent storage",
                public_key
            )
        })?;

        // set the alias to the key in the storage as well as in the cached key
        key.set_alias(alias);
        self.storage
            .new_key_alias(&public_key, alias)
            .await
            .with_context(|| {
                format!(
                    "Failed to store the public key ({})'s alias ({}) on the persistent storage",
                    public_key, alias
                )
            })?;

        Ok(public_key)
    }

    pub async fn set_current_key(&mut self, index: usize) -> Result<&Key> {
        // first we attempt to select the key
        let key = self
            .keys
            .keys_mut()
            .get_mut(index)
            .ok_or_else(|| anyhow!("No key for the given index {}", index))?;

        if self.current_key != Some(index) {
            if let Some(key) = key.key() {
                self.network
                    .connect(
                        &mut self.rng,
                        self.config.remote_address,
                        self.config.remote_id,
                        key,
                    )
                    .await;
            }
        }

        // only set the index of the current key once we know we have such key available
        self.current_key = Some(index);

        Ok(key)
    }

    pub async fn set_current_passport(&mut self, hash: Option<Hash>) -> Result<()> {
        let new = hash != self.current_passport;
        if let Some(hash) = hash {
            if self.passports.get_by_id(&hash).is_some() {
                // query the upstream node about the newly selected passport
                // so we have an idea where the network is at in term of
                // understanding our passport
                //
                if new {
                    self.network.send_message(Message::new_get_passport(hash));
                }

                // then upon receiving the passport we can check the current
                // local status. If we were missing blocks: lucky us, if the
                // node is missing blocks we can send them and notify the
                // network about the changes
            }
        }

        self.current_passport = hash;
        Ok(())
    }

    pub async fn process_network_input(&mut self) -> Result<()> {
        use asmtp_network::MessageType;

        if let Some(message) = self.network.receive_message() {
            match message.message_type() {
                MessageType::Gossip => {
                    self.process_gossip(
                        message
                            .gossip_checked()
                            .expect("already know it is a gossip"),
                    );
                }
                MessageType::Topic => {
                    self.process_topic(
                        message
                            .topic_checked()
                            .expect("already know it is a topic message"),
                    )
                    .await?;
                }
                MessageType::PutPassport => {
                    self.process_put_passport(
                        message
                            .put_passport_checked()
                            .expect("already know it is a put passport"),
                    )
                    .await?;
                }
                MessageType::GetPassport => {
                    self.process_get_passport(
                        message
                            .get_passport_checked()
                            .expect("already know it is a get passport"),
                    );
                }
                MessageType::RegisterTopic => {
                    self.process_register_topic(
                        message
                            .register_topic_checked()
                            .expect("already know it is a register topic"),
                    );
                }
                MessageType::DeregisterTopic => {
                    self.process_deregister_topic(
                        message
                            .deregister_topic_checked()
                            .expect("already know it is a deregister topic"),
                    );
                }
                MessageType::QueryTopicMessages => {
                    self.process_query_topic(
                        message
                            .query_topic_messages_checked()
                            .expect("already know it is a query topic"),
                    );
                }
            }
        }

        Ok(())
    }

    fn process_gossip(&mut self, _gossip: GossipSlice) {
        // we are not expecting to receive gossips
    }

    async fn process_topic(&mut self, topic_msg: (Topic, &[u8])) -> Result<()> {
        let (topic, message) = topic_msg;

        self.storage.new_message(&topic, message).await?;
        Ok(())
    }

    async fn process_put_passport(
        &mut self,
        passport: (Hash, PassportBlocksSlice<'_>),
    ) -> Result<()> {
        let (id, blocks) = passport;
        let passport = PassportImporter::from_blocks(blocks.iter())?;

        ensure!(
            id == passport.id(),
            "The passport's ID does not match the received id"
        );

        if let Some(p) = self.passports.get_by_id(&id) {
            #[allow(clippy::comparison_chain)]
            // todo: here we want to do something a bit more involve to compare
            //       the both chain as we don't want to lose previously interesting
            //       blocks
            if p.blocks().len() < blocks.len() {
                self.network
                    .send_message(Message::new_put_passport(id, p.blocks()));
            } else if p.blocks().len() > blocks.len() {
                // todo: update blocks of passport
            }
        } else {
            self.storage.new_passport(blocks).await?;
        }

        Ok(())
    }

    fn process_get_passport(&mut self, passport: Hash) {
        if let Some(passport) = self.passports.get_by_id(&passport) {
            let blocks = passport.blocks();
            self.network
                .send_message(Message::new_put_passport(passport.id(), blocks));
        }
    }

    fn process_register_topic(&mut self, _topic: Topic) {
        // we are not accepting registering topics from the network
    }

    fn process_deregister_topic(&mut self, _topic: Topic) {
        // we are not accepting de-registering topics from the network
    }

    fn process_query_topic(&mut self, _topic_time: (Topic, Time)) {
        // we are not going to handle peer node to query messages on a topic from our node
    }

    pub fn current_key(&self) -> Option<&Key> {
        let index = self.current_key?;
        self.keys.keys().get(index)
    }

    pub fn current_key_mut(&mut self) -> Option<&mut Key> {
        let index = self.current_key?;
        self.keys.keys_mut().get_mut(index)
    }

    pub fn get_current_passport(&self) -> Option<&Passport> {
        let hash = self.current_passport.as_ref()?;
        self.passports.get_by_id(hash)
    }

    pub async fn create_new_passport(&mut self, passphrase: Seed) -> Result<Hash> {
        let key = self
            .current_key_mut()
            .ok_or_else(|| anyhow!("You cannot create a passport without a key"))?;
        let alias = key
            .alias()
            .ok_or_else(|| anyhow!("Key needs to have an alias"))?
            .to_owned();
        let author = key
            .key()
            .ok_or_else(|| anyhow!("Key needs to have access to the secret key too"))?
            .clone();

        let passport =
            keynesis::passport::Passport::create(&mut self.rng, &alias, &author, passphrase)?;

        let passport_id = self
            .storage
            .new_passport(passport.blocks())
            .await
            .context("Failed to save the passport's block to db storage")?;
        self.storage
            .link_key_to_passport(&author.public_key(), &passport_id)
            .await
            .context("Failed to link key to passport ID")?;

        let passport = Passport::new(passport);

        // notify the remote peer of a newly created passport
        self.network
            .send_message(Message::new_put_passport(passport_id, passport.blocks()));

        self.passports.insert(passport);

        Ok(passport_id)
    }
}
