use crate::rest::{session::Session, state::State};
use std::sync::Arc;
use tokio::sync::Notify;

pub async fn delete_sessions(
    state: State,
    session: Session,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.clear_all_sessions(session) {
        Err(error) => {
            tracing::warn!(error = ?error, "Invalid privilege");
            Err(warp::reject::custom(error))
        }
        Ok(()) => Ok(warp::reply()),
    }
}

pub async fn shutdown(
    state: State,
    notify: Arc<Notify>,
    session: Session,
) -> Result<impl warp::Reply, warp::Rejection> {
    delete_sessions(state, session).await?;

    notify.notify_waiters();

    Ok(warp::reply())
}
