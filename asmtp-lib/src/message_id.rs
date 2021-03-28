use cryptoxide::blake2b::Blake2b;
use keynesis::passport::block::Time;
use serde::{Deserialize, Serialize};
use std::{
    array::TryFromSliceError,
    convert::{TryFrom, TryInto as _},
    fmt::{self, Formatter},
    ops::{Bound, RangeBounds},
    str::FromStr,
};

/// the identifier of the message
///
/// the identifier is composed of 2 part. the time it has been **received** at and the
/// cryptographic hash.
///
/// The message id is not to be shared across the public network. However it can be
/// used to store messages in a local storage.
///
/// The construction of the `MessageId` is such that it is easy to search messages
/// by time range in a Key Value database (the time is stored as big endian so the
/// ordering is kept consistent).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageId([u8; Self::SIZE]);

impl MessageId {
    const HASH_SIZE: usize = 16;
    const TIME_SIZE: usize = Time::SIZE;
    pub const SIZE: usize = Self::HASH_SIZE + Self::TIME_SIZE;

    /// create a new [`MessageId`] from the given byte slice
    ///
    /// the time will be the current time
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        let mut message_id = Self::timed(Time::now());

        Blake2b::blake2b(&mut message_id.0[Self::TIME_SIZE..], bytes.as_ref(), &[]);

        message_id
    }

    #[inline(always)]
    fn timed(time: Time) -> Self {
        let mut message_id = [0u8; Self::SIZE];
        message_id[..Self::TIME_SIZE].copy_from_slice(&time.to_be_bytes());

        Self(message_id)
    }

    /// access the hash component of the [`MessageId`]
    pub fn hash(&self) -> &[u8] {
        &self.0[Self::TIME_SIZE..]
    }

    /// access the [`Time`] component of the [`MessageId`]
    pub fn time(&self) -> Time {
        Time::from(u32::from_be_bytes(
            self.0[..Self::TIME_SIZE]
                .try_into()
                .expect("4 bytes of BE encoded u16"),
        ))
    }

    /// convert a [`Time`] range into a [`MessageId`] range. That way it is convenient
    /// to query messages by arrival time when querying the database
    ///
    pub fn time_range(range: impl RangeBounds<Time>) -> impl RangeBounds<Self> {
        let start = match range.start_bound() {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(t) => Bound::Included(Self::timed(*t)),
            Bound::Excluded(t) => Bound::Excluded(Self::timed(*t)),
        };

        let end = match range.end_bound() {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(t) => Bound::Included(Self::timed(*t)),
            Bound::Excluded(t) => Bound::Excluded(Self::timed(*t)),
        };

        (start, end)
    }
}

impl AsRef<[u8]> for MessageId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<[u8; Self::SIZE]> for MessageId {
    fn from(bytes: [u8; Self::SIZE]) -> Self {
        Self(bytes)
    }
}

impl From<MessageId> for [u8; MessageId::SIZE] {
    fn from(message_id: MessageId) -> Self {
        message_id.0
    }
}

impl From<MessageId> for String {
    fn from(message_id: MessageId) -> Self {
        message_id.to_string()
    }
}

impl<'a> TryFrom<&'a [u8]> for MessageId {
    type Error = TryFromSliceError;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

impl<'a> TryFrom<&'a str> for MessageId {
    type Error = <Self as FromStr>::Err;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl TryFrom<String> for MessageId {
    type Error = <Self as FromStr>::Err;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}

impl fmt::Debug for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MessageId")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        hex::encode(&self.0).fmt(f)
    }
}

impl FromStr for MessageId {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0; Self::SIZE];
        hex::decode_to_slice(s, &mut bytes)?;
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// tests that the order is kept by increasing time which ever is the
    /// composition of the following hash
    #[test]
    fn ordering_kept() {
        let t1 = Time::from(0x200FF);
        let t2 = Time::from(0x40000);

        let bytes1 = [0xFF; MessageId::HASH_SIZE];
        let bytes2 = [0x08; MessageId::HASH_SIZE];

        let mut message_id1 = MessageId::timed(t1);
        message_id1.0[MessageId::TIME_SIZE..].copy_from_slice(&bytes1);
        let mut message_id2 = MessageId::timed(t2);
        message_id2.0[MessageId::TIME_SIZE..].copy_from_slice(&bytes2);

        // test initial assumptions
        assert!(t1 < t2);
        assert!(bytes1 > bytes2);
        assert!(message_id1 < message_id2);
    }
}
