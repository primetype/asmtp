use anyhow::{Context as _, Result};
use poldercast::{Gossip, GossipSlice};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Gossips {
    gossips: sled::Tree,

    min_gossip_refresh: Duration,
    last_update: Arc<Mutex<Instant>>,
}

impl Gossips {
    pub(crate) fn new(db: &sled::Db, min_gossip_refresh: Duration) -> Result<Self> {
        let gossips = db
            .open_tree("network::gossips")
            .context("Cannot open the gossips sled tree")?;
        Ok(Self {
            gossips,
            min_gossip_refresh,
            last_update: Arc::new(Mutex::new(Instant::now())),
        })
    }

    pub(crate) fn needs_updated(&self) -> bool {
        self.last_update.lock().unwrap().elapsed() > self.min_gossip_refresh
    }

    pub(crate) fn gossips(&self) -> Result<Vec<Gossip>> {
        let mut _locked = self.last_update.lock().unwrap();

        let all = self.gossips.iter();
        let mut gossips = Vec::new();

        for gossip in all {
            let (gossip, _) = gossip?;

            let gossip = GossipSlice::try_from_slice(gossip.as_ref())
                .context("Cannot retrieve gossip from the storage")?
                .to_owned();
            gossips.push(gossip);
        }

        Ok(gossips)
    }

    pub(crate) fn update(&self, gossips: Vec<Gossip>) -> Result<()> {
        let mut locked = self.last_update.lock().unwrap();

        self.gossips.clear()?;

        let mut batch = sled::Batch::default();

        for gossip in gossips {
            batch.insert(gossip.as_ref(), &[]);
        }

        self.gossips
            .apply_batch(batch)
            .context("cannot save the gossips persistently")?;

        *locked = Instant::now();
        Ok(())
    }
}
