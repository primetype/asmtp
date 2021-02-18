pub mod config;
mod connections;
mod topology;

pub use self::config::Config;
use self::{connections::Connections, topology::Topology};
use crate::{secret::Secret, storage::Storage};
use anyhow::{anyhow, bail, Context as _, Result};
use asmtp_network::{net::Listener, Message};
use bytes::Bytes;
use indexmap::IndexSet;
use keynesis::{
    hash::Blake2b,
    key::{ed25519, Dh as _},
};
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
    known_cache: MessageCache,
    gossipers: GossipCache,
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

        let id = secret.public();

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
            topology: topology.clone(),
            storage,
            connections: Connections::new(secret, topology, &config),
            known_cache: MessageCache::new(&config),
            gossipers: GossipCache::new(&config),
            listener,
            command: command_receiver,
            config,
            id,
        };

        let handle = tokio::spawn(async move {
            let mut runner = runner;
            runner.run().await
        });

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
    async fn run(&mut self) -> Result<()> {
        // on startup, select the existing profile for registered interest in
        // gossiping with.
        for profile in self.topology.view_for(None, Selection::Any) {
            self.gossipers.register_interest(profile.id());
        }

        loop {
            if let Some(id) = self.gossipers.next_gossip_peer() {
                if let Some(gossiper) = self.topology.get(&id) {
                    let gossips = self.topology.gossips_for(&id);
                    tracing::info!(
                        to = %gossiper.id(),
                        num_gossips = gossips.len(),
                        "sending gossips"
                    );
                    if let Err(error) = self.connections.send_gossips(gossiper, gossips).await {
                        tracing::warn!(reason = %error, peer = %id, "Cannot send gossip to peer")
                    }
                }
            }

            if self.gossipers.is_empty() {
                let random = self.topology.view_for(None, Selection::Any);
                if !random.is_empty() {
                    let index = rand::rngs::OsRng.next_u32() as usize % random.len();
                    self.gossipers.register_interest(random[index].id());
                }
            }

            if self.storage.needs_update_known_gossips() {
                let view = self.topology.view_for(None, Selection::Any);
                let gossips = view.iter().map(|p| p.gossip()).cloned().collect();

                // if an error occurs here in the storage we better try to deal with it
                // so this function will returns and make the network stop
                self.storage.update_known_gossips(gossips)?;
            }

            tokio::select! {
                _ = tokio::time::sleep(self.config.heart_beat) => {
                    self.beat()
                }
                // handle receiving commands
                command = self.command.recv() => {
                    let stop = self.handle_command(command).await?;
                    if stop { break; }
                }
                // accept new connections from the listener
                //
                // new connections handshake will run within another task
                // and will be queued until completion in the `accepting_tasks`
                accepting = self.listener.accept::<_, ed25519::SecretKey>(OsRng) => {
                    let accepting = accepting.context("failed to accept a new connection")?;
                    self.connections.accept(accepting).await;
                }

                // receiving messages from the connections
                (peer, message) = self.connections.receive() => {
                    if let Err(error) = self.handle_message(peer, message).await {
                        tracing::warn!(reason = %error, peer = %peer, "failed to handle peer's message")
                    }
                }
            }
        }

        Ok(())
    }

    fn beat(&self) {
        let number_connections = self.connections.number_connections();

        // TODO:
        // this is an opportunity to start displaying some useful information
        // such as the number of opened connections or the number of messages
        // the network sent or received etc...
        tracing::info!(number_connections, "beat");
    }

    async fn handle_command(&mut self, command: Option<Command>) -> Result<bool> {
        match command {
            None => bail!("failed to receive anymore commands"),
            Some(Command::Shutdown) => Ok(true),
            Some(Command::Subscriptions { add, remove }) => {
                self.topology.subscriptions(add, remove);
                Ok(false)
            }
        }
    }

    async fn handle_message(&mut self, peer: ed25519::PublicKey, message: Message) -> Result<()> {
        tracing::debug!(message = ?message.message_type(), peer = %peer, "Handling incoming message");

        // ********************************************************************
        //
        // public operations that may happen from any node
        //
        // though we might want to perform some checks and evaluate
        // how peers are behaving so we don't end up with nodes flooding
        // our network
        if let Some(gossip) = message.gossip_checked() {
            self.gossipers.register_interest(peer);
            self.topology.accept_gossip(gossip.to_owned());
        } else if let Some((topic, content)) = message.topic_checked() {
            if !self.known_cache.check(&content) {
                return Ok(());
            }
            tracing::debug!(topic = ?topic, "received original message");

            // propagate the topic message to other services
            self.storage
                .handle_incoming_message(topic, Bytes::from(content.to_vec()))?;

            let view = self
                .topology
                .view_for(Some(&peer), Selection::Topic { topic });

            self.connections.send_all(view, message).await;
        } else if let Some((topic, time)) = message.query_topic_messages_checked() {
            if let Some(messages) = self.storage.messages(topic, time)? {
                // todo: here we are blocking the current task by
                // processing all the messages. This is a bit non
                // productive, instead we should do that in a separate
                // threads/task : `task::spawn` and forget with a clone
                for (_, message) in messages {
                    self.connections
                        .send_to_peer(&peer, Message::new_topic(topic, message.as_ref()))
                        .await
                }
            }
        }
        // ********************************************************************
        //
        // the following operations are additions from the poldercast protocol
        // and are used to exchange passport across the network as requested
        //
        else if let Some(id) = message.get_passport_checked() {
            let blocks = self
                .storage
                .handle_get_passport(id)
                .with_context(|| format!("Failed to find passport {}", id))?;
            self.connections
                .send_to_peer(&peer, Message::new_put_passport(id, blocks.as_slice()))
                .await
        } else if let Some((id, slice)) = message.put_passport_checked() {
            // TODO: we need to check that the passport is being *PUT* by a
            // approved peer. or that this is a request passport from a previously
            // sent passport to that peer specifically.
            //

            if let Err(error) = self
                .storage
                .handle_put_passport(peer, id, slice.to_blocks())
            {
                tracing::warn!(peer = %peer, passport = %id, reason = %error, "cannot accept new passport")
            }
        } else if let Some(topic) = message.register_topic_checked() {
            self.storage.put_topic(peer, topic)?
        } else if let Some(topic) = message.deregister_topic_checked() {
            self.storage.remove_topic(peer, topic)?
        }
        // ********************************************************************
        //
        // None of the commands we received are handled by our node
        //
        else {
            bail!("Unknown message type: {:?}", message.message_type());
        }

        Ok(())
    }
}
