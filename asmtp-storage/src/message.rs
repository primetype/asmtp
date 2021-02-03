use anyhow::{Context as _, Result};
use asmtp_lib::MessageId;
use futures::Future;
use keynesis::passport::block::Time;
use poldercast::Topic;
use sled::IVec;
use std::{
    convert::TryFrom as _,
    ops::RangeBounds,
    pin::Pin,
    task::{Context, Poll},
};

/// store of topic messages
///
/// messages will be stored as follow:
///
/// ```text
/// topic::<topic id>::message::<message_id> = <message>
/// topic::<topic id>::message::<message_id> = <message>
/// ...
/// topic::<topic id>::message::<message_id> = <message>
/// ```
///
pub struct Message {
    messages: sled::Tree,
    id: Topic,
}

pub struct MessageSubscriber(sled::Subscriber);

pub struct MessageIter(sled::Iter);

impl Message {
    /// open or create a new `Tree` for the given `Topic` ID
    ///
    pub fn open(db: &sled::Db, id: Topic) -> Result<Self> {
        let base = {
            let mut vec = b"asmtp::topic::".to_vec();

            vec.extend_from_slice(id.as_ref());
            vec
        };
        let messages = {
            let mut name = base.clone();
            name.extend_from_slice(b"::message::");
            db.open_tree(base)?
        };

        Ok(Self { messages, id })
    }

    pub fn insert(&self, message: impl AsRef<[u8]>) -> Result<MessageId> {
        let message_id = MessageId::new(&message);

        self.messages
            .insert(&message_id, message.as_ref())
            .context("Failed to insert the topic message")?;
        Ok(message_id)
    }

    pub fn last_message_id(&self) -> Result<Option<MessageId>> {
        if let Some((last, _)) = self.messages.last()? {
            let message_id = MessageId::try_from(last.as_ref()).expect("we only have message id");
            Ok(Some(message_id))
        } else {
            Ok(None)
        }
    }

    /// get all the messages in the given time range
    pub fn range_time(&self, range: impl RangeBounds<Time>) -> MessageIter {
        let range = MessageId::time_range(range);
        MessageIter(self.messages.range(range))
    }

    /// remove all timed items in the given time range
    pub fn remove_range(&self, range: impl RangeBounds<Time>) -> Result<()> {
        let mut batch = sled::Batch::default();
        for (message_id, _) in self.range_time(range) {
            batch.remove(message_id.as_ref());
        }

        self.messages
            .apply_batch(batch)
            .context("Failed to remove messages from the storage")
    }

    pub fn topic(&self) -> &Topic {
        &self.id
    }

    /// a subscriber to be notified on new messages written on this thread
    pub fn subscribe(&self) -> MessageSubscriber {
        MessageSubscriber(self.messages.watch_prefix(&[]))
    }

    /// erase all the messages from the db
    pub fn clear(self) -> Result<()> {
        self.messages
            .clear()
            .context("Failed to remove the topic from the storage")?;
        Ok(())
    }
}

impl Iterator for MessageIter {
    type Item = (MessageId, IVec);
    fn next(&mut self) -> Option<Self::Item> {
        let (last, message) = self.0.next()?.ok()?;
        MessageId::try_from(last.as_ref())
            .ok()
            .map(|id| (id, message))
    }
}

impl Future for MessageSubscriber {
    type Output = Option<MessageId>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut subscriber = self.as_mut();
        loop {
            let subscriber = Pin::new(&mut subscriber.0);
            match futures::ready!(subscriber.poll(cx)) {
                None => return Poll::Ready(None),
                Some(sled::Event::Remove { .. }) => {
                    continue;
                }
                Some(sled::Event::Insert { key, value: _ }) => {
                    return Poll::Ready(MessageId::try_from(key.as_ref()).ok())
                }
            }
        }
    }
}
