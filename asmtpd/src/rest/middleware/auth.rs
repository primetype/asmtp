use crate::{
    rest::{header, session::Session, sessions::Sessions},
    SessionId,
};
use warp::{
    filters::header::{header, optional as optional_header},
    reject::{self, Rejection},
    Filter,
};

/// require the request to have a valid, live, authenticated session
///
pub fn authenticated_session(
    sessions: Sessions,
) -> impl Filter<Extract = (Session,), Error = Rejection> + Clone {
    header::<SessionId>(header::SESSION_ID)
        .map(move |id: SessionId| sessions.lookup(&id))
        .and_then(|r| async {
            match r {
                Ok(session) => Ok(session),
                Err(error) => Err(reject::custom(error)),
            }
        })
}

/// optionally require the session to be authenticated (valid and live)
///
pub fn maybe_authenticated_session(
    sessions: Sessions,
) -> impl Filter<Extract = (Option<Session>,), Error = Rejection> + Clone {
    optional_header::<SessionId>(header::SESSION_ID)
        .map(move |id: Option<SessionId>| {
            if let Some(id) = id {
                sessions.lookup(&id).map(Some)
            } else {
                Ok(None)
            }
        })
        .and_then(|r| async {
            match r {
                Ok(session) => Ok(session),
                Err(error) => Err(reject::custom(error)),
            }
        })
}
