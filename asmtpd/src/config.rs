use crate::{network, secret, storage};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};
use structopt::StructOpt;

#[derive(Debug, PartialEq, Eq, Clone, StructOpt, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[structopt(flatten)]
    #[serde(default)]
    pub secret: secret::Config,

    #[structopt(flatten)]
    #[serde(default)]
    pub network: network::Config,

    #[structopt(flatten)]
    #[serde(default)]
    pub storage: storage::Config,

    #[structopt(skip)]
    pub users: HashSet<String>,
}

impl Config {
    pub const EXAMPLE: &'static str = include_str!("config.yaml");

    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let file = std::fs::File::open(path)
            .with_context(|| format!("Cannot open file: {}", path.display()))?;
        serde_yaml::from_reader(file)
            .with_context(|| format!("Invalid config file: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_example() {
        let example = Config::EXAMPLE;

        let _: Config = serde_yaml::from_str(example).expect("Valid example");
    }
}
