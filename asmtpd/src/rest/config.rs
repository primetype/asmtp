use crate::rest::api::config::Cors;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use structopt::StructOpt;

pub(crate) const DEF_SESSIONS_MAX_ACTIVE: &str = "10000";
pub(crate) const DEF_SESSIONS_MAX_IDLE: &str = "1800";
pub(crate) const DEF_SESSIONS_MAX_LIFESPAN: &str = "7200";

#[derive(StructOpt, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// the address and port number to listen to for HTTP queries
    /// to the public API
    ///
    #[serde(default = "default_listen")]
    #[structopt(long = "rest-listen", default_value = "127.0.0.1:8080")]
    pub listen: SocketAddr,

    /// set the Cross-Origin Resource Sharing settings
    #[serde(default)]
    #[structopt(skip)]
    pub cors: Cors,

    #[serde(default)]
    #[structopt(skip)]
    pub state: SessionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, StructOpt)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SessionConfig {
    /// SESSIONS - max number of live sessions in the session state
    #[structopt(long, env = "ASMTPD_SESSIONS_MAX_ACTIVE", default_value = DEF_SESSIONS_MAX_ACTIVE)]
    pub max_active_sessions: usize,
    /// SESSIONS - number of seconds since session has been last used
    #[structopt(long, env = "ASMTPD_SESSIONS_MAX_IDLE", default_value = DEF_SESSIONS_MAX_IDLE)]
    pub max_idle: u64,
    /// SESSIONS - number of seconds since session has been created
    #[structopt(long, env = "ASMTPD_SESSIONS_MAX_LIFESPAN", default_value = DEF_SESSIONS_MAX_LIFESPAN)]
    pub max_lifespan: u64,
}

fn default_listen() -> SocketAddr {
    "127.0.0.1:8080".parse().unwrap()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            cors: Default::default(),
            state: SessionConfig::default(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_active_sessions: 10_000,
            max_idle: 1800,         // 30min max of idle
            max_lifespan: 2 * 3600, // 2 hours max of lifespan
        }
    }
}
