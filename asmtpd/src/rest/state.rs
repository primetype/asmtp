use crate::{
    rest::{handler::topic, session::Session, sessions::Sessions, SessionConfig},
    secret::Secret,
    storage::Storage,
};
use anyhow::{Context as _, Error};
use asmtp_storage::Passport;
use keynesis::{
    noise::{HandshakeStateError, IK},
    passport::block::{Block, Hash},
};
use poldercast::Topic;
use std::collections::BTreeMap;
use thiserror::Error;
use warp::reject::Reject;

#[derive(Debug, Error)]
pub enum HandleAuthError {
    #[error("Handshake with peer failed")]
    InvalidHandshake(#[from] HandshakeStateError),

    #[error("login in rejected for this")]
    Rejected,

    #[error("Internal error")]
    InternalError(#[from] anyhow::Error),
}
impl Reject for HandleAuthError {}

#[derive(Debug, Error)]
pub enum HandlePostPassportError {
    #[error("Passport is invalid or cannot be reconstructed")]
    InvalidPassport(#[from] anyhow::Error),
}
impl Reject for HandlePostPassportError {}

#[derive(Debug, Error)]
pub enum GetTopicError {
    #[error("No topic registered for: {topic}")]
    NotFound { topic: Topic },

    #[error("Error while processing topic request with the database")]
    InternalError(#[from] anyhow::Error),
}
impl Reject for GetTopicError {}

#[derive(Error, Debug)]
#[error("Session does not have privilege for this operation")]
pub struct PrivilegeError;
impl Reject for PrivilegeError {}

#[derive(Clone)]
pub struct State {
    db: Storage,

    sessions: Sessions,

    secret: Secret,
}

impl State {
    pub fn new(db: Storage, secret: Secret, config: SessionConfig) -> Result<Self, Error> {
        let sessions = Sessions::new(config);

        Ok(Self {
            db,

            sessions,

            secret,
        })
    }

    pub fn sessions(&self) -> &Sessions {
        &self.sessions
    }

    /// get the passport associated to this running instance
    ///
    /// This can be used to check if a given id is allowed to access
    /// certain ID operations
    pub fn server_passport(&self) -> Option<Passport> {
        let r = self
            .db
            .get_passport_from_key(self.secret.as_ref().public_key());

        match r {
            Err(error) => {
                tracing::error!(reason = ?error, "Failed to query database");
                None
            }
            Ok(r) => r,
        }
    }

    pub fn ensure_is_admin_session(&self, root_session: Session) -> Result<(), PrivilegeError> {
        let id = root_session.remote_public_identity();
        let authorized = if let Some(passport) = self.server_passport() {
            passport.light_passport().active_master_keys().contains(&id)
        } else {
            false
        };

        if authorized {
            Ok(())
        } else {
            Err(PrivilegeError)
        }
    }

    pub fn post_passport_blocks(
        &self,
        blocks: Vec<Block>,
    ) -> Result<Hash, HandlePostPassportError> {
        let id = self.db.put_passport(blocks)?;
        Ok(id)
    }

    pub fn get_passport_blocks(&self, id: Hash) -> Result<Vec<Block>, anyhow::Error> {
        self.db.get_passport_blocks(id)
    }

    pub fn get_find_passport_id(
        &self,
        partial_id: impl AsRef<[u8]>,
    ) -> anyhow::Result<BTreeMap<sled::IVec, Hash>> {
        self.db.get_find_passport_id(partial_id)
    }

    /// force clear all the sessions
    ///
    /// This function will force all the active sessions to terminate, forcing
    /// everyone to re-authenticate
    pub fn clear_all_sessions(&self, root_session: Session) -> Result<(), PrivilegeError> {
        self.ensure_is_admin_session(root_session)?;

        self.sessions.clear();
        Ok(())
    }

    /// authenticate the user and returns the second half of the handshake
    ///
    /// There's no need to share the SessionId because of how noise
    /// works. the user should be able to retrieve it from handling
    /// the return noise message
    pub fn auth(&self, body: Vec<u8>) -> Result<Vec<u8>, HandleAuthError> {
        let mut output = Vec::with_capacity(1024);
        let session = IK::new(rand::thread_rng(), &[]);
        let session = session.receive(self.secret.as_ref(), body.as_slice())?;

        let remote_peer_id = session.remote_public_identity();

        if let Some(_) = self.db.get_passport_from_key(*remote_peer_id)? {
            tracing::info!(id = %remote_peer_id, "user authenticated");
        } else {
            return Err(HandleAuthError::Rejected);
        }

        let session = session.reply(&mut output)?;
        let session = Session::new(session);

        self.sessions.insert(session);

        Ok(output)
    }

    pub fn topic_get_messages(
        &self,
        topic: Topic,
        range: topic::TimeRange,
    ) -> Result<Vec<topic::Message>, GetTopicError> {
        if let Some(message) = self.db.get_topic(&topic)? {
            let mut output = Vec::new();
            for (id, message) in message.range_time(range) {
                output.push(topic::Message {
                    id,
                    message: message.as_ref().into(),
                });
            }

            Ok(output)
        } else {
            Err(GetTopicError::NotFound { topic })
        }
    }

    pub fn topic_post(&self, topic: Topic) -> Result<(), GetTopicError> {
        let _ = self
            .db
            .subscribe_message(topic)
            .context("Cannot insert new topic message")?;

        Ok(())
    }

    pub fn topic_post_message(&self, topic: Topic, bytes: Vec<u8>) -> Result<(), GetTopicError> {
        if let Some(message) = self.db.get_topic(&topic)? {
            message
                .insert(bytes)
                .context("Cannot insert new topic message")?;
            Ok(())
        } else {
            Err(GetTopicError::NotFound { topic })
        }
    }

    pub fn topic_delete_messages(
        &self,
        topic: Topic,
        range: topic::TimeConstraint,
    ) -> Result<(), GetTopicError> {
        if let Some(message) = self.db.get_topic(&topic)? {
            message
                .remove_range(range)
                .context("Cannot delete range in the topic")?;
            Ok(())
        } else {
            Err(GetTopicError::NotFound { topic })
        }
    }
}
