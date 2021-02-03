use crate::{network::Config, secret::Secret};
use anyhow::{anyhow, bail, Result};
use asmtp_network::{
    net::{Accepting, Connection, ConnectionReader, ConnectionWriter},
    Message,
};
use futures::prelude::*;
use keynesis::key::ed25519::{PublicKey, SecretKey};
use lru::LruCache;
use poldercast::{Gossip, Profile};
use rand::rngs::OsRng;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

enum Command {
    Send(Message),
    Gossips(Vec<Gossip>),
}

pub struct Connections {
    /// we are using an LRU Cache so we don't keep opened
    /// connections indefinitely if we are not using them
    ///
    /// we are not using an async Mutex here because the operation
    /// of adding or removing an entry from the mutex should be
    /// atomic, isolated and we should not hold the lock more than
    /// necessary
    ///
    // todo: pub this in a type so it is easier to change
    // behavior with time
    to: Arc<Mutex<LruCache<PublicKey, mpsc::Sender<Command>>>>,
    secret: Secret,

    message_sender: mpsc::Sender<(PublicKey, Message)>,
    message_receiver: mpsc::Receiver<(PublicKey, Message)>,
}

struct Runtime {
    inbound: ConnectionReader,
    outbound: ConnectionWriter,

    command_receiver: mpsc::Receiver<Command>,
    message_sender: mpsc::Sender<(PublicKey, Message)>,
}

impl Connections {
    pub fn new(secret: Secret, config: &Config) -> Self {
        let (message_sender, message_receiver) = mpsc::channel(config.message_queue_size);

        Self {
            to: Arc::new(Mutex::new(LruCache::new(config.max_opened_connections))),
            secret,

            message_sender,
            message_receiver,
        }
    }

    pub async fn receive(&mut self) -> (PublicKey, Message) {
        // we own at least one `message_sender` so there is always
        // a sender available
        self.message_receiver
            .recv()
            .await
            .expect("We should always receive something or wait indefinitely")
    }

    pub async fn accept(&mut self, accepting: Accepting<OsRng, SecretKey>) {
        let message_sender = self.message_sender.clone();

        let secret = self.secret.clone();

        let entries = self.to.clone();

        let _ = tokio::spawn(async move {
            if let Err(error) = accept(message_sender, secret, entries, accepting).await {
                tracing::warn!(reason = ?error, "Cannot accept inbound connection");
            }
        });
    }

    fn get_or_connect(&mut self, node: Arc<Profile>) -> Result<mpsc::Sender<Command>> {
        let id = node.id();

        loop {
            match self.to.lock().unwrap().get(&id).cloned() {
                Some(entry) => {
                    if entry.is_closed() {
                        self.to.lock().unwrap().pop(&id);
                    } else {
                        return Ok(entry);
                    }
                }
                None => {
                    let (command_sender, command_receiver) = mpsc::channel(8);
                    let message_sender = self.message_sender.clone();

                    let secret = self.secret.clone();
                    let entries = self.to.clone();

                    {
                        let command_sender = command_sender.clone();
                        let _ = tokio::spawn(async move {
                            if let Err(error) = connect(
                                message_sender,
                                command_sender.clone(),
                                command_receiver,
                                secret,
                                entries,
                                node,
                            )
                            .await
                            {
                                tracing::warn!(reason = ?error, "Cannot accept inbound connection");
                            }
                        });
                    }

                    return Ok(command_sender);
                }
            }
        }
    }

    pub async fn send_all(&mut self, nodes: Vec<Arc<Profile>>, message: Message) {
        for profile in nodes {
            if let Err(error) = self.send(profile, message.clone()).await {
                tracing::warn!(reason = ?error, "Cannot send message to peer");
            }
        }
    }

    pub async fn send_gossips(&mut self, peer: Arc<Profile>, gossips: Vec<Gossip>) -> Result<()> {
        let sender = self.get_or_connect(peer)?;

        sender
            .send(Command::Gossips(gossips))
            .await
            .map_err(|_| anyhow!("Cannot send gossips to peer"))
    }

    pub async fn send(&mut self, node: Arc<Profile>, message: Message) -> Result<()> {
        let sender = self.get_or_connect(node)?;

        sender
            .send(Command::Send(message))
            .await
            .map_err(|_| anyhow!("Cannot send message to peer"))
    }
}

impl Runtime {
    fn new(
        connection: Connection,
        command_receiver: mpsc::Receiver<Command>,
        message_sender: mpsc::Sender<(PublicKey, Message)>,
    ) -> Self {
        let (inbound, outbound) = connection.into_parts();

        Self {
            outbound,
            inbound,
            command_receiver,
            message_sender,
        }
    }

    #[tracing::instrument(
        skip(self),
        fields(
            id = %self.inbound.remote_public_identity(),
            address = %self.inbound.remote_address(),
            session_id = %self.inbound.session_id(),
        ),
        err,
        level = "info"
    )]
    async fn run(self) -> Result<()> {
        let Self {
            mut inbound,
            mut outbound,
            mut command_receiver,
            message_sender,
        } = self;

        tracing::info!("connected");

        loop {
            tokio::select! {
                result = inbound.next() => {
                    match result {
                        None => {
                            // disconnected
                            break;
                        }
                        Some((_peer, Err(error))) => {
                            // TODO: disconnect the whole node maybe?
                            tracing::warn!(reason = ?error, "Error while receiving message from peer");
                        }
                        Some((id, Ok(message))) => {
                            tracing::debug!("received new message");
                            if let Err(error) = message_sender.send((id, message)).await {
                                tracing::error!(reason = %error, "Cannot handle inbound message");
                                bail!("Error while sending message to rest of the node: {}", error)
                            }
                        }
                    }
                }
                command = command_receiver.recv() => {
                    match command {
                        None => break,
                        Some(Command::Send(message)) => {
                            tracing::debug!("sending message");
                            if let Err(error) = outbound.send(message).await {
                                tracing::warn!(reason = ?error, "cannot forward message message");
                            }
                        }
                        Some(Command::Gossips(gossips)) => {
                            tracing::debug!(num_gossips = gossips.len(), "sending gossips");
                            for gossip in gossips {
                                let message = Message::new_gossip(gossip.as_slice());
                                if let Err(error) = outbound.send(message).await {
                                    tracing::warn!(reason = ?error, "cannot forward gossip message");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("shutting down");

        Ok(())
    }
}

async fn connect(
    message_sender: mpsc::Sender<(PublicKey, Message)>,
    command_sender: mpsc::Sender<Command>,
    command_receiver: mpsc::Receiver<Command>,
    secret: Secret,
    entries: Arc<Mutex<LruCache<PublicKey, mpsc::Sender<Command>>>>,
    node: Arc<Profile>,
) -> Result<()> {
    let id = node.id();
    let address = node.address();

    let connection = Connection::connect_to(OsRng, &secret, address, id).await?;

    entries.lock().unwrap().put(id, command_sender);
    let runtime = Runtime::new(connection, command_receiver, message_sender);

    let r = runtime.run().await;

    entries.lock().unwrap().pop(&id);

    r
}

async fn accept(
    message_sender: mpsc::Sender<(PublicKey, Message)>,
    secret: Secret,
    entries: Arc<Mutex<LruCache<PublicKey, mpsc::Sender<Command>>>>,
    accepting: Accepting<OsRng, SecretKey>,
) -> Result<()> {
    let (command_sender, command_receiver) = mpsc::channel(8);

    let connection = accepting
        .handshake(secret.as_ref(), |pk| !entries.lock().unwrap().contains(pk))
        .await?;

    let id = *connection.remote_public_identity();

    entries.lock().unwrap().put(id, command_sender);

    let runtime = Runtime::new(connection, command_receiver, message_sender);

    let r = runtime.run().await;

    entries.lock().unwrap().pop(&id);

    r
}
