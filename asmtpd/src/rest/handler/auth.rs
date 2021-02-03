use crate::rest::{session::Session, state::State};

pub async fn post_auth(
    body: Vec<u8>,
    state: State,
    _session: Option<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.auth(body) {
        Ok(body) => Ok(body),
        Err(error) => Err(warp::reject::custom(error)),
    }
}
