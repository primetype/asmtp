use keynesis::{
    hash::Blake2b,
    noise::{CipherStateError, TransportState},
};
use std::fmt;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::SessionId;

struct SessionInternal {
    last_used: Instant,
    state: TransportState<Blake2b>,
}

#[derive(Clone)]
pub struct Session {
    id: SessionId,
    started: Instant,
    inner: Arc<Mutex<SessionInternal>>,
}

impl SessionInternal {
    fn new(state: TransportState<Blake2b>) -> Self {
        Self {
            last_used: Instant::now(),
            state,
        }
    }

    fn update_last_use(&mut self) -> Duration {
        let ret = self.last_used.elapsed();

        self.last_used = Instant::now();

        ret
    }
}

impl Session {
    pub fn new(state: TransportState<Blake2b>) -> Self {
        let id = SessionId(*state.noise_session());
        let started = Instant::now();
        let inner = Arc::new(Mutex::new(SessionInternal::new(state)));

        Self { id, started, inner }
    }

    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn remote_public_identity(&self) -> keynesis::key::ed25519::PublicKey {
        *self
            .inner
            .lock()
            .expect("lock should be valid")
            .remote_public_identity()
    }

    /// return the lifespan of the Session (i.e. the duration since its creation)
    ///
    /// This function does not block
    pub fn lifespan(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn update_last_use(&self) -> Duration {
        self.inner
            .lock()
            .expect("lock the inner state should not fail")
            .update_last_use()
    }

    pub fn encrypt<P>(&self, payload: P) -> Result<Vec<u8>, CipherStateError>
    where
        P: AsRef<[u8]>,
    {
        let mut inner = self
            .inner
            .lock()
            .expect("lock the inner state should not fail");

        let mut output = Vec::with_capacity(payload.as_ref().len().wrapping_add(16));
        inner.send(payload, &mut output)?;

        Ok(output)
    }

    pub fn decrypt<P>(&self, payload: P) -> Result<Vec<u8>, CipherStateError>
    where
        P: AsRef<[u8]>,
    {
        let mut inner = self
            .inner
            .lock()
            .expect("lock the inner state should not fail");

        let mut output = Vec::with_capacity(payload.as_ref().len().wrapping_sub(16));
        inner.receive(payload, &mut output)?;

        Ok(output)
    }
}

impl Deref for SessionInternal {
    type Target = TransportState<Blake2b>;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for SessionInternal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl From<TransportState<Blake2b>> for Session {
    fn from(state: TransportState<Blake2b>) -> Self {
        Self::new(state)
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("started", &self.lifespan())
            .finish()
    }
}
