use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, Hash, Clone, StructOpt, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// path to the persistent storage file
    ///
    /// if no existing file is found, a new storage file will be created
    #[structopt(long = "storage-path")]
    pub path: PathBuf,

    /// number of minutes to store gossips in the persistent storage
    ///
    /// this will set the storage rate refresh of the current state
    /// of the "view" gossips. This will allow to have a good set of
    /// nodes early on in between restart
    #[serde(default = "default_gossips_refresh_rate")]
    #[structopt(long = "gossip-refresh-rate", parse(try_from_str = duration))]
    pub gossip_refresh_rate: Duration,

    /// the passport cache size
    ///
    /// this is the number of fully restored passport to keep in memory
    #[serde(default = "default_passport_cache_size")]
    #[structopt(long = "storage-passport-cache-size", default_value = "256")]
    pub passport_cache_size: usize,
}

fn default_passport_cache_size() -> usize {
    256
}

fn default_gossips_refresh_rate() -> Duration {
    Duration::from_secs(30)
}

fn duration(s: &str) -> Result<Duration> {
    let i: u64 = s
        .parse()
        .context("expecting to parse a duration in minutes")?;
    Ok(Duration::from_secs(i * 60))
}
