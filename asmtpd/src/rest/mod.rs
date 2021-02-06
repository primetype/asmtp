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
use anyhow::{bail, Context as _, Result};
use keynesis::passport::block::Block;
use std::sync::Arc;
use tokio::sync::Notify;
use warp::Filter;

pub fn import_passport(storage: Storage, passport_blocks: Vec<Block>) -> Result<()> {
    let id = storage
        .put_passport(passport_blocks)
        .context("Cannot set the server's passport")?;

    tracing::info!(id = %id, "passport successfully imported");

    Ok(())
}

/// launch the REST server and returns object to notify the server to shutdown
pub fn run(config: Config, storage: Storage, secret: Secret) -> Result<Arc<Notify>> {
    let notify = Arc::new(Notify::new());

    let state =
        state::State::new(storage, secret, config.state).context("Cannot initialize REST state")?;

    let server_passport_id = if let Some(passport) = state.server_passport() {
        passport.id()
    } else {
        bail!("Cannot start REST server without a server passport")
    };

    let shutdown_command = notify.clone();
    let watcher = notify.clone();

    let routes = api::all(config.cors, shutdown_command, state).with(warp::trace::request());

    let (address, task) = warp::serve(routes)
        .bind_with_graceful_shutdown(config.listen, async move { watcher.notified().await });

    tracing::info!(address = %address, server_passport = %server_passport_id, "starting listening for incoming HTTP queries");
    tokio::spawn(task);

    Ok(notify)
}
