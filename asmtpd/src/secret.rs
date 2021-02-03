use anyhow::{Context as _, Result};
use asmtp_lib::Entropy;
use keynesis::{
    key::{
        ed25519::{self, SecretKey},
        Dh, SharedSecret,
    },
    Seed,
};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;

#[derive(Clone)]
pub struct Secret {
    secret: Arc<SecretKey>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize, StructOpt)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// path to the entropy file
    ///
    #[structopt(
        long = "entropy",
        default_value = "entropy.txt",
        env = "ASMTPD_ENTROPY_FILE"
    )]
    #[serde(default = "default_entropy")]
    pub entropy: PathBuf,

    /// password to derive the entropy to the private key
    ///
    /// if not set, the service will ask for it
    #[structopt(long = "password", env = "ASMTPD_PASSWORD", hide_env_values = true)]
    #[serde(skip)]
    #[serde(default)]
    pub password: Option<String>,
}

impl Secret {
    pub fn new(config: Config) -> Result<Self> {
        let entropy: Entropy = std::fs::read_to_string(&config.entropy)
            .with_context(|| format!("Cannot open the entropy file {}", config.entropy.display()))?
            .parse()
            .with_context(|| {
                format!(
                    "Cannot parse the entropy file: {}",
                    config.entropy.display()
                )
            })?;

        let password = if let Some(password) = config.password {
            password
        } else {
            dialoguer::Password::new()
                .allow_empty_password(true)
                .with_prompt("Enter the secret key password")
                .interact()
                .context("Cannot retrieve the entropy password")?
        };

        let seed = Seed::derive_from_key(entropy, password);

        let secret = SecretKey::generate(&mut seed.into_rand_chacha());

        let public_id = secret.public_key();
        tracing::info!(public = %public_id,"secret loaded");

        Ok(Self {
            secret: Arc::new(secret),
        })
    }

    pub fn as_ref(&self) -> &SecretKey {
        &self.secret
    }
}

impl Dh for Secret {
    fn name() -> &'static str {
        <SecretKey as Dh>::name()
    }

    fn generate<RNG>(rng: &mut RNG) -> Self
    where
        RNG: RngCore + CryptoRng,
    {
        Self {
            secret: Arc::new(SecretKey::generate(rng)),
        }
    }

    fn public(&self) -> ed25519::PublicKey {
        self.secret.public_key()
    }

    fn dh(&self, public: &ed25519::PublicKey) -> SharedSecret {
        self.secret.dh(public)
    }
}

fn default_entropy() -> PathBuf {
    PathBuf::from("entropy.txt")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            entropy: default_entropy(),
            password: None,
        }
    }
}
