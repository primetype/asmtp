use anyhow::{ensure, Context as _, Result};
use asmtp_client::{
    app,
    event::{Event, Events, Key},
    ui,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use keynesis::key::ed25519::PublicKey;
use std::{io::stdout, net::SocketAddr, path::PathBuf, str::FromStr};
use structopt::StructOpt;
use tui::{backend::CrosstermBackend, Terminal};

#[derive(Debug, StructOpt)]
struct Options {
    /// the public remote address of the ASMTPD server
    ///
    // currently we are expecting the socket address though on the longer
    // run we can set a URL to resolve with DNS or other means.
    #[structopt(long = "remote-address", default_value = "86.31.102.125:9800")]
    remote_address: SocketAddr,

    /// the public remote public key (identity)
    ///
    // while this is currently mandatory it should not remain for long. Indeed
    // this value should only be useful when connecting for the first time to
    // the ASMTPd server. Later we should be able to get the passport's updates
    // of the remote servers and be able to connect to any
    #[structopt(
        long = "remote-id",
        default_value = "7353f1e7fb03b2346638b4e2b93f810c84853787970be0844df63cdc9979a01d"
    )]
    remote_id: PublicKey,

    /// directory to use to store all the persistent information
    ///
    // we hide this option though as we will want to use it only for debug purpose
    // and ideally we will want to have users use the default directory
    #[structopt(long = "working-directory", hidden = true)]
    working_directory: Option<PathBuf>,

    /// the default seed to start
    #[structopt(long = "seed", hidden = true)]
    seed: Option<Seed>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = main_().await {
        panic!("{:?}", error)
    }
}

async fn main_() -> Result<()> {
    let options = Options::from_args();

    let config = app::Config {
        directory: options.working_directory,
        remote_address: options.remote_address,
        remote_id: options.remote_id,
    };

    let app = if let Some(seed) = options.seed {
        app::App::with_seed(config, seed.into()).await
    } else {
        app::App::new(config).await
    }
    .context("Failed to initialize the application state")?;

    start_ui(app).await
}

async fn start_ui(mut app: app::App) -> Result<()> {
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let events = Events::new(250);
    let mut ui = ui::Ui::new(terminal.backend(), &app).await;

    loop {
        ui.update(terminal.backend(), &mut app).await?;
        app.process_network_input().await?;

        terminal.draw(|mut f| {
            //
            ui.draw(&mut f);
        })?;

        match events.next().context("Failed to capture event")? {
            Event::Tick => {
                // todo
            }
            Event::Input(input) => {
                if input == Key::Ctrl('c') {
                    break;
                }
                ui.input(input)
            }
        }
    }

    terminal.show_cursor()?;
    close_application()?;

    Ok(())
}

fn close_application() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

#[derive(Debug)]
struct Seed(Vec<u8>);

impl From<Seed> for keynesis::Seed {
    fn from(seed: Seed) -> Self {
        let mut bytes = [0; keynesis::Seed::SIZE];
        bytes[..seed.0.len()].copy_from_slice(&seed.0);

        Self::from(bytes)
    }
}

impl FromStr for Seed {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).context("Expecting hexadecimal encoded bytes")?;
        ensure!(
            bytes.len() <= keynesis::Seed::SIZE,
            "Cannot have a see that long"
        );
        Ok(Self(bytes))
    }
}
