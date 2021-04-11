use anyhow::{bail, ensure, Context, Result};
use asmtp_lib::Entropy;
use keynesis::{
    key::ed25519::{PublicKey, SecretKey},
    Seed,
};
use serde::{ser::Serializer, Deserialize, Deserializer, Serialize};
use std::{
    fmt::{self, Formatter},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const DEFAULT_KEY_TIMEOUT: Duration = Duration::from_secs(60 * 30);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum KeyFile {
    /// store the [`Seed`] on the device in clear text
    ///
    /// While this is the most convenient it is not necessarily the most
    /// secure. However it does not mean it is a bad choice. In most
    /// cases if you trust your computer's security to already be
    /// at the maximum this option should be enough, not optimal, but enough
    ///
    /// If an adversary can access your computer's hard drive it is likely that
    /// you have other problems to address (your HD should have been encrypted
    /// and you should have had better firewall and anti-virus protections...)
    ///
    Seed {
        /// the [`Seed`] will be stored in clear text
        ///
        /// The [`Seed`] will then be used to retrieve the [`SecretKey`]
        #[serde(
            serialize_with = "serialize_seed",
            deserialize_with = "deserialize_seed"
        )]
        seed: Seed,
    },
    /// store the [`Seed`] for the [`SecretKey`] in 2 separate components.
    ///
    /// * an entropy in clear text on the disk
    /// * a passphrase the users need to remember
    ///
    /// This is a higher security level than the [`KeyConfig::Seed`] option.
    /// Also there is no passphrase authentication. So it is not possible
    /// to brute force the passphrase by just hacking the encryption of the
    /// secret key. This allows users and applications to leverage plausible
    /// deniability.
    ///
    /// Conveniently once the passphrase has been asked, it will be kept
    /// as such for the given `timeout` duration. A longer time might be
    /// more convenient to prevent the users to enter the passphrase often.
    EntropyWithPassword {
        /// the [`Entropy`] in clear text
        ///
        /// a passphrase will be necessary to derive the [`Seed`].
        /// The [`Seed`] will then be used to retrieve the [`SecretKey`]
        entropy: Entropy,
        /// Duration to keep the secret key in memory for. This will not
        /// delete the [`Entropy`]. It is simply the duration before asking
        /// for the user's passphrase again.
        ///
        /// the default is 30minutes. After 30minute the application will
        /// require the password to be entered again
        #[serde(default = "default_key_timeout")]
        timeout: Duration,
    },
}

pub struct Key {
    filepath: PathBuf,
    config: KeyFile,
    key: Option<SecretKey>,
    public_key: Option<PublicKey>,
    alias: Option<String>,
    last_used: Instant,
}

impl Key {
    fn new<P>(filepath: P, config: KeyFile) -> Self
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref().to_path_buf();
        let last_used = Instant::now();

        let key = match &config {
            // if we have a `Seed` we can already load the secret key in
            // memory, no need to waste time or make the UX more complicated
            KeyFile::Seed { seed } => Some(SecretKey::new(seed.clone().into_rand_chacha())),
            // here we will need to ask the user for the Passphrase
            // so better wait until it is actually needed
            KeyFile::EntropyWithPassword { .. } => None,
        };

        let public_key = key.as_ref().map(|k| k.public_key());

        Self {
            filepath,
            config,
            key,
            alias: None,
            public_key,
            last_used,
        }
    }

    pub fn create<P>(filepath: P, config: KeyFile) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();
        ensure!(!filepath.is_file());

        fs::write(
            filepath,
            serde_yaml::to_string(&config).context("Cannot serialize the key configuration")?,
        )
        .context("Failed to save new key on the configuration file")?;

        Ok(Self::new(filepath, config))
    }

    pub fn open<P>(filepath: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let filepath = filepath.as_ref();

        if filepath.is_file() {
            let s = fs::read_to_string(&filepath).context("Failed to read Key File's content")?;
            let config = serde_yaml::from_str(&s).context("Invalid Key File")?;

            Ok(Self::new(filepath, config))
        } else if filepath.exists() {
            bail!("not a valid key file: {}", filepath.display())
        } else {
            bail!("No key file found")
        }
    }

    fn set_secret_key(&mut self, key: SecretKey) -> PublicKey {
        let pk = key.public_key();
        self.key = Some(key);
        self.last_used = Instant::now();
        pk
    }

    pub fn passphrase<T>(&mut self, passphrase: T) -> Result<PublicKey>
    where
        T: AsRef<[u8]>,
    {
        match &self.config {
            KeyFile::Seed { .. } => bail!("Passphrase was not needed"),
            KeyFile::EntropyWithPassword { entropy, .. } => {
                let mut key = [0; 32];
                keynesis::hash::Blake2b::blake2b(&mut key, passphrase.as_ref(), &[]);
                let seed = Seed::derive_from_key(entropy.as_ref(), key);
                let key = SecretKey::new(&mut seed.into_rand_chacha());
                Ok(self.set_secret_key(key))
            }
        }
    }

    pub fn key_timedout(&self) -> bool {
        let elapsed = match &self.config {
            KeyFile::EntropyWithPassword { timeout, .. } => self.last_used.elapsed() > *timeout,
            KeyFile::Seed { .. } => false,
        };
        self.key.is_none() || elapsed
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    pub fn set_alias<S>(&mut self, alias: S)
    where
        S: Into<String>,
    {
        self.alias = Some(alias.into())
    }

    pub fn config(&self) -> &KeyFile {
        &self.config
    }

    pub fn last_used(&self) -> Instant {
        self.last_used
    }

    pub fn public_key(&self) -> Option<&PublicKey> {
        self.public_key.as_ref()
    }

    pub fn key(&mut self) -> Option<&SecretKey> {
        if self.key_timedout() {
            self.key = None;
            None
        } else {
            self.key.as_ref()
        }
    }
}

fn serialize_seed<S>(seed: &Seed, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    seed.to_string().serialize(serializer)
}

fn deserialize_seed<'de, D>(deserializer: D) -> Result<Seed, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as _;

    let s = String::deserialize(deserializer)?;
    s.parse().map_err(D::Error::custom)
}

const fn default_key_timeout() -> Duration {
    DEFAULT_KEY_TIMEOUT
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Key")
            .field("filepath", &self.filepath)
            .field("config", &self.config)
            .field("last_used", &self.last_used)
            .field("public_key", &self.public_key)
            .field("key", &"...")
            .finish()
    }
}
