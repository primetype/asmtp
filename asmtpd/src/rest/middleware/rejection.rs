use crate::rest::{
    sessions::SessionError,
    state::{GetTopicError, HandleAuthError, HandlePostPassportError},
};
use serde::Serialize;
use std::convert::Infallible;
use warp::{
    filters::body::BodyDeserializeError, http::StatusCode, reject::Rejection, reply::Reply,
};

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
    details: Option<String>,
}

pub async fn handle(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;
    let more;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
        more = None;
    } else if let Some(e) = err.find::<SessionError>() {
        match e {
            SessionError::Expired | SessionError::IdleTooLong => {
                code = StatusCode::UNAUTHORIZED;
                message = "SESSION EXPIRED";
            }
            SessionError::NotFound => {
                code = StatusCode::NETWORK_AUTHENTICATION_REQUIRED;
                message = "AUTHENTICATION REQUIRED";
            }
        }
        more = Some(e.to_string());
    } else if let Some(e) = err.find::<HandleAuthError>() {
        code = StatusCode::BAD_REQUEST;
        message = "BAD_REQUEST";
        more = Some(e.to_string());
    } else if let Some(e) = err.find::<HandlePostPassportError>() {
        match e {
            HandlePostPassportError::InvalidPassport(_) => {
                code = StatusCode::BAD_REQUEST;
                message = "BAD_REQUEST";
            }
            HandlePostPassportError::InternalError(_) => {
                code = StatusCode::INTERNAL_SERVER_ERROR;
                message = "INTERNAL_SERVER_ERROR";
            }
        }
        more = Some(e.to_string());
    } else if let Some(e) = err.find::<GetTopicError>() {
        match e {
            GetTopicError::NotFound { .. } => {
                code = StatusCode::NOT_FOUND;
                message = "NOT_FOUND";
                more = Some(e.to_string());
            }
            GetTopicError::InternalError(_) => {
                code = StatusCode::INTERNAL_SERVER_ERROR;
                message = "INTERNAL_SERVER_ERROR";
                more = None;
            }
        }
    } else if let Some(e) = err.find::<BodyDeserializeError>() {
        code = StatusCode::BAD_REQUEST;
        message = "BAD_REQUEST";
        more = Some(e.to_string());
    } else {
        tracing::error!("unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION";
        more = None;
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
        details: more,
    });

    Ok(warp::reply::with_status(json, code))
}
