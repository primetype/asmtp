use crate::app::{Key, KeyFile};
use anyhow::{bail, Context as _, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Keys {
    directory: PathBuf,
    keys: Vec<Key>,
}

impl Keys {
    pub fn new<P>(directory: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let directory = directory.as_ref();

        if directory.is_file() {
            bail!("Expecting a directory, not a file: {}", directory.display())
        } else if !directory.exists() {
            fs::create_dir_all(directory).with_context(|| {
                format!("Failed to create keys directory ({})", directory.display())
            })?
        }

        let mut keys = Vec::new();
        for entry in fs::read_dir(directory)
            .with_context(|| format!("Failed to list files in {}", directory.display()))?
        {
            let entry = entry.with_context(|| {
                format!("Failed to read entry in directory {}", directory.display())
            })?;
            let path = entry.path();

            let key = Key::open(&path)
                .with_context(|| format!("Failed to open key {}", entry.path().display()))?;
            keys.push(key);
        }

        let directory = directory.to_path_buf();
        Ok(Self { keys, directory })
    }

    pub fn add_key(&mut self, config: KeyFile) -> Result<usize> {
        let path = self.directory.join(format!("{}.key", self.keys.len()));
        let key = Key::create(&path, config)
            .with_context(|| format!("Failed to create key {}", path.display()))?;
        self.keys.push(key);
        Ok(self.len() - 1)
    }

    pub fn keys(&self) -> &[Key] {
        self.keys.as_slice()
    }

    pub fn keys_mut(&mut self) -> &mut [Key] {
        self.keys.as_mut_slice()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Key> {
        self.keys.iter()
    }
}
