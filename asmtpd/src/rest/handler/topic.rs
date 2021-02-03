use crate::rest::{session::Session, state::State};
use asmtp_lib::MessageId;
use keynesis::passport::block::Time;
use poldercast::Topic;
use serde::{Deserialize, Serialize};
use std::ops::{Bound, RangeBounds};

#[derive(Debug, Clone)]
pub struct TimeDef(pub Time);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimeRange {
    pub from: Option<TimeDef>,
    pub to: Option<TimeDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimeConstraint {
    pub until: Option<TimeDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub message: Box<[u8]>,
}

pub async fn get_messages(
    topic: Topic,
    state: State,
    _session: Option<Session>,
    time_range: TimeRange,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.topic_get_messages(topic, time_range) {
        Ok(vec) => Ok(warp::reply::json(&vec)),
        Err(error) => {
            tracing::info!(error = ?error, "failed to get topic's messages");
            Err(warp::reject::custom(error))
        }
    }
}

pub async fn post(
    topic: Topic,
    state: State,
    _session: Option<Session>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.topic_post(topic) {
        Ok(()) => Ok(warp::reply()),
        Err(error) => {
            tracing::info!(error = ?error, "failed to post topic");
            Err(warp::reject::custom(error))
        }
    }
}

pub async fn post_messages(
    topic: Topic,
    state: State,
    _session: Option<Session>,
    body: Vec<u8>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.topic_post_message(topic, body) {
        Ok(()) => Ok(warp::reply()),
        Err(error) => {
            tracing::info!(error = ?error, "failed to post topic's messages");
            Err(warp::reject::custom(error))
        }
    }
}

pub async fn delete_messages(
    topic: Topic,
    state: State,
    _session: Option<Session>,
    time_constraint: TimeConstraint,
) -> Result<impl warp::Reply, warp::Rejection> {
    match state.topic_delete_messages(topic, time_constraint) {
        Ok(()) => Ok(warp::reply()),
        Err(error) => {
            tracing::info!(error = ?error, "failed to delete topic's messages");
            Err(warp::reject::custom(error))
        }
    }
}

impl RangeBounds<Time> for TimeRange {
    fn start_bound(&self) -> Bound<&Time> {
        match &self.from {
            None => Bound::Unbounded,
            Some(TimeDef(from)) => Bound::Included(from),
        }
    }

    fn end_bound(&self) -> Bound<&Time> {
        match &self.to {
            None => Bound::Unbounded,
            Some(TimeDef(to)) => Bound::Included(to),
        }
    }
}

impl RangeBounds<Time> for TimeConstraint {
    fn start_bound(&self) -> Bound<&Time> {
        Bound::Unbounded
    }

    fn end_bound(&self) -> Bound<&Time> {
        match &self.until {
            None => Bound::Unbounded,
            Some(TimeDef(until)) => Bound::Included(until),
        }
    }
}

impl Serialize for TimeDef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        if serializer.is_human_readable() {
            // TODO: use chrono crate to have a human readable format instead
            serializer.serialize_u32(*self.0)
        } else {
            serializer.serialize_u32(*self.0)
        }
    }
}

impl<'de> Deserialize<'de> for TimeDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct TimeDefVisitor;

        impl<'de> serde::de::Visitor<'de> for TimeDefVisitor {
            type Value = TimeDef;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("A time in ISO format or as seconds since COVID_EPOCH")
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TimeDef(Time::from(value)))
            }
        }

        if deserializer.is_human_readable() {
            // TODO: use chrono crate to have a human readable format instead
            deserializer.deserialize_u32(TimeDefVisitor)
        } else {
            deserializer.deserialize_u32(TimeDefVisitor)
        }
    }
}
