use anyhow::{Context as _, Result};
use poldercast::Topic;
use std::future::Future;
use std::{
    convert::TryFrom as _,
    ops::RangeBounds,
    pin::Pin,
    task::{Context, Poll},
};

pub enum SubscriptionEvent {
    Subscribe(Topic),
    Unsubscribe(Topic),
}

/// store of topics
///
/// messages will be stored as follow:
///
/// ```text
/// topics::<topic id>
/// topics::<topic id>
/// ...
/// topics::<topic id>
/// ```
///
#[derive(Clone)]
pub struct Messages {
    messages: sled::Tree,
}

pub struct MessagesSubscriber(sled::Subscriber);

pub struct MessagesIter(sled::Iter);

impl Messages {
    pub fn open(db: &sled::Db) -> Result<Self> {
        let base = b"asmtp::topics::";

        let messages = db
            .open_tree(base)
            .context("Cannot open topic messages sub tree")?;

        Ok(Self { messages })
    }

    pub fn contains(&self, topic: &Topic) -> Result<bool> {
        self.messages
            .contains_key(topic)
            .context("failed to access the message's subtree")
    }

    pub fn insert(&self, topic: Topic) -> Result<()> {
        self.messages
            .insert(topic, &[])
            .map(|_| ())
            .context("Failed to insert new topic in the topic list")
    }

    pub fn remove(&self, topic: &Topic) -> Result<()> {
        self.messages
            .remove(topic)
            .map(|_| ())
            .context("Failed to remove topic from subtree")
    }

    /// get all the messages in the given time range
    pub fn range(&self, range: impl RangeBounds<Topic>) -> MessagesIter {
        MessagesIter(self.messages.range(range))
    }

    /// a subscriber to be notified on new messages written on this thread
    pub fn subscribe(&self) -> MessagesSubscriber {
        MessagesSubscriber(self.messages.watch_prefix(&[]))
    }
}

impl Iterator for MessagesIter {
    type Item = Topic;
    fn next(&mut self) -> Option<Self::Item> {
        let (last, _) = self.0.next()?.ok()?;
        Topic::try_from(last.as_ref()).ok()
    }
}

impl Future for MessagesSubscriber {
    type Output = Option<SubscriptionEvent>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut subscriber = self.as_mut();
        loop {
            let subscriber = Pin::new(&mut subscriber.0);
            match futures::ready!(subscriber.poll(cx)) {
                None => return Poll::Ready(None),
                Some(sled::Event::Remove { key }) => {
                    return Poll::Ready(
                        Topic::try_from(key.as_ref())
                            .ok()
                            .map(SubscriptionEvent::Unsubscribe),
                    )
                }
                Some(sled::Event::Insert { key, value }) => {
                    debug_assert!(
                        value.is_empty(),
                        "we are expecting all values to be empty here"
                    );
                    return Poll::Ready(
                        Topic::try_from(key.as_ref())
                            .ok()
                            .map(SubscriptionEvent::Subscribe),
                    );
                }
            }
        }
    }
}
