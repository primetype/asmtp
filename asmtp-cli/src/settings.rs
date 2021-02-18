use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use keynesis::key::curve25519::PublicKey;
use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct CommandLine {
    /// path directory to the ASMTP local directory
    ///
    /// default is to leave it unset to use the system's project directory:
    ///
    /// * MacOS: ~/Library/Application\ Support/uk.co.primetype.asmtp
    #[structopt(long)]
    pub dir: Option<PathBuf>,

    /// the ASMTP server to contact to sync passports and send messages
    #[structopt(long = "remote-address", env = "ASMTP_CLI_REMOTE_ADDRESS")]
    pub remote_address: SocketAddr,

    /// remote's public key
    #[structopt(long = "remote-id", env = "ASMTP_CLI_REMOTE_ID")]
    pub remote_id: PublicKey,

    /// instead of entering the prompt, export the local passport
    #[structopt(long = "export-passport")]
    pub passport_export: Option<PathBuf>,

    /// instead of entering the prompt, import the given passport
    ///
    /// if the passport exist already it will be updated with the new blocks
    #[structopt(long = "import-passport")]
    pub passport_import: Option<PathBuf>,
}

#[derive(Clone)]
pub struct Settings {
    dirs: ProjectDirs,
    remote_address: SocketAddr,
    remote_id: PublicKey,
    passport_export: Option<PathBuf>,
    passport_import: Option<PathBuf>,
}

impl Settings {
    pub fn gather() -> Result<Self> {
        let cli = CommandLine::from_args();

        let remote_address = cli.remote_address;
        let remote_id = cli.remote_id;
        let dirs = if let Some(dir) = cli.dir {
            ProjectDirs::from_path(dir.clone()).ok_or_else(|| {
                anyhow!("Failed to find a project dir with the given path {:?}", dir)
            })?
        } else {
            ProjectDirs::from("uk.co", "primetype", "asmtp").ok_or_else(|| {
                anyhow!("Failed to find a valid HOME directory from the operating system")
            })?
        };

        let passport_export = cli.passport_export;
        let passport_import = cli.passport_import;

        Ok(Self {
            dirs,
            remote_address,
            remote_id,
            passport_export,
            passport_import,
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.dirs.config_dir().join("asmtp.conf")
    }

    pub fn db_file(&self) -> PathBuf {
        self.dirs.data_local_dir().join("db.sled")
    }

    pub fn remote_address(&self) -> SocketAddr {
        self.remote_address
    }

    pub fn remote_id(&self) -> PublicKey {
        self.remote_id
    }

    pub fn import_passport(&self) -> Option<&PathBuf> {
        self.passport_import.as_ref()
    }

    pub fn export_passport(&self) -> Option<&PathBuf> {
        self.passport_export.as_ref()
    }
}
