use anyhow::{Context, Result};
use asmtp_network::{
    net::{Connection, ConnectionReader, ConnectionWriter},
    Message, SessionId,
};
use futures::prelude::*;
use keynesis::key::{curve25519::PublicKey, ed25519::SecretKey};
use rand::{CryptoRng, RngCore};
use std::{
    net::SocketAddr,
    sync::{mpsc as std_mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct NetworkStats {
    pub peer_id: PublicKey,
    pub peer_address: SocketAddr,
    pub current_id: PublicKey,
    pub session_id: SessionId,
    pub connection_established_since: Instant,
    pub number_message_sent: usize,
    pub last_message_sent: Instant,
    pub number_message_received: usize,
    pub last_message_received: Instant,
    pub error: Option<Arc<anyhow::Error>>,
    pub last_error_received: Option<Instant>,
}

#[derive(Default)]
pub struct Network {
    inner: Option<Inner>,
    connection_failure: Option<anyhow::Error>,
}

struct Inner {
    stats: Arc<Mutex<NetworkStats>>,
    outbound_messages: mpsc::Sender<Message>,
    inbound_messages: std_mpsc::Receiver<Message>,
    _condition: oneshot::Sender<()>,
}

struct Runtime {
    stats: Arc<Mutex<NetworkStats>>,
    outbound: ConnectionWriter,
    inbound: ConnectionReader,
    outbound_messages: mpsc::Receiver<Message>,
    inbound_messages: std_mpsc::Sender<Message>,
    shutdown_condition: oneshot::Receiver<()>,
}

impl Network {
    pub fn disconnect(&mut self) {
        let _previous = self.inner.take();
    }

    pub fn stats(&self) -> Option<Arc<Mutex<NetworkStats>>> {
        let inner = self.inner.as_ref()?;
        Some(Arc::clone(&inner.stats))
    }

    pub fn send_message(&self, message: Message) -> Option<Message> {
        if let Some(inner) = self.inner.as_ref() {
            inner.send_message(message)
        } else {
            Some(message)
        }
    }

    pub fn receive_message(&mut self) -> Option<Message> {
        if let Some(inner) = self.inner.as_mut() {
            inner.receive_message()
        } else {
            None
        }
    }

    pub fn connection_failure(&self) -> Option<&anyhow::Error> {
        self.connection_failure.as_ref()
    }

    pub async fn connect<RNG>(
        &mut self,
        rng: RNG,
        remote_address: SocketAddr,
        remote_identity: PublicKey,
        sk: &SecretKey,
    ) where
        RNG: CryptoRng + RngCore,
    {
        let new = Inner::new(rng, remote_address, remote_identity, sk).await;
        match new {
            Ok(new) => {
                // no error
                let _previous = self.inner.replace(new);
                self.connection_failure.take();
            }
            Err(error) => {
                // replace the error
                self.connection_failure.replace(error);
            }
        }
    }
}

impl Inner {
    async fn new<RNG>(
        rng: RNG,
        remote_address: SocketAddr,
        remote_identity: PublicKey,
        sk: &SecretKey,
    ) -> Result<Self>
    where
        RNG: CryptoRng + RngCore,
    {
        let current_id = sk.public_key();
        let connection = tokio::time::timeout(
            Duration::from_secs(1),
            Connection::connect_to(rng, sk, remote_address, remote_identity),
        )
        .await
        .context("Cannot connect to remote peer")?
        .context("Failed to establish secure connection to peer")?;
        let stats = Arc::new(Mutex::new(NetworkStats {
            peer_id: *connection.remote_public_identity(),
            peer_address: connection.remote_address(),
            current_id,
            session_id: *connection.session_id(),
            connection_established_since: Instant::now(),
            number_message_sent: 1,
            last_message_sent: Instant::now(),
            number_message_received: 1,
            last_message_received: Instant::now(),
            error: None,
            last_error_received: None,
        }));

        let (inbound_sender, inbound_receiver) = std_mpsc::channel();
        let (outbound_sender, outbound_receiver) = mpsc::channel(12);
        let (_condition, shutdown_condition) = oneshot::channel();

        let runtime = Runtime::new(
            connection,
            Arc::clone(&stats),
            outbound_receiver,
            inbound_sender,
            shutdown_condition,
        );

        tokio::task::spawn(async move { runtime.run().await });

        Ok(Self {
            stats,
            outbound_messages: outbound_sender,
            inbound_messages: inbound_receiver,
            _condition,
        })
    }

    pub fn send_message(&self, message: Message) -> Option<Message> {
        if let Err(error) = self.outbound_messages.try_send(message) {
            match error {
                tokio::sync::mpsc::error::TrySendError::Full(message) => Some(message),
                tokio::sync::mpsc::error::TrySendError::Closed(message) => Some(message),
            }
        } else {
            None
        }
    }

    pub fn receive_message(&mut self) -> Option<Message> {
        self.inbound_messages.try_recv().ok()
    }
}

impl Runtime {
    fn new(
        connection: Connection,
        stats: Arc<Mutex<NetworkStats>>,
        outbound_messages: mpsc::Receiver<Message>,
        inbound_messages: std_mpsc::Sender<Message>,
        shutdown_condition: oneshot::Receiver<()>,
    ) -> Self {
        let (inbound, outbound) = connection.into_parts();
        Self {
            stats: Arc::clone(&stats),
            outbound,
            inbound,
            outbound_messages,
            inbound_messages,
            shutdown_condition,
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                _ = &mut self.shutdown_condition => {
                    // here we check the shutdown condition is actually
                    // either alive or not triggered.
                    //
                    // may it be an error or an successful delivery
                    // it means the `network::Inner` has been dropped and
                    // it is time to disconnect
                    break;
                }
                inbound = self.inbound.next() => {
                    let close = self.handle_inbound(inbound).await;
                    if close {
                        break;
                    }
                }
                outbound = self.outbound_messages.recv() => {
                    let close = self.handle_outbound(outbound).await;
                    if close {
                        break;
                    }
                }
            }
        }
    }

    async fn handle_outbound(&mut self, outbound: Option<Message>) -> bool {
        if let Some(outbound) = outbound {
            let result = self.outbound.send(outbound).await;
            if let Ok(mut stats) = self.stats.lock() {
                if let Err(error) = result {
                    stats.last_error_received = Some(Instant::now());
                    stats.error = Some(Arc::new(error));
                }
                stats.last_message_sent = Instant::now();
                stats.number_message_sent += 1;
            }
            false
        } else {
            true
        }
    }

    async fn handle_inbound(
        &mut self,
        inbound: Option<(PublicKey, anyhow::Result<Message>)>,
    ) -> bool {
        match inbound {
            None => {
                // disconnected
                if let Ok(mut stats) = self.stats.lock() {
                    stats.last_error_received = Some(Instant::now());
                    stats.error = Some(Arc::new(anyhow::anyhow!("Disconnected...")));
                }
                true
            }
            Some((_peer, Err(error))) => {
                if let Ok(mut stats) = self.stats.lock() {
                    stats.last_message_received = Instant::now();
                    stats.number_message_received += 1;
                    stats.last_error_received = Some(Instant::now());
                    stats.error = Some(Arc::new(error));
                }
                false
            }
            Some((_peer, Ok(message))) => {
                if let Ok(mut stats) = self.stats.lock() {
                    stats.last_message_received = Instant::now();
                    stats.number_message_received += 1;
                }
                // if we cannot send the reply back to the mpsc
                // it means there is no receiver to receive from
                // so we can simply returns we want to close the
                // connection
                self.inbound_messages.send(message).is_err()
            }
        }
    }
}
