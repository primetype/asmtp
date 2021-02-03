use anyhow::{bail, ensure, Context as _, Result};
use keynesis::passport::{
    block::{Block, BlockSlice, Hash, Previous},
    LightPassport,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::TryFrom,
    fmt,
    rc::Rc,
};

#[derive(Clone)]
pub struct Passports {
    blocks: sled::Tree,
    passport_id: sled::Tree,
    ids: sled::Tree,
}

pub struct Passport {
    passport: LightPassport,
    id: Hash,
}

/// load passport from unordered blocks
pub struct PassportImporter {
    current: LightPassport,
    pending: HashMap<Hash, Vec<Rc<Block>>>,
    added: HashSet<Hash>,
}

impl PassportImporter {
    pub fn new(block: BlockSlice) -> Result<Self> {
        let current = LightPassport::new(block)?;

        Ok(Self {
            current,
            pending: HashMap::new(),
            added: HashSet::new(),
        })
    }

    pub fn load(&mut self, block: Block) -> Result<()> {
        let missing_parents = if let Previous::Previous(parent) = block.header().previous() {
            !self.added.contains(&parent)
        } else {
            bail!("Cannot have no parent block")
        };

        if missing_parents {
            self.put(block)?;
        } else {
            let id = block.header().hash();
            self.current.update(block.as_slice())?;
            self.added.insert(id);

            let mut resolved = VecDeque::new();
            resolved.push_back(id);

            while let Some(id) = resolved.pop_front() {
                for children in self.take(&id) {
                    self.current.update(children.as_slice())?;
                    self.added.insert(children.header().hash());
                    resolved.push_back(children.header().hash());
                }
            }
        }

        Ok(())
    }

    pub fn finalize(self) -> Result<LightPassport> {
        ensure!(
            self.pending.is_empty(),
            "Some blocks are still pending to be applied on the passport"
        );
        Ok(self.current)
    }

    fn put(&mut self, block: Block) -> Result<()> {
        let block = Rc::new(block);

        match block.header().previous() {
            Previous::None => bail!("Cannot add a block without a parent block"),
            Previous::Previous(parent) => {
                self.pending
                    .entry(parent)
                    .or_default()
                    .push(Rc::clone(&block));
                Ok(())
            }
        }
    }

    fn take(&mut self, parent: &Hash) -> Vec<Block> {
        let mut result = Vec::new();

        if let Some(children) = self.pending.remove(parent) {
            for child in children {
                if let Ok(event) = Rc::try_unwrap(child) {
                    result.push(event)
                }
            }
        }

        result
    }
}

impl Passport {
    pub fn light_passport(&self) -> &LightPassport {
        &self.passport
    }

    pub fn id(&self) -> Hash {
        self.id
    }
}

impl Passports {
    const PASSPORT_BLOCKS: &'static str = "asmtp::passport::block::";
    const PASSPORT_ID: &'static str = "asmtp::passport::id::";
    const PASSPORT_IDS: &'static str = "asmtp::passport::ids::";

    pub fn new(db: &sled::Db) -> Result<Self> {
        let blocks = db
            .open_tree(Self::PASSPORT_BLOCKS)
            .with_context(|| format!("Cannot open sled::Tree \"{}\"", Self::PASSPORT_BLOCKS))?;
        let passport_id = db
            .open_tree(Self::PASSPORT_BLOCKS)
            .with_context(|| format!("Cannot open sled::Tree \"{}\"", Self::PASSPORT_ID))?;
        let ids = db
            .open_tree(Self::PASSPORT_IDS)
            .with_context(|| format!("Cannot open sled::Tree \"{}\"", Self::PASSPORT_IDS))?;

        Ok(Self {
            blocks,
            passport_id,
            ids,
        })
    }

    /// search a passport ID with the given prefix
    ///
    /// this function will scan through the public key or block id
    /// id to retrieve one or multiple possible passport ID
    pub fn search_ids<K>(&self, key: K) -> sled::Iter
    where
        K: AsRef<[u8]>,
    {
        self.ids.scan_prefix(key)
    }

    pub fn all_passports(&self) -> Result<Vec<Hash>> {
        let mut passports = Vec::new();

        for p in self.passport_id.iter() {
            let (h, _) = p?;
            let hash = Hash::try_from(h.as_ref())
                .context("Cannot read passport ID from the persistent storage")?;

            passports.push(hash);
        }

        Ok(passports)
    }

    pub fn create_or_update(&self, block: BlockSlice) -> Result<Passport> {
        let parent = block.header().hash();

        if let Some(mut passport) = self.get(parent)? {
            // update the block
            passport
                .passport
                .update(block)
                .context("Block cannot be applied to the existing passport")?;

            self.passport_id.insert(passport.id.as_ref(), &[])?;
            self.put_block_tail(&passport.id, block)?;

            Ok(passport)
        } else {
            let passport = PassportImporter::new(block)?.finalize()?;
            let id = block.header().hash();

            self.put_block_head(block)?;

            Ok(Passport { passport, id })
        }
    }

    pub fn get(&self, id: impl AsRef<[u8]>) -> Result<Option<Passport>> {
        let id = self
            .ids
            .get(id)
            .context("Failed to query persistent storage for passport's ID")?;
        if let Some(key) = id {
            let id = Hash::try_from(key.as_ref()).context("Invalid Passport ID")?;
            let mut iter = self.blocks.scan_prefix(&key);

            let mut passport = if let Some(first) = iter.next() {
                let (_, first) = first?;
                let event = BlockSlice::try_from_slice(&mut first.as_ref())
                    .context("Passport loaded from storage does not contains a valid state")?;

                PassportImporter::new(event)?
            } else {
                return Ok(None);
            };

            for event in iter {
                let (_, event) = event?;
                let block = BlockSlice::try_from_slice(&mut event.as_ref())
                    .context("Passport loaded from storage does not contains a valid state")?;

                passport.load(block.to_block())?;
            }

            let passport = passport.finalize()?;

            Ok(Some(Passport { id, passport }))
        } else {
            Ok(None)
        }
    }

    fn put_public_id<ID>(&self, public_id: ID, id: &Hash) -> Result<()>
    where
        ID: AsRef<[u8]> + fmt::Debug,
    {
        self.ids
            .insert(public_id.as_ref(), id.to_string().as_bytes())
            .with_context(|| {
                format!(
                    "Failed to set the public identity ({:?} - {})",
                    public_id, id
                )
            })?;
        Ok(())
    }

    fn put_block_head(&self, block: BlockSlice) -> Result<()> {
        let id = block.header().hash();

        self.put_block(&id, &id, block)
    }

    fn put_block_tail(&self, id: &Hash, block: BlockSlice) -> Result<()> {
        let mut key = id.as_ref().to_vec();
        key.extend(b".");
        key.extend(block.header().hash().as_ref());

        self.put_block(key, id, block)
    }

    fn put_block(&self, key: impl AsRef<[u8]>, id: &Hash, block: BlockSlice) -> Result<()> {
        self.blocks.insert(key, block.as_ref())?;

        // we have added a new block, add the associated metadata of the block in our DB
        for content in block.content().iter() {
            if let Some(entry) = content.register_master_key() {
                self.put_public_id(entry.key(), id)?;
            } else if let Some(entry) = content.deregister_master_key() {
                self.ids.remove(entry.key())?;
            } else if let Some(entry) = content.set_shared_key() {
                self.put_public_id(entry.key(), id)?;
            }
        }

        Ok(())
    }
}
