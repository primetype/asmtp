use crate::codec::encryption::MAX_FRAME_LENGTH;
use anyhow::{anyhow, ensure, Context as _, Result};
use bytes::{BufMut as _, Bytes, BytesMut};
use keynesis::passport::{
    block::{Hash, Time},
    PassportBlocksSlice,
};
use poldercast::{GossipSlice, Topic};
use std::convert::{TryFrom, TryInto};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
#[repr(u8)]
pub enum MessageType {
    Gossip = 1,
    Topic = 2,

    PutPassport = 3,
    GetPassport = 4,

    RegisterTopic = 5,
    DeregisterTopic = 6,

    QueryTopicMessages = 7,
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
            3 => Some(Self::PutPassport),
            4 => Some(Self::GetPassport),
            5 => Some(Self::RegisterTopic),
            6 => Some(Self::DeregisterTopic),
            7 => Some(Self::QueryTopicMessages),

            0 | 8..=u8::MAX => None,
        }
    }
}

impl Message {
    const MAX_SIZE: usize = MAX_FRAME_LENGTH - MessageType::SIZE;
    const MIN_SIZE: usize = MessageType::SIZE + Hash::SIZE;

    /// create a new message from the given gossip
    pub fn new_gossip(gossip: GossipSlice<'_>) -> Self {
        let mut bytes = BytesMut::with_capacity(MessageType::SIZE + gossip.as_ref().len());
        bytes.reserve(MessageType::SIZE + gossip.as_ref().len());

        bytes.put_u8(MessageType::Gossip.to_u8());
        bytes.put_slice(gossip.as_ref());

        Self(bytes.freeze())
    }

    /// create a new message from the given topic and content
    pub fn new_topic(topic: Topic, message: impl AsRef<[u8]>) -> Self {
        let size = MessageType::SIZE + Topic::SIZE + message.as_ref().len();
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::Topic.to_u8());
        bytes.put_slice(topic.as_ref());
        bytes.put_slice(message.as_ref());

        Self(bytes.freeze())
    }

    pub fn new_get_passport(id: Hash) -> Self {
        let size = MessageType::SIZE + Hash::SIZE;
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::GetPassport.to_u8());
        bytes.put_slice(id.as_ref());

        Self(bytes.freeze())
    }

    pub fn new_put_passport(id: Hash, passport: PassportBlocksSlice) -> Self {
        let size = MessageType::SIZE + Hash::SIZE + passport.len();
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::PutPassport.to_u8());
        bytes.put_slice(id.as_ref());
        bytes.put_slice(passport.as_ref());

        Self(bytes.freeze())
    }

    pub fn new_register_topic(topic: Topic) -> Self {
        let size = MessageType::SIZE + Topic::SIZE;
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::RegisterTopic.to_u8());
        bytes.put_slice(topic.as_ref());

        Self(bytes.freeze())
    }

    pub fn new_deregister_topic(topic: Topic) -> Self {
        let size = MessageType::SIZE + Topic::SIZE;
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::DeregisterTopic.to_u8());
        bytes.put_slice(topic.as_ref());

        Self(bytes.freeze())
    }

    pub fn new_query_topic_messages(topic: Topic, time: Time) -> Self {
        let size = MessageType::SIZE + Topic::SIZE + Time::SIZE;
        let mut bytes = BytesMut::with_capacity(size);
        bytes.reserve(size);

        bytes.put_u8(MessageType::QueryTopicMessages.to_u8());
        bytes.put_slice(topic.as_ref());
        bytes.put_u32(*time);

        Self(bytes.freeze())
    }

    #[inline(always)]
    pub fn as_slice(&self) -> MessageSlice<'_> {
        MessageSlice(self.0.as_ref())
    }

    pub fn message_type(&self) -> MessageType {
        self.as_slice().message_type()
    }

    pub fn gossip_checked(&self) -> Option<GossipSlice<'_>> {
        self.as_slice()
            .gossip_checked()
            .expect("Expecting to have a valid Gossip message")
    }

    pub fn topic_checked(&self) -> Option<(Topic, &[u8])> {
        self.as_slice()
            .topic_checked()
            .expect("Expecting to have a valid topic message")
    }

    pub fn get_passport_checked(&self) -> Option<Hash> {
        self.as_slice()
            .get_passport()
            .expect("Expected a valid get passport message")
    }

    pub fn put_passport_checked(&self) -> Option<(Hash, PassportBlocksSlice<'_>)> {
        self.as_slice()
            .put_passport()
            .expect("Expected a valid put passport message")
    }

    pub fn register_topic_checked(&self) -> Option<Topic> {
        self.as_slice()
            .register_topic()
            .expect("Expected a valid topic registration message")
    }

    pub fn deregister_topic_checked(&self) -> Option<Topic> {
        self.as_slice()
            .deregister_topic()
            .expect("Expected a valid topic deregistration message")
    }

    pub fn query_topic_messages_checked(&self) -> Option<(Topic, Time)> {
        self.as_slice()
            .query_topic_messages()
            .expect("Expected a valid topic message query")
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
                message
                    .gossip_checked()?
                    .ok_or_else(|| anyhow!("Expected a gossip message"))?;
            }
            MessageType::Topic => {
                message
                    .topic_checked()?
                    .ok_or_else(|| anyhow!("Expected a topic message"))?;
            }
            MessageType::GetPassport => {
                message
                    .get_passport()?
                    .ok_or_else(|| anyhow!("Expected a get passport message"))?;
            }
            MessageType::PutPassport => {
                message
                    .put_passport()?
                    .ok_or_else(|| anyhow!("Expected a put passport message"))?;
            }
            MessageType::RegisterTopic => {
                message
                    .register_topic()?
                    .ok_or_else(|| anyhow!("Expected a topic registration message"))?;
            }
            MessageType::DeregisterTopic => {
                message
                    .deregister_topic()?
                    .ok_or_else(|| anyhow!("Expected a topic deregistration message"))?;
            }
            MessageType::QueryTopicMessages => {
                message
                    .query_topic_messages()?
                    .ok_or_else(|| anyhow!("Expected a query of topic message"))?;
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

    pub fn get_passport(self) -> Result<Option<Hash>> {
        if self.message_type() == MessageType::GetPassport {
            let hash = &self.0[1..];
            Hash::try_from(hash)
                .context("Not enough bytes for a passport ID")
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn put_passport(self) -> Result<Option<(Hash, PassportBlocksSlice<'a>)>> {
        if self.message_type() == MessageType::PutPassport {
            let hash = &self.0[1..1 + Hash::SIZE];
            let id = Hash::try_from(hash).context("Not enough bytes for a passport ID")?;

            let blocks = PassportBlocksSlice::try_from_slice(&self.0[1 + Hash::SIZE..])?;

            Ok(Some((id, blocks)))
        } else {
            Ok(None)
        }
    }

    pub fn register_topic(self) -> Result<Option<Topic>> {
        if self.message_type() == MessageType::RegisterTopic {
            let topic = &self.0[1..1 + Topic::SIZE];
            let topic = Topic::try_from(topic).context("Not enough bytes for a Topic")?;

            Ok(Some(topic))
        } else {
            Ok(None)
        }
    }

    pub fn deregister_topic(self) -> Result<Option<Topic>> {
        if self.message_type() == MessageType::DeregisterTopic {
            let topic = &self.0[1..1 + Topic::SIZE];
            let topic = Topic::try_from(topic).context("Not enough bytes for a Topic")?;

            Ok(Some(topic))
        } else {
            Ok(None)
        }
    }

    pub fn query_topic_messages(self) -> Result<Option<(Topic, Time)>> {
        if self.message_type() == MessageType::QueryTopicMessages {
            let topic = &self.0[1..1 + Topic::SIZE];
            let topic = Topic::try_from(topic).context("Not enough bytes for a Topic")?;

            let time = u32::from_be_bytes(self.0[1 + Topic::SIZE..].try_into().unwrap()).into();

            Ok(Some((topic, time)))
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
        &self.0
    }
}
