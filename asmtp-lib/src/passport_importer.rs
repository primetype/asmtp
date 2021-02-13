use anyhow::{bail, ensure, Result};
use keynesis::passport::{
    block::{Block, BlockSlice, Hash, Previous},
    LightPassport, Passport,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

/// load passport from unordered blocks
pub struct PassportImporter<B, P> {
    current: P,
    pending: HashMap<Hash, Vec<Rc<B>>>,
    added: HashSet<Hash>,
}

impl PassportImporter<Block, Passport> {
    pub fn new_owned(block: Block) -> Result<Self> {
        let current = Passport::new(block)?;

        Ok(Self {
            current,
            pending: HashMap::new(),
            added: HashSet::new(),
        })
    }

    pub fn from_blocks_owned(iter: impl IntoIterator<Item = Block>) -> Result<Passport> {
        let mut blocks = iter.into_iter();

        let mut importer = if let Some(head) = blocks.next() {
            Self::new_owned(head)?
        } else {
            bail!("invalid")
        };

        for block in blocks {
            importer.put(block)?;
        }

        importer.finalize()
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
            self.current.push(block)?;
            self.added.insert(id);

            let mut resolved = VecDeque::new();
            resolved.push_back(id);

            while let Some(id) = resolved.pop_front() {
                for children in self.take(&id) {
                    let id = children.header().hash();
                    self.current.push(children)?;
                    self.added.insert(id);
                    resolved.push_back(id);
                }
            }
        }

        Ok(())
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
}

impl<'a> PassportImporter<BlockSlice<'a>, LightPassport> {
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
}

impl<B, P> PassportImporter<B, P> {
    pub fn finalize(self) -> Result<P> {
        ensure!(
            self.pending.is_empty(),
            "Some blocks are still pending to be applied on the passport"
        );
        Ok(self.current)
    }

    fn take(&mut self, parent: &Hash) -> Vec<B> {
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
