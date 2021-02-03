use anyhow::{bail, Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    convert::TryFrom,
    fmt::{self, Formatter},
    hash::{Hash, Hasher},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, Hash, Clone, StructOpt, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// path to the persistent storage file
    ///
    /// if no existing file is found, a new storage file will be created
    #[structopt(long = "storage-path")]
    pub path: PathBuf,

    /// default mode for the persistent storage (high throughput vs low space)
    #[serde(default)]
    #[structopt(long = "storage-mode", default_value = DEFAULT_MODE, possible_values = AVAILABLE_MODES)]
    pub mode: Mode,

    /// print the sled's storage profile performance on drop (at shutdown)
    ///
    #[serde(default)]
    #[structopt(long = "storage-profile-on-drop", hidden = true)]
    pub profile_on_drop: bool,

    /// number of minutes to store gossips in the persistent storage
    ///
    /// this will set the storage rate refresh of the current state
    /// of the "view" gossips. This will allow to have a good set of
    /// nodes early on in between restart
    #[serde(default = "default_gossips_refresh_rate")]
    #[structopt(long = "gossip-refresh-rate", parse(try_from_str = duration))]
    pub gossip_refresh_rate: Duration,

    /// use `zstd` compression of the storage with the given compression
    /// factor.
    ///
    /// if no value is given, no compression will happen. Valid values are
    /// from 1 to 22 (levels >= 20 are 'ultra')
    #[serde(default)]
    #[structopt(long = "storage-compression-factor", hidden = true)]
    pub compression_factor: Option<i32>,

    /// the passport cache size
    ///
    /// this is the number of fully restored passport to keep in memory
    #[serde(default = "default_passport_cache_size")]
    #[structopt(long = "storage-passport-cache-size", default_value = "256")]
    pub passport_cache_size: usize,
}

const AVAILABLE_MODES: &[&str] = &[Mode::HIGH_THROUGHPUT, Mode::LOW_SPACE];
const DEFAULT_MODE: &str = Mode::HIGH_THROUGHPUT;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Mode(sled::Mode);

impl Mode {
    pub const HIGH_THROUGHPUT: &'static str = "high-throughput";
    pub const LOW_SPACE: &'static str = "low-space";

    fn as_str(&self) -> &str {
        match &self.0 {
            sled::Mode::HighThroughput => Self::HIGH_THROUGHPUT,
            sled::Mode::LowSpace => Self::LOW_SPACE,
        }
    }
}

impl PartialEq<Self> for Mode {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(other.as_str())
    }
}
impl Eq for Mode {}
impl PartialOrd<Self> for Mode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
impl Ord for Mode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl Hash for Mode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self(sled::Mode::HighThroughput)
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl FromStr for Mode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::HIGH_THROUGHPUT => Ok(Self(sled::Mode::HighThroughput)),
            Self::LOW_SPACE => Ok(Self(sled::Mode::LowSpace)),
            unknown => bail!("Unknown storage mode value: {}", unknown),
        }
    }
}

impl Into<String> for Mode {
    fn into(self) -> String {
        self.as_str().to_owned()
    }
}

impl TryFrom<String> for Mode {
    type Error = <Self as FromStr>::Err;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}

impl Into<sled::Mode> for Mode {
    fn into(self) -> sled::Mode {
        self.0
    }
}
