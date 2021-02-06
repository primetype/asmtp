use crate::rest::{session::Session, state::State};
use keynesis::passport::block::{Block, Hash};
use warp::http::StatusCode;

#[derive(Debug, Clone)]
pub struct PartialId(Vec<u8>);

pub async fn get_passport(
    id: Hash,
    state: State,
    _session: Option<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.get_passport_blocks(id) {
        Err(error) => {
            tracing::error!(error = ?error, "error while getting passport's block");
            Err(warp::reject())
        }
        Ok(passport) => {
            let events = passport
                .into_iter()
                .map(|b| hex::encode(b.as_ref()))
                .collect::<Vec<_>>();
            Ok(warp::reply::json(&events))
        }
    }
}

pub async fn get_find_passport_id(
    id: PartialId,
    state: State,
    _session: Option<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.get_find_passport_id(id.0) {
        Ok(result) => {
            let result = result
                .into_iter()
                .map(|(k, v)| (hex::encode(k.as_ref()), v.to_string()))
                .collect::<Vec<_>>();
            Ok(warp::reply::with_status(
                warp::reply::json(&result),
                warp::http::StatusCode::OK,
            ))
        }
        Err(error) => {
            tracing::error!(error = ?error, "error while getting passport from keywall lib");
            Err(warp::reject())
        }
    }
}

pub async fn post_passport(
    blocks: Vec<Block>,
    state: State,
    session: Session,
) -> Result<impl warp::Reply, warp::Rejection> {
    if let Err(error) = state.ensure_is_admin_session(session) {
        return Err(warp::reject::custom(error));
    }

    match state.post_passport_blocks(blocks) {
        Err(error) => Err(warp::reject::custom(error)),
        Ok(id) => Ok(warp::reply::with_status(
            id.to_string(),
            StatusCode::CREATED,
        )),
    }
}

impl std::fmt::Display for PartialId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        hex::encode(&self.0).fmt(f)
    }
}

impl std::str::FromStr for PartialId {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(Self)
    }
}
