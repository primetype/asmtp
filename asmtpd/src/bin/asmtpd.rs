use anyhow::Context as _;
use asmtpd::{network::Network, secret::Secret, storage::Storage, Config};
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
    #[structopt(long = "log-level", default_value = "info", global = true)]
    log_level: Level,

    /// path of the configuration file of the server
    #[structopt(long = "config")]
    config: PathBuf,

    /// set the password instead of having the problem prompted for it
    #[structopt(long = "password", env = "ASMTPD_KEY_PASSWORD", hide_env_values = true)]
    password: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = main_run().await {
        eprintln!("{:?}", error);
        std::process::exit(1);
    } else {
        ()
    }
}

async fn main_run() -> anyhow::Result<()> {
    let args = Args::from_args();

    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(args.log_level)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed")?;

    let mut config = Config::from_file(args.config).context("cannot load initial settings")?;
    config.secret.password = args.password;

    let secret = Secret::new(config.secret).context("Cannot start the secret Key Manager")?;
    let storage = Storage::new(config.storage, config.users).context("Cannot load storage")?;
    let network = Network::new(secret.clone(), storage.clone(), config.network)
        .await
        .context("Cannot load the network task")?;

    println!("ctrl-c to stop the node...");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shuting down via CTRL-C instruction")
        }
    }

    network
        .shutdown()
        .await
        .context("Cannot shutdown the network task")?;

    // give an extra 200ms for the system to stop the associated tasks properly
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(())
}
