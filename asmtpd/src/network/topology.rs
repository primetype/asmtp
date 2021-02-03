use crate::secret::Secret;
use keynesis::key::ed25519::PublicKey;
use poldercast::{layer::Selection, Gossip, Profile, Topic};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Topology {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    secret: Secret,
    topology: poldercast::Topology,
}

impl Inner {
    fn new(secret: Secret, topology: poldercast::Topology) -> Self {
        Self { secret, topology }
    }

    fn subscriptions(&mut self, add: Vec<Topic>, remove: Vec<Topic>) {
        for topic in add {
            self.topology.subscribe_topic(topic);
        }
        for topic in remove {
            self.topology.unsubscribe_topic(&topic)
        }

        self.topology
            .update_profile_subscriptions(self.secret.as_ref());
    }

    fn accept_gossip(&mut self, gossip: Gossip) {
        let peer = Profile::from_gossip(gossip);

        self.topology.add_peer(peer);
    }

    fn gossips_for(&mut self, recipient: &PublicKey) -> Vec<Gossip> {
        self.topology.gossips_for(recipient)
    }

    fn view_for(&mut self, from: Option<&PublicKey>, selection: Selection) -> Vec<Arc<Profile>> {
        self.topology.view(from, selection)
    }
}

impl Topology {
    pub fn new(address: SocketAddr, secret: Secret) -> Self {
        let topology = poldercast::Topology::new(address, secret.as_ref());

        let inner = Inner::new(secret, topology);

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn get(&self, id: &PublicKey) -> Option<Arc<Profile>> {
        let mut inner = self.inner.lock().unwrap();
        inner.topology.get(id).cloned()
    }

    pub fn subscriptions(&self, add: Vec<Topic>, remove: Vec<Topic>) {
        let mut inner = self.inner.lock().unwrap();
        inner.subscriptions(add, remove)
    }

    pub fn accept_gossip(&self, gossip: Gossip) {
        self.inner.lock().unwrap().accept_gossip(gossip)
    }

    pub fn view_for(&self, from: Option<&PublicKey>, selection: Selection) -> Vec<Arc<Profile>> {
        self.inner.lock().unwrap().view_for(from, selection)
    }

    pub fn gossips_for(&self, recipient: &PublicKey) -> Vec<Gossip> {
        self.inner.lock().unwrap().gossips_for(recipient)
    }
}
