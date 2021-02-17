use anyhow::{Context as _, Result};
use asmtp_lib::Entropy;
use asmtpd::{secret::Secret, Config};
use keynesis::{key::ed25519, Seed};
use poldercast::{Gossip, Subscriptions};
use std::path::PathBuf;
use structopt::StructOpt;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(StructOpt, Debug)]
struct Args {
    /// set log levels
    ///
    /// useful for trying to debug some operations happening
    /// while executing some of the commands
    #[structopt(long = "log-level", default_value = "warn", global = true)]
    log_level: Level,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// print the default configuration to the standard output
    DefaultConfig,

    /// generate a new keypair
    GenerateNewKey {
        /// path of the file to store the entropy in
        ///
        /// if no value is given, this value will be asked during the
        /// generation time
        #[structopt(long = "entropy-output")]
        entropy: Option<PathBuf>,

        /// set the password instead of having the problem prompted for it
        ///
        #[structopt(long = "password", env = "ASMTPD_KEY_PASSWORD", hide_env_values = true)]
        password: Option<String>,
    },

    MakeGossip {
        /// set the password instead of having the problem prompted for it
        ///
        #[structopt(long = "password", env = "ASMTPD_KEY_PASSWORD", hide_env_values = true)]
        password: Option<String>,

        /// path of the configuration file of the server
        #[structopt(long = "config")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::from_args();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(args.log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let result = match args.cmd {
        Command::DefaultConfig => default_config()
            .await
            .context("Cannot generate default configuration"),
        Command::GenerateNewKey { entropy, password } => generate_new_key(entropy, password)
            .await
            .context("Cannot generate new key"),
        Command::MakeGossip { password, config } => make_gossip(password, config)
            .await
            .context("Cannot make gossip"),
    };

    if let Err(error) = result {
        eprintln!("{:#?}", error);
        std::process::exit(1);
    }
}

async fn default_config() -> Result<()> {
    println!("{}", Config::EXAMPLE);
    Ok(())
}

async fn make_gossip(password: Option<String>, config: PathBuf) -> Result<()> {
    let mut config = Config::from_file(config)?;

    let password = if let Some(password) = password {
        tracing::info!("using password from environment or command line parameter");
        password
    } else {
        dialoguer::Password::new()
            .allow_empty_password(false)
            .with_prompt("Enter password")
            .interact()
            .context("Failed gather password")?
    };
    config.secret.password = Some(password);

    let secret =
        Secret::new(config.secret).context("Cannot retrieve the secret to generate the gossip")?;

    println!("address: {}", config.network.public_address);

    let gossip = Gossip::new(
        config.network.public_address,
        secret.secret(),
        Subscriptions::new().as_slice(),
    );

    println!("{:#?}", &gossip);

    println!(
        "Gossip generated: \"{}\"",
        hex::encode(gossip.as_slice().as_ref())
    );

    Ok(())
}

async fn generate_new_key(entropy_output: Option<PathBuf>, password: Option<String>) -> Result<()> {
    println!("Generating new entropy to use as part of the seed for the new key");
    let entropy = Entropy::generate(rand::thread_rng());
    println!("New seed: \"{}\"", entropy);

    let entropy_output = if let Some(entropy_output) = entropy_output {
        entropy_output
    } else {
        let s: String = dialoguer::Input::new()
            .with_prompt("Output file to store the entropy")
            .allow_empty(false)
            .with_initial_text("entropy.txt")
            .interact_text()
            .context("Failed to confirm entropy output file")?;
        PathBuf::from(s)
    };
    tracing::info!(file = ?entropy_output, "writing entropy in file");
    std::fs::write(&entropy_output, entropy.to_string())
        .with_context(|| format!("Cannot write entropy to file: {}", entropy_output.display()))?;

    let password = if let Some(password) = password {
        tracing::info!("using password from environment or command line parameter");
        password
    } else {
        dialoguer::Password::new()
            .with_confirmation(
                "Confirm new password",
                "Password mismatched, put your game together",
            )
            .allow_empty_password(false)
            .with_prompt("Enter new password")
            .interact()
            .context("Failed to confirm new password")?
    };

    println!("Generating new key... this may take some times...");

    let instant = std::time::Instant::now();
    // run the new seed generation
    let seed = Seed::derive_from_key(entropy, password);
    let elapsed = instant.elapsed().as_millis();
    tracing::info!(elapsed = %elapsed, "new seed generated");

    let key = ed25519::SecretKey::new(&mut seed.into_rand_chacha());

    println!("New private key generated successfully");
    println!("Public Identity: {}", key.public_key());

    Ok(())
}
