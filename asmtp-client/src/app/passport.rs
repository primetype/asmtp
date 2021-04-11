use keynesis::{
    key::ed25519::PublicKey,
    passport::{self, block::Hash, PassportBlocksSlice},
};
use std::{
    collections::HashSet,
    fmt::{self, Formatter},
    sync::Arc,
};

pub struct Passport {
    passport: passport::Passport,
}

impl Passport {
    pub fn new(passport: passport::Passport) -> Self {
        Self { passport }
    }

    pub fn id(&self) -> Hash {
        self.passport.id()
    }

    pub fn keys(&self) -> &HashSet<Arc<PublicKey>> {
        self.passport.active_master_keys()
    }

    pub fn blocks(&self) -> PassportBlocksSlice<'_> {
        self.passport.blocks()
    }
}

impl fmt::Debug for Passport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Passport")
            .field("id", &self.id())
            .field("storage", &"...")
            .finish()
    }
}
