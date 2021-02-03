pub mod config;
pub mod private;
pub mod public;

use crate::rest::state::State;
use std::sync::Arc;
use tokio::sync::Notify;
use warp::Filter;

pub fn all(
    cors: config::Cors,
    notify: Arc<Notify>,
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    private::apis(state.clone(), notify).or(public::apis(cors, state))
}
