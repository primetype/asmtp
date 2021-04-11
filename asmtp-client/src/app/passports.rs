use crate::app::Passport;
use anyhow::Result;
use keynesis::{key::ed25519::PublicKey, passport::block::Hash};
use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct Passports {
    passports: HashMap<Hash, Passport>,
    keys: HashMap<PublicKey, HashSet<Hash>>,
}

impl Passports {
    pub fn new() -> Result<Self> {
        let passports = HashMap::new();
        let keys = HashMap::new();
        Ok(Self { passports, keys })
    }

    pub fn is_empty(&self) -> bool {
        self.passports.is_empty()
    }

    pub fn len(&self) -> usize {
        self.passports.len()
    }

    pub fn get_by_id(&self, id: &Hash) -> Option<&Passport> {
        self.passports.get(id)
    }

    pub fn contains_key(&self, key: &PublicKey) -> bool {
        self.keys.contains_key(key)
    }

    pub fn get_by_key(&self, key: &PublicKey) -> Option<impl Iterator<Item = &Passport>> {
        let ids = self.keys.get(key)?;

        Some(ids.iter().filter_map(move |id| self.passports.get(id)))
    }

    pub fn insert(&mut self, passport: Passport) {
        let id = passport.id();

        for key in passport.keys() {
            self.keys.entry(*key.as_ref()).or_default().insert(id);
        }

        match self.passports.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(passport);
            }
            Entry::Occupied(mut entry) => {
                // the passport is already present
                // this could mean it has been updated
                entry.insert(passport);
            }
        }
    }
}
