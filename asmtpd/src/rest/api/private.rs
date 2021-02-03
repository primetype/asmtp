use crate::rest::{handler, header, middleware, state::State};
use std::sync::Arc;
use tokio::sync::Notify;
use warp::Filter;

pub fn apis(
    state: State,
    notify: Arc<Notify>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["Content-Type", "accept", header::SESSION_ID])
        .build();

    let log = warp::log::custom(|info| {
        let remote = info
            .remote_addr()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "Unknown".to_owned());
        let user_agent = info.user_agent().unwrap_or("unknown");
        let host = info.host().unwrap_or("unknown");

        tracing::info!(
            remote = %remote,
            method = %info.method(),
            path = %info.path(),
            status = %info.status(),
            version = ?info.version(),
            user_agent = %user_agent,
            elapsed = ?info.elapsed(),
            host = %host,
        );
    });

    let privileged_handlers = delete_sessions(state.clone()).or(shutdown(state, notify));

    privileged_handlers
        .recover(middleware::rejection::handle)
        .with(cors)
        .with(log)
}

/* HANDLERS **************************************************************** */

fn shutdown(
    state: State,
    notify: Arc<Notify>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("sessions")
        .and(warp::delete())
        .and(with_state(state))
        .and(with_notify(notify))
        .and(middleware::auth::authenticated_session(sessions))
        .and_then(handler::root_control::shutdown)
}

fn delete_sessions(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("sessions")
        .and(warp::delete())
        .and(with_state(state))
        .and(middleware::auth::authenticated_session(sessions))
        .and_then(handler::root_control::delete_sessions)
}

/* HELPERS ***************************************************************** */

fn with_notify(
    notify: Arc<Notify>,
) -> impl Filter<Extract = (Arc<Notify>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || notify.clone())
}

fn with_state(
    state: State,
) -> impl Filter<Extract = (State,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}
