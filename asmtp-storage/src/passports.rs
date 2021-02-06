use anyhow::{bail, ensure, Context as _, Result};
use keynesis::passport::{
    block::{Block, BlockSlice, Hash, Previous},
    LightPassport,
};
use sled::IVec;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
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
pub struct PassportImporter<B> {
    current: LightPassport,
    pending: HashMap<Hash, Vec<Rc<B>>>,
    added: HashSet<Hash>,
}

impl<'a> PassportImporter<BlockSlice<'a>> {
    pub fn new(block: BlockSlice) -> Result<Self> {
        let current = LightPassport::new(block)?;

        Ok(Self {
            current,
            pending: HashMap::new(),
            added: HashSet::new(),
        })
    }

    pub fn from_blocks(iter: impl IntoIterator<Item = BlockSlice<'a>>) -> Result<LightPassport> {
        let mut blocks = iter.into_iter();

        let mut importer = if let Some(head) = blocks.next() {
            Self::new(head)?
        } else {
            bail!("invalid")
        };

        for block in blocks {
            importer.put(block)?;
        }

        importer.finalize()
    }

    pub fn load(&mut self, block: BlockSlice<'a>) -> Result<()> {
        let missing_parents = if let Previous::Previous(parent) = block.header().previous() {
            !self.added.contains(&parent)
        } else {
            bail!("Cannot have no parent block")
        };

        if missing_parents {
            self.put(block)?;
        } else {
            let id = block.header().hash();
            self.current.update(block)?;
            self.added.insert(id);

            let mut resolved = VecDeque::new();
            resolved.push_back(id);

            while let Some(id) = resolved.pop_front() {
                for children in self.take(&id) {
                    self.current.update(children)?;
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

    fn put(&mut self, block: BlockSlice<'a>) -> Result<()> {
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

    fn take(&mut self, parent: &Hash) -> Vec<BlockSlice<'a>> {
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
    pub fn search_ids<K>(&self, key: K) -> Result<BTreeMap<IVec, Hash>>
    where
        K: AsRef<[u8]>,
    {
        let k = self.ids.scan_prefix(key);
        let mut result = BTreeMap::new();

        for entry in k {
            let (key, value) = entry?;

            let hash = Hash::try_from(value.as_ref())?;
            result.insert(key, hash);
        }

        Ok(result)
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

    pub fn put_passport(&self, passport_blocks: &Vec<Block>) -> Result<Passport> {
        // check the passport is valid before doing anything
        let passport = PassportImporter::from_blocks(passport_blocks.iter().map(|b| b.as_slice()))?;

        // this should always be true if the above succeed
        let id = passport_blocks[0].header().hash();

        let mut iter = passport_blocks.into_iter();

        self.put_block_head(iter.next().unwrap().as_slice())?;
        for block in iter {
            self.put_block_tail(&id, block.as_slice())?;
        }

        Ok(Passport { passport, id })
    }

    pub fn create_or_update(&self, block: BlockSlice) -> Result<Passport> {
        let parent = block.header().hash();

        if let Some(mut passport) = self.get(parent)? {
            // update the block
            passport
                .passport
                .update(block)
                .context("Block cannot be applied to the existing passport")?;

            self.put_block_tail(&passport.id, block)?;

            Ok(passport)
        } else {
            let passport = PassportImporter::new(block)?.finalize()?;
            let id = block.header().hash();

            self.put_block_head(block)?;

            Ok(Passport { passport, id })
        }
    }

    pub fn get_blocks(&self, id: Hash) -> Result<Vec<Block>> {
        let iter = self.blocks.scan_prefix(&id);
        let mut blocks = Vec::new();
        for event in iter {
            let (_, event) = event?;
            let block = BlockSlice::try_from_slice(&mut event.as_ref())
                .context("Passport loaded from storage does not contains a valid state")?;
            blocks.push(block.to_block());
        }

        Ok(blocks)
    }

    pub fn get(&self, id: impl AsRef<[u8]>) -> Result<Option<Passport>> {
        let id = self
            .ids
            .get(id)
            .context("Failed to query persistent storage for passport's ID")?;
        if let Some(key) = id {
            let id = Hash::try_from(key.as_ref()).context("Invalid Passport ID")?;
            let blocks = self.get_blocks(id)?;

            let passport = PassportImporter::from_blocks(blocks.iter().map(|b| b.as_slice()))?;

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

        self.passport_id.insert(id.as_ref(), &[])?;
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
