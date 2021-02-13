use anyhow::{Context as _, Result};
use keynesis::{
    key::ed25519::{PublicKey, SecretKey},
    passport::{
        block::{Block, BlockSlice},
        LightPassport, Passport,
    },
    Seed,
};
use rand::rngs::OsRng;
use serde::{
    de::{Deserializer, Error as _},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::path::Path;

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

    #[serde(default)]
    remote: String,

    #[serde(
        serialize_with = "blocks_serialize",
        deserialize_with = "blocks_deserialize",
        default
    )]
    blocks: Vec<Block>,
}

#[derive(Default, Clone)]
pub struct State {
    key: Option<SecretKey>,
    passport: Option<LightPassport>,

    file: StateFile,
}

impl State {
    fn new_with(mut file: StateFile) -> Result<Self> {
        if file.seed.is_none() {
            file.seed = Some(Seed::generate(&mut OsRng));
        }

        let key = if let Some(seed) = file.seed.clone() {
            Some(SecretKey::new(&mut seed.into_rand_chacha()))
        } else {
            None
        };

        let mut blocks = file.blocks.iter().map(|b| b.as_slice());
        let passport = if let Some(head) = blocks.next() {
            let mut passport =
                LightPassport::new(head).context("Invalid block in the passport's file")?;
            for block in blocks {
                passport
                    .update(block)
                    .context("Invalid block in the passport's file")?;
            }
            Some(passport)
        } else {
            None
        };

        Ok(Self {
            key,
            passport,
            file,
        })
    }

    pub fn create_passport(&mut self, passphrase: Seed) -> anyhow::Result<()> {
        let mut rng = Seed::generate(&mut OsRng).into_rand_chacha();
        let author = self.key.as_ref().unwrap();
        let alias = self.alias();

        let passport = Passport::create(&mut rng, alias, author, passphrase)
            .context("Failed to create passport")?;

        let blocks = passport.blocks().iter().cloned().collect();

        self.file.blocks = blocks;

        Ok(())
    }

    pub fn load<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let state_file = if path.as_ref().is_file() {
            serde_json::from_reader(
                std::fs::OpenOptions::new()
                    .read(true)
                    .truncate(false)
                    .write(false)
                    .append(false)
                    .create(false)
                    .open(path.as_ref())
                    .with_context(|| {
                        format!("Failed to open config file: {}", path.as_ref().display())
                    })?,
            )
            .with_context(|| format!("Failed to read settings from {}", path.as_ref().display()))?
        } else {
            StateFile::default()
        };

        Self::new_with(state_file).with_context(|| {
            format!(
                "Failed to load the application state from config file: {}",
                path.as_ref().display()
            )
        })
    }

    pub fn save<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        serde_json::to_writer_pretty(
            std::fs::OpenOptions::new()
                .append(false)
                .truncate(true)
                .write(true)
                .read(false)
                .create(true)
                .open(path.as_ref())
                .with_context(|| {
                    format!("Failed to open config file: {}", path.as_ref().display())
                })?,
            &self.file,
        )
        .with_context(|| {
            format!(
                "Cannot save application state to {}",
                path.as_ref().display()
            )
        })
    }

    pub fn alias(&self) -> &str {
        self.file.alias.as_str()
    }

    pub fn set_alias(&mut self, alias: impl AsRef<str>) {
        self.file.alias = alias.as_ref().to_owned();
    }

    pub fn remote(&self) -> &str {
        self.file.remote.as_str()
    }

    pub fn public_key(&self) -> Option<PublicKey> {
        self.key.as_ref().map(|k| k.public_key())
    }

    pub fn light_passport(&self) -> Option<&LightPassport> {
        self.passport.as_ref()
    }

    pub fn has_alias(&self) -> bool {
        !self.file.alias.is_empty()
    }

    pub fn has_remote(&self) -> bool {
        !self.file.remote.is_empty()
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    pub fn has_passport(&self) -> bool {
        self.passport.is_some()
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

fn blocks_serialize<S>(blocks: &Vec<Block>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let blocks: Vec<_> = blocks
        .into_iter()
        .map(|b| hex::encode(b.as_ref()))
        .collect();

    blocks.serialize(serializer)
}

fn blocks_deserialize<'de, D>(deserializer: D) -> Result<Vec<Block>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Vec::<String>::deserialize(deserializer)?;

    let mut blocks = Vec::with_capacity(s.len());

    for s in s {
        let bytes = hex::decode(s).map_err(D::Error::custom)?;

        let block = BlockSlice::try_from_slice(&bytes)
            .map_err(D::Error::custom)?
            .to_block();
        blocks.push(block);
    }

    Ok(blocks)
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("key", &self.key)
            .field("file", &self.file)
            .finish()
    }
}
