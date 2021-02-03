use crate::{
    rest::{Session, SessionConfig},
    SessionId,
};
use lru::LruCache;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use warp::reject::Reject;

// Default values
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session has expired")]
    Expired,
    #[error("Session has been IDLE for too long")]
    IdleTooLong,
    #[error("Session not found")]
    NotFound,
}

#[derive(Clone)]
pub struct Sessions {
    sessions: Arc<Mutex<LruCache<SessionId, Session>>>,
    config: Arc<SessionConfig>,
}

impl Sessions {
    pub fn new(config: SessionConfig) -> Self {
        let sessions = Arc::new(Mutex::new(LruCache::new(config.max_active_sessions)));

        Self {
            sessions,
            config: Arc::new(config),
        }
    }

    /// insert the new session in the session cache
    pub fn insert(&self, session: Session) {
        let id = *session.id();

        let mut sessions = self
            .sessions
            .lock()
            .expect("the lock should always be valid");
        sessions.put(id, session);
    }

    pub fn lookup(&self, session: &SessionId) -> Result<Session, SessionError> {
        let mut sessions = self
            .sessions
            .lock()
            .expect("the lock should always be valid");

        if let Some(session) = sessions.get(session).cloned() {
            if session.lifespan().as_secs() > self.config.max_lifespan {
                sessions.pop(session.id());
                Err(SessionError::Expired)
            } else if session.update_last_use().as_secs() > self.config.max_idle {
                sessions.pop(session.id());
                Err(SessionError::IdleTooLong)
            } else {
                Ok(session)
            }
        } else {
            Err(SessionError::NotFound)
        }
    }

    pub fn clear(&self) {
        self.sessions
            .lock()
            .expect("the lock should always be valid")
            .clear()
    }
}

impl Reject for SessionError {}
