pub mod config;
mod connections;
mod topology;

pub use self::config::Config;
use self::{connections::Connections, topology::Topology};
use crate::{secret::Secret, storage::Storage};
use anyhow::{anyhow, bail, Context as _, Result};
use asmtp_network::net::Listener;
use bytes::Bytes;
use indexmap::IndexSet;
use keynesis::{hash::Blake2b, key::ed25519};
use lru::LruCache;
use poldercast::{layer::Selection, Topic};
use rand::{rngs::OsRng, RngCore as _};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};

pub struct Network {
    command: mpsc::Sender<Command>,
    handle: JoinHandle<Result<()>>,
}

struct Runner {
    topology: Topology,
    storage: Storage,
    connections: Connections,
    listener: Listener,
    command: mpsc::Receiver<Command>,
    config: Config,
    id: ed25519::PublicKey,
}

struct GossipCache {
    queue_size: usize,
    min_elapsed: Duration,
    will_gossip: IndexSet<ed25519::PublicKey>,
    has_gossiped: LruCache<ed25519::PublicKey, Instant>,
}

enum Command {
    Shutdown,
    Subscriptions { add: Vec<Topic>, remove: Vec<Topic> },
}

pub struct MessageCache {
    max: usize,
    messages: IndexSet<[u8; 32]>,
}

impl MessageCache {
    pub fn new(config: &Config) -> Self {
        Self {
            max: config.known_message_cache_size,
            messages: IndexSet::with_capacity(config.known_message_cache_size),
        }
    }

    pub fn check(&mut self, obj: impl AsRef<[u8]>) -> bool {
        let obj = obj.as_ref();
        let mut hash = [0; 32];
        Blake2b::blake2b(&mut hash, obj, &[]);

        while self.messages.len() >= self.max {
            self.messages.pop();
        }

        self.messages.insert(hash)
    }
}

impl GossipCache {
    fn new(config: &Config) -> Self {
        Self {
            queue_size: config.gossiping.queue_size,
            min_elapsed: config.gossiping.minimum_time_elapsed,
            will_gossip: IndexSet::with_capacity(config.gossiping.queue_size),
            has_gossiped: LruCache::new(config.gossiping.history_size),
        }
    }

    fn is_empty(&self) -> bool {
        self.will_gossip.is_empty()
    }

    fn next_gossip_peer(&mut self) -> Option<ed25519::PublicKey> {
        let id = self.will_gossip.pop()?;

        self.has_gossiped.put(id, Instant::now());

        Some(id)
    }

    fn register_interest(&mut self, id: ed25519::PublicKey) {
        if let Some(last_gossip) = self.has_gossiped.get(&id) {
            if last_gossip.elapsed() < self.min_elapsed {
                // not enough time elapsed, return
                return;
            }
        }

        // we don't want to go beyond the capacity
        // so we don't blow our internal memory working for other nodes
        while self.will_gossip.len() >= self.queue_size {
            self.will_gossip.pop();
        }

        self.will_gossip.insert(id);
    }
}

impl Network {
    pub async fn new(secret: Secret, storage: Storage, config: Config) -> Result<Self> {
        let (command_sender, command_receiver) = mpsc::channel(8);

        let id = secret.as_ref().public_key();

        let listen_address = config.listen_address;
        let public_address = config.public_address;
        tracing::info!(
            listen_address = %listen_address,
            public_address = %public_address,
            "listening for inbound connections"
        );
        let listener = Listener::new(listen_address).await?;
        let topology = Topology::new(public_address, secret.clone());

        // load the initial subscriptions from the storage
        //
        // load the subscriptions first so every layers of the poldercast
        // topology can get the necessary preset data regarding how the
        // subscriptions should look like before adding peers
        let initial_subscriptions = storage.topic_subscriptions()?;
        topology.subscriptions(initial_subscriptions, Vec::new());

        // add the initial gossips from the configuration file
        for gossip in config.known_gossips.iter().cloned() {
            topology.accept_gossip(gossip.0);
        }

        // get all the initial gossips we may already know from the database
        for gossip in storage.known_gossips()? {
            topology.accept_gossip(gossip);
        }

        let runner = Runner {
            topology,
            storage,
            connections: Connections::new(secret, &config),
            listener,
            command: command_receiver,
            config,
            id,
        };

        let handle = tokio::spawn(runner.run());

        Ok(Self {
            command: command_sender,
            handle,
        })
    }

    pub async fn update_subscriptions(&self, add: Vec<Topic>, remove: Vec<Topic>) -> Result<()> {
        self.command
            .send(Command::Subscriptions { add, remove })
            .await
            .map_err(|_| anyhow!("Cannot subscriptions update command to the network"))?;

        Ok(())
    }

    pub async fn shutdown(self) -> Result<()> {
        self.command
            .send(Command::Shutdown)
            .await
            .map_err(|_| anyhow!("Cannot send shutdown command to the network"))?;

        let mut handle = self.handle;

        tokio::select! {
            result = &mut handle => {
                match result {
                    Ok(result) => result,
                    Err(error) => bail!("error while waiting for network to shutdown: {}", error)
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                handle.abort();
                bail!("shutdown timedout, aborting instead...")
            }
        }
    }
}

impl Runner {
    #[tracing::instrument(
        skip(self),
        fields(
            listen_address = %self.config.listen_address,
            public_address = %self.config.public_address,
            id = %self.id,
        ),
        level = "info"
    )]
    async fn run(self) -> Result<()> {
        let Self {
            topology,
            mut storage,
            mut connections,
            listener,
            mut command,
            config,
            id: _,
        } = self;

        let mut gossipers = GossipCache::new(&config);
        let mut known_cache = MessageCache::new(&config);

        // on startup, select the existing profile for registered interest in
        // gossiping with.
        for profile in topology.view_for(None, Selection::Any) {
            gossipers.register_interest(profile.id());
        }

        loop {
            if let Some(id) = gossipers.next_gossip_peer() {
                if let Some(gossiper) = topology.get(&id) {
                    let gossips = topology.gossips_for(&id);
                    tracing::info!(
                        to = %gossiper.id(),
                        num_gossips = gossips.len(),
                        "sending gossips"
                    );
                    if let Err(error) = connections.send_gossips(gossiper, gossips).await {
                        tracing::warn!(reason = %error, peer = %id, "Cannot send gossip to peer")
                    }
                }
            }

            if gossipers.is_empty() {
                let random = topology.view_for(None, Selection::Any);
                if !random.is_empty() {
                    let index = rand::rngs::OsRng.next_u32() as usize % random.len();
                    gossipers.register_interest(random[index].id());
                }
            }

            if storage.needs_update_known_gossips() {
                let view = topology.view_for(None, Selection::Any);
                let gossips = view.iter().map(|p| p.gossip()).cloned().collect();

                // if an error occurs here in the storage we better try to deal with it
                // so this function will returns and make the network stop
                storage.update_known_gossips(gossips)?;
            }

            tokio::select! {
                _ = tokio::time::sleep(config.heart_beat) => {
                    // TODO:
                    // this is an opportunity to start displaying some useful information
                    // such as the number of opened connections or the number of messages
                    // the network sent or received etc...
                    tracing::info!("beat");
                }
                // handle receiving commands
                command = command.recv() => {
                    match command {
                        None => bail!("failed to receive anymore commands"),
                        Some(Command::Shutdown) => {
                            break;
                        }
                        Some(Command::Subscriptions { add, remove }) => {
                            topology.subscriptions(add, remove);
                        }
                    }
                }
                // accept new connections from the listener
                //
                // new connections handshake will run within another task
                // and will be queued until completion in the `accepting_tasks`
                accepting = listener.accept::<_, ed25519::SecretKey>(OsRng) => {
                    let accepting = accepting.context("failed to accept a new connection")?;
                    connections.accept(accepting).await;
                }

                // receiving messages from the connections
                (peer, message) = connections.receive() => {

                    if let Some(gossip) = message.gossip_checked() {
                        gossipers.register_interest(peer);
                        topology.accept_gossip(gossip.to_owned());
                    } else if let Some((topic, content)) = message.topic_checked() {
                        if !known_cache.check(&content) {
                            continue
                        }

                        tracing::debug!(topic = ?topic, "received original message");

                        // propagate the topic message to other services
                        if let Err(error) = storage.handle_incoming_message(topic, Bytes::from(content.to_vec())) {
                            // not forward anything that we find improper here
                            tracing::warn!(reason = %error, "rejecting the message");
                            continue;
                        }

                        let view = topology.view_for(Some(&peer), Selection::Topic { topic });

                        connections.send_all(view, message).await;
                    }
                }
            }
        }

        Ok(())
    }
}
