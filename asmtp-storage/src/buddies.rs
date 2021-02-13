use anyhow::{Context as _, Result};
use keynesis::passport::block::Hash;
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    fmt::{self, Formatter},
    str::FromStr,
};

#[derive(Clone)]
pub struct Buddies {
    buddies: sled::Tree,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct Buddy(String);

impl Buddies {
    const BUDDIES: &'static str = "asmtp::buddies::";

    pub fn new(db: &sled::Db) -> Result<Self> {
        let buddies = db
            .open_tree(Self::BUDDIES)
            .context("Failed to open buddies sub tree")?;

        Ok(Self { buddies })
    }

    pub fn search(&self, alias: impl AsRef<[u8]>) -> Result<BTreeMap<Buddy, Hash>> {
        let iter = self.buddies.scan_prefix(alias);

        let mut result = BTreeMap::new();
        for entry in iter {
            let (alias, id) = entry.context("Failed to fetch buddy entry from DB")?;
            let alias = Buddy(String::from_utf8_lossy(alias.as_ref()).into_owned());
            let id = Hash::try_from(id.as_ref()).context("entry contains invalid Hash")?;

            result.insert(alias, id);
        }

        Ok(result)
    }

    pub fn contains(&self, buddy: impl AsRef<[u8]>) -> Result<bool> {
        self.buddies
            .contains_key(buddy)
            .context("Failed to query the buddy list from the persistent storage")
    }

    pub fn get(&self, buddy: impl AsRef<[u8]>) -> Result<Option<Hash>> {
        let hash = self
            .buddies
            .get(buddy)
            .context("Failed to query the buddy list from persistent storage")?;

        let r = if let Some(hash) = hash {
            Some(Hash::try_from(hash.as_ref()).context("DB contains an invalid passport id")?)
        } else {
            None
        };

        Ok(r)
    }

    pub fn insert(&self, buddy: &Buddy, hash: Hash) -> Result<()> {
        self.buddies
            .insert(buddy.as_ref(), hash.as_ref())
            .context("Failed to save new buddy")
            .map(|_| ())
    }
}

impl AsRef<[u8]> for Buddy {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Display for Buddy {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Buddy {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Buddy(s.to_owned()))
    }
}

impl From<String> for Buddy {
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl<'a> From<&'a str> for Buddy {
    fn from(s: &'a str) -> Self {
        Self(s.to_owned())
    }
}
