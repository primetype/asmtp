use anyhow::{Context as _, Result};
use poldercast::GossipSlice;
use serde::{Deserialize, Serialize};
use std::{
    convert::TryFrom,
    fmt::{self, Formatter},
    net::SocketAddr,
    str::FromStr,
    time::Duration,
};
use structopt::StructOpt;

/// network configuration of the node
///
/// set the different values that controls the nodes behavior
#[derive(StructOpt, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// this is the address the network will listen for incoming
    /// connections
    pub listen_address: SocketAddr,

    /// the address that can be used to reach to us
    ///
    /// This is the public IP and port number to reach to this device
    /// allowing the `listen_address` value to be different and to manage
    /// port forwarding and other internal work.
    pub public_address: SocketAddr,

    /// the maximal number of opened connections
    ///
    /// set the maximum value of default connections
    #[serde(default = "default_max_opened_connections")]
    pub max_opened_connections: usize,

    /// the inbound message queue size
    ///
    /// this is the number of entries that can be queued in the
    /// inbound message queue (messages coming from other nodes)
    #[serde(default = "default_message_queue_size")]
    pub message_queue_size: usize,

    /// the maximum size of the cache of known messages
    ///
    /// we are keeping a hash of the messages who passed by
    /// so we don't re-propagate messages we already visited
    #[serde(default = "default_known_message_cache_size")]
    pub known_message_cache_size: usize,

    #[structopt(flatten)]
    #[serde(default)]
    pub gossiping: Gossip,

    /// the heart beat of the network (in seconds).
    ///
    /// make sure to wake up the network every `heart_beat`
    /// so we can perform some _sanity_ operations
    #[structopt(parse(try_from_str = duration))]
    #[serde(default = "default_heart_beat")]
    pub heart_beat: Duration,

    #[serde(default)]
    pub known_gossips: Vec<KnownGossip>,
}

fn default_heart_beat() -> Duration {
    Duration::from_secs(1)
}

fn default_known_message_cache_size() -> usize {
    10_240
}

fn default_message_queue_size() -> usize {
    64
}

fn default_max_opened_connections() -> usize {
    128
}

fn default_gossiping_history_size() -> usize {
    10_240
}

fn default_gossiping_queue_size() -> usize {
    128
}

fn default_gossiping_minimum_time_elapsed() -> Duration {
    Duration::from_secs(30)
}

#[derive(StructOpt, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gossip {
    /// the minimal time to elapsed before we try to gossip
    /// with another node. Value in seconds.
    ///
    /// This is to prevent gossiping with a nodes continuously
    /// so the default value is rather high
    #[structopt(long = "gossiping-minimum-time-elapsed", parse(try_from_str = duration))]
    #[serde(default = "default_gossiping_minimum_time_elapsed")]
    pub minimum_time_elapsed: Duration,

    /// the size of the queue of entries we are about to gossip with
    ///
    /// we are registering the intent of gossiping with a nodes until
    /// it is appropriate for us to do so. We are registering the
    /// interest in a node until it is appropriate
    #[structopt(long = "gossiping-queue-size")]
    #[serde(default = "default_gossiping_queue_size")]
    pub queue_size: usize,

    /// the size of the cache we keep in memory of all the gossiping
    /// events we had with others.
    ///
    /// This is to prevent to continuously send gossips to the same
    /// nodes over and over.
    #[structopt(long = "gossiping-history-size")]
    #[serde(default = "default_gossiping_history_size")]
    pub history_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct KnownGossip(pub(crate) poldercast::Gossip);

fn duration(s: &str) -> Result<Duration> {
    let i = s
        .parse()
        .context("expecting to parse a duration in seconds")?;
    Ok(Duration::from_secs(i))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "[::1]:9876".parse().unwrap(),
            public_address: "[::1]:9876".parse().unwrap(),
            max_opened_connections: default_max_opened_connections(),
            message_queue_size: default_message_queue_size(),
            known_message_cache_size: default_known_message_cache_size(),
            gossiping: Gossip::default(),
            heart_beat: default_heart_beat(),
            known_gossips: Vec::new(),
        }
    }
}

impl Default for Gossip {
    fn default() -> Self {
        Self {
            minimum_time_elapsed: default_gossiping_minimum_time_elapsed(),
            queue_size: default_gossiping_queue_size(),
            history_size: default_gossiping_history_size(),
        }
    }
}

impl From<KnownGossip> for String {
    fn from(known_gossip: KnownGossip) -> Self {
        known_gossip.to_string()
    }
}

impl TryFrom<String> for KnownGossip {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl fmt::Display for KnownGossip {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0.as_ref()))
    }
}

impl FromStr for KnownGossip {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).context("invalid gossip")?;
        let gossip = GossipSlice::try_from_slice(&bytes).context("Invalid gossip")?;
        Ok(KnownGossip(gossip.to_owned()))
    }
}
