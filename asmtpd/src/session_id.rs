use std::{fmt, str::FromStr};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct SessionId(pub(crate) [u8; 64]);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(&self.0))
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SessionId")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

impl FromStr for SessionId {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut session_id = SessionId([0; 64]);
        hex::decode_to_slice(s, &mut session_id.0)?;
        Ok(session_id)
    }
}
