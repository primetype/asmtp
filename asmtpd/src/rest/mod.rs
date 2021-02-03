mod api;
mod config;
mod handler;
pub mod header;
mod middleware;
mod session;
mod sessions;
mod state;

pub use self::{
    config::{Config, SessionConfig},
    session::Session,
    sessions::Sessions,
};
use crate::{secret::Secret, storage::Storage};
use anyhow::{Context as _, Result};
use std::sync::Arc;
use tokio::sync::Notify;
use warp::Filter;

/// launch the REST server and returns object to notify the server to shutdown
pub fn run(config: Config, storage: Storage, secret: Secret) -> Result<Arc<Notify>> {
    let notify = Arc::new(Notify::new());

    let state =
        state::State::new(storage, secret, config.state).context("Cannot initialize REST state")?;

    let shutdown_command = notify.clone();
    let watcher = notify.clone();

    let routes = api::all(config.cors, shutdown_command, state).with(warp::trace::request());

    let (address, task) = warp::serve(routes)
        .bind_with_graceful_shutdown(config.listen, async move { watcher.notified().await });

    tracing::info!(address = %address, "starting listening for incoming HTTP queries");
    tokio::spawn(task);

    Ok(notify)
}
