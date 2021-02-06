use crate::rest::{api::config, handler, header, middleware, state::State};
use keynesis::passport::block::{Block, BlockSlice, Hash};
use poldercast::Topic;
use warp::Filter;

pub fn apis(
    _cors: config::Cors,
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    // TODO: extract the CORS config from the `_cors` variable
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["Content-Type", "accept", header::SESSION_ID])
        .build();

    let routes = auth(state.clone())
        .or(post_passport(state.clone()))
        .or(get_passport(state.clone()))
        .or(get_find_passport_id(state.clone()))
        .or(topic_get_messages(state.clone()))
        .or(topic_post(state.clone()))
        .or(topic_post_message(state.clone()))
        .or(topic_delete(state));

    routes.recover(middleware::rejection::handle).with(cors)
}

/* HANDLERS **************************************************************** */

// POST /auth
fn auth(state: State) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("auth")
        .and(warp::post())
        .and(bytes_body())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and_then(handler::auth::post_auth)
}

// POST /passport
fn post_passport(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("passport")
        .and(warp::post())
        .and(json_body_blocks())
        .and(with_state(state))
        .and(middleware::auth::authenticated_session(sessions))
        .and_then(handler::passport::post_passport)
}

// GET /passport/<id>
fn get_passport(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("passport" / Hash)
        .and(warp::get())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and_then(handler::passport::get_passport)
}

// GET /passport/search/<str>
fn get_find_passport_id(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    use self::handler::passport::PartialId;

    let sessions = state.sessions().clone();
    warp::path!("passport" / "search" / PartialId)
        .and(warp::get())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and_then(handler::passport::get_find_passport_id)
}

// GET /topic/{id}/messages?from={lower_bound}&to={upper_bound}
fn topic_get_messages(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("topic" / Topic)
        .and(warp::get())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and(warp::filters::query::query())
        .and_then(handler::topic::get_messages)
}

// POST /topic/{id}
fn topic_post(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("topic" / Topic)
        .and(warp::post())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and_then(handler::topic::post)
}

// POST /topic/{id}/message
fn topic_post_message(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("topic" / Topic / "message")
        .and(warp::post())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and(bytes_body())
        .and_then(handler::topic::post_messages)
}

// DELETE /topic/{id}?until={upper_bound}
fn topic_delete(
    state: State,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let sessions = state.sessions().clone();
    warp::path!("topic" / Topic)
        .and(warp::delete())
        .and(with_state(state))
        .and(middleware::auth::maybe_authenticated_session(sessions))
        .and(warp::filters::query::query())
        .and_then(handler::topic::delete_messages)
}

/* HELPERS ***************************************************************** */

fn with_state(
    state: State,
) -> impl Filter<Extract = (State,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn bytes_body() -> impl Filter<Extract = (Vec<u8>,), Error = warp::Rejection> + Copy {
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024)
        .and(warp::body::bytes())
        .map(|bytes: warp::hyper::body::Bytes| bytes.to_vec())
}

fn json_body_blocks() -> impl Filter<Extract = (Vec<Block>,), Error = warp::Rejection> + Clone {
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024 * 1024)
        .and(warp::body::json())
        .and_then(|blocks: Vec<String>| async move {
            let mut r = Vec::new();

            for block in blocks {
                let bytes = match hex::decode(block) {
                    Ok(bytes) => bytes,
                    Err(_error) => return Err(warp::reject::reject()),
                };

                let block = match BlockSlice::try_from_slice(&bytes) {
                    Ok(block) => block.to_block(),
                    Err(_error) => return Err(warp::reject::reject()),
                };

                r.push(block);
            }

            Ok(r)
        })
}
