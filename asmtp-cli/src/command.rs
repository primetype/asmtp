use anyhow::bail;
use std::{
    fmt::{self, Formatter},
    str::FromStr,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Command {
    Sync,
    Info,
    Exit,
    Help,
    Message,
    Open,
}

impl Command {
    pub fn help_about(&self) -> String {
        let m = match self {
            Self::Info => "print the local information",
            Self::Sync => "sync the local passport with the remote peer",
            Self::Exit => "terminate the current program",
            Self::Help => "list all the commands",
            Self::Message => "Send a message to one of your buddies",
            Self::Open => "Open an encrypted message",
        };

        m.to_owned()
    }

    pub const ALL: &'static [Self] = &[
        Self::Info,
        Self::Sync,
        Self::Exit,
        Self::Help,
        Self::Message,
        Self::Open,
    ];
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let r = match self {
            Self::Info => "info",
            Self::Sync => "sync",
            Self::Exit => "exit",
            Self::Help => "help",
            Self::Message => "message",
            Self::Open => "open",
        };

        r.fmt(f)
    }
}

impl FromStr for Command {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "exit" | "quit" | "q" => Ok(Command::Exit),
            "info" => Ok(Command::Info),
            "sync" => Ok(Command::Sync),
            "message" => Ok(Command::Message),
            "open" => Ok(Command::Open),
            "?" | "help" => Ok(Command::Help),
            _ => bail!(
                "Unknown command: {:?} (try \"{}\" for the list of commands)",
                s,
                Self::Help
            ),
        }
    }
}
