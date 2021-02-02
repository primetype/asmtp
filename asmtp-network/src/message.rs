use crate::codec::encryption::MAX_FRAME_LENGTH;
use anyhow::{ensure, Context as _, Result};
use bytes::{BufMut as _, Bytes, BytesMut};
use poldercast::{GossipSlice, Topic};
use std::convert::TryFrom;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
#[repr(u8)]
pub enum MessageType {
    Gossip = 1,
    Topic = 2,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct MessageSlice<'a>(&'a [u8]);

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Message(Bytes);

impl MessageType {
    const SIZE: usize = 1;

    #[inline]
    fn to_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    fn try_from_u8(t: u8) -> Option<Self> {
        match t {
            1 => Some(Self::Gossip),
            2 => Some(Self::Topic),
            0 | 3..=u8::MAX => None,
        }
    }
}

impl Message {
    const MAX_SIZE: usize = MAX_FRAME_LENGTH - MessageType::SIZE;
    const MIN_SIZE: usize = MessageType::SIZE + Topic::SIZE;

    /// create a new message from the given gossip
    pub fn new_gossip(gossip: GossipSlice<'_>) -> Self {
        let mut bytes = BytesMut::with_capacity(MessageType::SIZE + gossip.as_ref().len());
        bytes.reserve(MessageType::SIZE + gossip.as_ref().len());

        bytes.put_u8(MessageType::Gossip.to_u8());
        bytes.put_slice(gossip.as_ref());

        Self(bytes.freeze())
    }

    /// create a new message from the given topic and content
    pub fn new_topic(topic: Topic, message: Bytes) -> Self {
        let mut bytes = BytesMut::with_capacity(MessageType::SIZE + Topic::SIZE + message.len());
        bytes.reserve(MessageType::SIZE + Topic::SIZE + message.len());

        bytes.put_u8(MessageType::Topic.to_u8());
        bytes.put_slice(topic.as_ref());
        bytes.put_slice(message.as_ref());

        Self(bytes.freeze())
    }

    #[inline(always)]
    pub fn as_slice<'a>(&'a self) -> MessageSlice<'a> {
        MessageSlice(self.0.as_ref())
    }

    pub fn message_type(&self) -> MessageType {
        self.as_slice().message_type()
    }

    pub fn gossip_checked<'a>(&'a self) -> Option<GossipSlice<'a>> {
        self.as_slice()
            .gossip_checked()
            .expect("Expecting to have a valid Gossip message")
    }

    pub fn topic_checked<'a>(&'a self) -> Option<(Topic, &'a [u8])> {
        self.as_slice()
            .topic_checked()
            .expect("Expecting to have a valid topic message")
    }

    pub fn to_bytes(&self) -> Bytes {
        self.0.clone()
    }
}

impl<'a> MessageSlice<'a> {
    pub fn try_from_slice(slice: &'a [u8]) -> Result<Self> {
        ensure!(
            slice.len() >= Message::MIN_SIZE,
            "Not enough bytes to complete the smallest message possible"
        );

        let message_type =
            MessageType::try_from_u8(slice[0]).context("Invalid message, unknown message type")?;
        let message = Self::from_slice_unchecked(slice);

        match message_type {
            MessageType::Gossip => {
                message.gossip_checked()?;
            }
            MessageType::Topic => {
                message.topic_checked()?;
            }
        }

        Ok(message)
    }

    #[inline(always)]
    fn from_slice_unchecked(slice: &'a [u8]) -> Self {
        assert!(
            slice.len() >= Message::MIN_SIZE,
            "Message cannot be smaller than the length of the type"
        );
        assert!(
            slice.len() <= Message::MAX_SIZE,
            "Message cannot be larger than the max frame length"
        );

        Self(slice)
    }

    /// create a owned version of the message
    pub fn to_message(&self) -> Message {
        Message(Bytes::from(self.0.to_vec()))
    }

    /// get the message type
    pub fn message_type(&self) -> MessageType {
        MessageType::try_from_u8(self.0[0])
            .expect("Should have at least one byte in the MessageSlice")
    }

    pub fn gossip_checked(&self) -> Result<Option<GossipSlice<'a>>> {
        if self.message_type() == MessageType::Gossip {
            GossipSlice::try_from_slice(&self.0[1..])
                .context("Unable to read a gossip from the given message")
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn topic_checked(&self) -> Result<Option<(Topic, &'a [u8])>> {
        if self.message_type() == MessageType::Topic {
            ensure!(
                self.0.len() > Topic::SIZE,
                "Not enough bytes in the message"
            );

            let topic_slice = &self.0[1..1 + Topic::SIZE];
            let topic = Topic::try_from(topic_slice)
                .context("Cannot parse the topic from the given message")?;

            let bytes = &self.0[1 + Topic::SIZE..];

            Ok(Some((topic, bytes)))
        } else {
            Ok(None)
        }
    }
}

impl AsRef<[u8]> for Message {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<'a> AsRef<[u8]> for MessageSlice<'a> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
