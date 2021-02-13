use crate::Settings;
use anyhow::{Context as _, Result};
use asmtp_lib::{PassportBlocks, PassportImporter};
use asmtp_storage::{Buddies, Buddy, Messages, Passports};
use keynesis::{
    key::ed25519::{PublicKey, SecretKey},
    passport::{block::Hash, Passport},
    Seed,
};
use rand::rngs::OsRng;
use serde::{
    de::{Deserializer, Error as _},
    ser::Serializer,
    Deserialize, Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StateFile {
    #[serde(default)]
    alias: String,

    #[serde(
        serialize_with = "seed_serialize",
        deserialize_with = "seed_deserialize",
        default
    )]
    seed: Option<Seed>,
}

#[derive(Clone)]
pub struct State {
    key: Option<SecretKey>,
    passport: Option<Passport>,

    passports: Passports,
    buddies: Buddies,
    messages: Messages,
    settings: Settings,
    file: StateFile,

    db: sled::Db,
}

impl State {
    fn new_with(settings: Settings, mut file: StateFile) -> Result<Self> {
        if file.seed.is_none() {
            file.seed = Some(Seed::generate(&mut OsRng));
        }

        let key = if let Some(seed) = file.seed.clone() {
            Some(SecretKey::new(&mut seed.into_rand_chacha()))
        } else {
            None
        };

        let sled_config = sled::Config::new().path(settings.db_file());
        let sled_db = sled_config.open().with_context(|| {
            format!(
                "Cannot open persistent storage file: {}",
                settings.db_file().display()
            )
        })?;
        let passports = Passports::new(&sled_db)
            .context("Failed to load the ASMTP local persistent storage")?;

        let passport = if let Some(id) = key.as_ref().map(|k| k.public_key()) {
            let l = passports.get(id)?;
            if let Some(id) = l.map(|p| p.id()) {
                let p = passports
                    .get_blocks(id)
                    .with_context(|| format!("Cannot retrieve a passport for the key {}", id))?;

                Some(PassportImporter::from_blocks_owned(
                    p.iter().map(|p| p.to_block()),
                )?)
            } else {
                None
            }
        } else {
            None
        };

        let buddies =
            Buddies::new(&sled_db).context("Failed to open the ASMTP local persistent storage")?;
        let messages = Messages::open(&sled_db)
            .context("Failed to open the ASMTP local persistent storage")?;

        Ok(Self {
            settings,
            key,
            passport,
            passports,
            messages,
            buddies,
            file,

            db: sled_db,
        })
    }

    pub fn create_passport(&mut self, passphrase: Seed, buddy_name: Buddy) -> anyhow::Result<()> {
        let mut rng = Seed::generate(&mut OsRng).into_rand_chacha();
        let author = self.key.as_ref().unwrap();
        let alias = self.alias();

        let passport = keynesis::passport::Passport::create(&mut rng, alias, author, passphrase)
            .context("Failed to create passport")?;

        let blocks: PassportBlocks<Vec<u8>> = passport.blocks().iter().cloned().collect();

        let _ = self
            .passports
            .put_passport(blocks.as_slice())
            .context("Failed to persistently save the local passport")?;
        self.passport = Some(passport);
        let id = self.id().unwrap();
        self.buddies
            .insert(&buddy_name, id)
            .context("Failed to register the alias name")?;

        Ok(())
    }

    pub fn load(settings: Settings) -> anyhow::Result<Self> {
        let config_file = settings.config_file();
        let state_file = if config_file.is_file() {
            serde_json::from_reader(
                std::fs::OpenOptions::new()
                    .read(true)
                    .truncate(false)
                    .write(false)
                    .append(false)
                    .create(false)
                    .open(&config_file)
                    .with_context(|| {
                        format!("Failed to open config file: {}", config_file.display())
                    })?,
            )
            .with_context(|| format!("Failed to read settings from {}", config_file.display()))?
        } else {
            StateFile::default()
        };

        Self::new_with(settings, state_file).with_context(|| {
            format!(
                "Failed to load the application state from config file: {}",
                config_file.display()
            )
        })
    }

    pub fn key(&self) -> Option<&SecretKey> {
        self.key.as_ref()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn blocks(&self) -> PassportBlocks<Vec<u8>> {
        self.passports.get_blocks(self.id().unwrap()).unwrap()
    }

    pub fn passports(&self) -> &Passports {
        &self.passports
    }

    pub fn buddies(&self) -> &Buddies {
        &self.buddies
    }

    pub fn messages(&self) -> &Messages {
        &self.messages
    }

    pub fn save(&self) -> Result<()> {
        self.db
            .flush()
            .context("Failed to store all buffer of the persistent storage")?;
        let config_file = self.settings.config_file();
        serde_json::to_writer_pretty(
            std::fs::OpenOptions::new()
                .append(false)
                .truncate(true)
                .write(true)
                .read(false)
                .create(true)
                .open(&config_file)
                .with_context(|| {
                    format!("Failed to open config file: {}", config_file.display())
                })?,
            &self.file,
        )
        .with_context(|| format!("Cannot save application state to {}", config_file.display()))
    }

    pub fn alias(&self) -> &str {
        self.file.alias.as_str()
    }

    pub fn set_alias(&mut self, alias: impl AsRef<str>) {
        self.file.alias = alias.as_ref().to_owned();
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.key.as_ref().map(|k| k.public_key())
    }

    pub fn passport(&self) -> Option<&Passport> {
        self.passport.as_ref()
    }

    pub fn db(&self) -> &sled::Db {
        &self.db
    }

    pub fn has_alias(&self) -> bool {
        !self.file.alias.is_empty()
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    pub fn has_passport(&self) -> bool {
        self.passport.is_some()
    }

    pub fn id(&self) -> Option<Hash> {
        // TODO: replace with passport.id() instead
        self.passport
            .as_ref()
            .map(|p| p.blocks()[0].header().hash())
    }
}

fn seed_serialize<S>(seed: &Option<Seed>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    seed.as_ref()
        .map(|seed| seed.to_string())
        .serialize(serializer)
}

fn seed_deserialize<'de, D>(deserializer: D) -> Result<Option<Seed>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;

    if let Some(s) = s {
        match s.parse() {
            Ok(seed) => Ok(Some(seed)),
            Err(error) => Err(D::Error::custom(error)),
        }
    } else {
        Ok(None)
    }
}
