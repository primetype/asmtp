use anyhow::{bail, ensure, Context as _, Result};
use asmtp_lib::{PassportBlocks, PassportBlocksSlice, PassportImporter};
use keynesis::passport::{
    block::{BlockSlice, Hash},
    LightPassport,
};
use sled::IVec;
use std::{collections::BTreeMap, convert::TryFrom, fmt};

#[derive(Clone)]
pub struct Passports {
    blocks: sled::Tree,
    passport_id: sled::Tree,
    ids: sled::Tree,
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

    pub fn put_passport(&self, passport_blocks: PassportBlocksSlice) -> Result<LightPassport> {
        // check the passport is valid before doing anything
        let passport = PassportImporter::from_blocks(passport_blocks.iter())?;
        let id;

        // this should always be true if the above succeed

        let mut iter = passport_blocks.iter();

        if let Some(head) = iter.next() {
            self.put_block_head(head)?;
            id = head.header().hash();
            self.ids.insert(id, id.as_ref())?;
        } else {
            bail!("Cannot have a passport of empty blocks")
        }
        for block in iter {
            self.put_block_tail(&id, block)?;
        }

        self.blocks.flush()?;

        Ok(passport)
    }

    pub fn create_or_update(&self, block: BlockSlice) -> Result<LightPassport> {
        let parent = block.header().hash();

        if let Some(mut passport) = self.get(parent)? {
            // update the block
            passport
                .update(block)
                .context("Block cannot be applied to the existing passport")?;

            self.put_block_tail(&passport.id(), block)?;

            Ok(passport)
        } else {
            let passport = PassportImporter::new(block)?.finalize()?;

            self.put_block_head(block)?;

            Ok(passport)
        }
    }

    pub fn get_blocks(&self, id: Hash) -> Result<PassportBlocks<Vec<u8>>> {
        let iter = self.blocks.scan_prefix(&id);
        let mut blocks = PassportBlocks::new();
        for event in iter {
            let (_, event) = event?;
            let block = BlockSlice::try_from_slice(&mut event.as_ref())
                .context("Passport loaded from storage does not contains a valid state")?;
            blocks.push(block);
        }

        ensure!(!blocks.as_slice().is_empty(), "Passport not found");

        Ok(blocks)
    }

    pub fn get(&self, id: impl AsRef<[u8]>) -> Result<Option<LightPassport>> {
        let id = self
            .ids
            .get(id)
            .context("Failed to query persistent storage for passport's ID")?;
        if let Some(key) = id {
            let id = Hash::try_from(key.as_ref()).context("Invalid Passport ID")?;
            let blocks = self.get_blocks(id)?;

            let passport = PassportImporter::from_blocks(blocks.iter())?;

            Ok(Some(passport))
        } else {
            Ok(None)
        }
    }

    fn put_public_id<ID>(&self, public_id: ID, id: &Hash) -> Result<()>
    where
        ID: AsRef<[u8]> + fmt::Debug,
    {
        self.ids
            .insert(public_id.as_ref(), id.as_ref().to_vec())
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
