use anyhow::{anyhow, bail, ensure, Context, Result};
use asmtp_cli::{Command, Settings, State, Style};
use asmtp_lib::{mk_topic, PassportBlocksSlice};
use asmtp_network::{net::Connection, Message};
use asmtp_storage::Buddy;
use cryptoxide::blake2b::Blake2b;
use dialoguer::{Confirm, Editor, Input, Password, Select};
use futures::{SinkExt, StreamExt};
use indicatif::ProgressBar;
use keynesis::{passport::block::Hash, Seed};
use poldercast::Topic;
use rand::rngs::OsRng;
use std::{collections::BTreeMap, path::Path};
use tokio::time::timeout;

#[tokio::main]
async fn main() {
    let style = Style::default();

    if let Err(error) = try_main(&style).await {
        report_error(&style, error);
        std::process::exit(1)
    }
}

async fn try_main(style: &Style) -> anyhow::Result<()> {
    let settings = Settings::gather()?;

    let mut state = State::load(settings).context("Failed to load state")?;

    if let Some(path) = state.settings().import_passport().cloned() {
        import_buddy(style, &mut state, &path)
            .with_context(|| format!("Failed to import passport {}", path.display()))?;
        return Ok(());
    }

    // make sure there is a device alias for this machine
    if !state.has_alias() {
        ask_device_alias(style, &mut state).context("Device's alias is mandatory")?;
    }

    // check there is a key (Though we create one by default already otherwise)
    if !state.has_key() {
        bail!("Even if the device does not have a key,it should have been generated")
    }

    // if there is no passport, we can check we are in match
    if !state.has_passport() {
        ask_passport(style, &mut state)
            .context("You will need a public passport to interact with others")?;
    }

    // check that the current device key is actually part of the active passport
    let passport = state.passport().unwrap();
    let public_key = state.public_key().unwrap();
    if !passport.active_master_keys().contains(&public_key) {
        bail!("It seems our device's key is no longer in the passport's active master key")
    }

    if let Some(path) = state.settings().export_passport().cloned() {
        export_buddy(style, &mut state, &path)
            .with_context(|| format!("Cannot export buddy passport to {}", path.display()))?;
        return Ok(());
    }

    let mut connection = None;

    loop {
        match prompt_command(style, &mut state) {
            Err(error) => report_error(style, error),
            Ok(Command::Exit) => break,
            Ok(Command::Help) => {
                for cmd in Command::ALL {
                    eprintln!(
                        "{}: {}",
                        style.dialoguer.values_style.apply_to(cmd),
                        cmd.help_about()
                    )
                }
            }
            Ok(Command::Message) => {
                let connection = if let Some(connection) = connection.as_mut() {
                    connection
                } else {
                    connection = Some(connect(style, &mut state).await?);
                    connection.as_mut().unwrap()
                };
                send_messages(style, &mut state, connection)
                    .await
                    .context("failed to send message to peer")?
            }
            Ok(Command::Info) => {
                //
                info(style, &mut state).context("Failed to print the info")?
            }
            Ok(Command::Open) => {
                open_message(style, &mut state).await?;
            }
            Ok(Command::Sync) => {
                let connection = if let Some(connection) = connection.as_mut() {
                    connection
                } else {
                    connection = Some(connect(style, &mut state).await?);
                    connection.as_mut().unwrap()
                };

                // submit the passport
                submit_passport(style, &mut state, connection).await?;

                // submit the buddy list so we can "subscribe to their update"
                // submit the topic list so we can expect messages from them peers
                submit_subscriptions(style, &mut state, connection).await?;

                // query for last messages
                sync_new_messages(style, &mut state, connection).await?;
            }
        }

        state.save()?;
    }

    state.save()?;

    Ok(())
}

fn prompt_command(style: &Style, state: &mut State) -> Result<Command> {
    Input::with_theme(&style.dialoguer)
        .allow_empty(false)
        .with_prompt(format!("({})", state.id().unwrap()))
        .interact()
        .context("Failed to query the command to run")
}

fn ask_device_alias(style: &Style, state: &mut State) -> Result<()> {
    let alias: String = Input::with_theme(&style.dialoguer)
        .allow_empty(false)
        .with_prompt("Set the device's name")
        .interact()
        .context("Failed to query the device's name")?;

    state.set_alias(alias);
    state.save().context("Failed to save state")?;

    Ok(())
}

fn report_error(style: &Style, error: anyhow::Error) {
    let mut chain = error.chain().rev();
    if let Some(head) = chain.next() {
        eprintln!(
            "{} {}",
            style.dialoguer.error_prefix,
            style.dialoguer.error_style.clone().bold().apply_to(head)
        );
    }

    if chain.len() > 0 {
        eprintln!();
    }

    for detail in chain {
        eprintln!(
            "{} caused by: {}",
            style.dialoguer.error_prefix,
            style.dialoguer.error_style.apply_to(detail)
        );
    }
}

fn ask_passport(style: &Style, state: &mut State) -> Result<()> {
    #[derive(Clone, Copy)]
    enum Choice {
        New,
    }
    impl std::fmt::Display for Choice {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                Self::New => f.write_str("New passport"),
            }
        }
    }
    const CHOICES: &[Choice] = &[Choice::New];
    let choice = Select::with_theme(&style.dialoguer)
        .items(CHOICES)
        .with_prompt("How to connect your new device")
        .interact()
        .context("Failed to query the passport creation method")?;
    let choice = CHOICES[choice];

    match choice {
        Choice::New => {
            create_new_passport(style, state)?;
        }
    }

    Ok(())
}

fn export_buddy<P: AsRef<Path>>(style: &Style, state: &mut State, path: P) -> Result<()> {
    let buddies = state.buddies().search(&[])?;

    let (buddy, hash) = match buddies.len().cmp(&1) {
        std::cmp::Ordering::Greater => {
            let aliases: Vec<_> = buddies.iter().map(|(n, _)| n).collect();

            let buddy = Select::with_theme(&style.dialoguer)
                .with_prompt("Select the passport to export")
                .items(&aliases)
                .paged(true)
                .interact()
                .context("Failed to query which buddy passport to export")?;

            buddies.get_key_value(aliases[buddy]).unwrap()
        }
        std::cmp::Ordering::Equal => {
            let (buddy, hash) = buddies.iter().next().unwrap();

            (buddy, hash)
        }
        std::cmp::Ordering::Less => bail!("no passport to export"),
    };

    let confirmation = Confirm::with_theme(&style.dialoguer)
        .with_prompt(format!(
            "Exporting {buddy} ({hash}). Are you sure?",
            buddy = style.alias.apply_to(buddy),
            hash = style.passport.apply_to(hash),
        ))
        .interact()
        .context("Failed to query agreement from user")?;

    if confirmation {
        let passport = state
            .passports()
            .get_blocks(*hash)
            .context("failed to query the passport's block from the persistent storage")?;

        ensure!(
            !passport.as_slice().is_empty(),
            "We should have the passport's block in the storage"
        );

        eprintln!(
            "exporting {buddy}'s passport ({hash})",
            buddy = style.alias.apply_to(buddy),
            hash = style.passport.apply_to(hash),
        );
        std::fs::write(path, passport.as_ref()).context("Failed to export passport to file")?;
    }

    Ok(())
}

fn import_buddy<P: AsRef<Path>>(style: &Style, state: &mut State, path: P) -> Result<()> {
    let bytes = std::fs::read(path).context("Failed to read passport")?;
    let blocks = PassportBlocksSlice::try_from_slice(&bytes).context("Invalid passport file")?;
    ensure!(!blocks.is_empty(), "Cannot import an empty passport");
    let id = blocks.get(0).unwrap().header().hash();

    eprintln!("Importing passport {}", style.passport.apply_to(id));

    if state
        .passports()
        .get(&id)
        .context("Failed to query persistent storage")?
        .is_none()
    {
        let buddy_name: Buddy = Input::with_theme(&style.dialoguer)
            .with_prompt("passport name (never shared)")
            .interact()
            .context("Failed to query for the passport's name")?;

        state.buddies().insert(&buddy_name, id)?
    }

    let passport = state.passports().put_passport(blocks)?;
    if let Some((_, their_key)) = passport.shared_key() {
        let (_, our_key) = state.passport().unwrap().shared_key().unwrap();
        let topic = mk_topic(our_key, their_key);
        state.messages().insert(topic)?;

        eprintln!(
            "registering topic ({topic}) with {id}",
            topic = style.topic.apply_to(topic),
            id = style.passport.apply_to(id)
        );
    }

    Ok(())
}

fn create_new_passport(style: &Style, state: &mut State) -> Result<()> {
    let buddy_name = Input::with_theme(&style.dialoguer)
        .with_prompt("passport name (never shared)")
        .default(Buddy::from(whoami::username()))
        .interact()
        .context("Failed to query for the passport's name")?;

    let passphrase = Password::with_theme(&style.dialoguer)
        .allow_empty_password(true)
        .with_prompt("Shared Key password")
        .with_confirmation(
            "Confirm the shared key password",
            "You need to be sure about your password",
        )
        .interact()
        .context("Failed to query for the shared key password")?;

    let pb = ProgressBar::new_spinner().with_style(style.spinner.clone());
    pb.set_message("generating new passport");
    pb.enable_steady_tick(style.spinner_interval);

    let mut key = [0; 32];
    cryptoxide::blake2b::Blake2b::blake2b(&mut key, passphrase.as_bytes(), &[]);

    let seed = Seed::derive_from_key(&key, &[]);
    state
        .create_passport(seed, buddy_name)
        .context("Failed to create the device's new passport")?;
    state.save().context("Failed to save state")?;
    pb.finish_and_clear();

    let id = state.id().unwrap();

    eprintln!(
        "{} {} {} {}",
        &style.dialoguer.success_prefix,
        &style
            .dialoguer
            .prompt_style
            .apply_to("New passport created"),
        &style.dialoguer.success_suffix,
        style.passport.apply_to(id),
    );

    Ok(())
}

fn info(style: &Style, state: &mut State) -> Result<()> {
    eprintln!(
        "passport id: {}",
        style.passport.apply_to(state.id().unwrap())
    );
    eprintln!("device alias: {}", style.alias.apply_to(state.alias()));
    eprintln!(
        "device key:   {}",
        style.public_key.apply_to(state.public_key().unwrap())
    );

    for (buddy, id) in state.buddies().search(&[])? {
        if state.passports().get(&id)?.is_some() {
            eprintln!(
                " {}{buddy}: {id}",
                style.dialoguer.success_prefix,
                buddy = style.alias.apply_to(buddy),
                id = style.passport.apply_to(id),
            );
        } else {
            eprintln!(
                " {}{buddy}: {id}",
                style.dialoguer.error_prefix,
                buddy = style.alias.apply_to(buddy),
                id = style.passport.apply_to(id),
            );
        }
    }

    Ok(())
}

async fn open_message(style: &Style, state: &mut State) -> Result<()> {
    let buddies = state.buddies().search(&[])?;
    let items: Vec<_> = buddies
        .iter()
        .filter_map(|(buddy, hash)| {
            if hash == &state.id().unwrap() {
                None
            } else {
                Some(buddy)
            }
        })
        .collect();
    ensure!(
        !items.is_empty(),
        "Sorry... it seems there is no passports available"
    );

    let index = Select::with_theme(&style.dialoguer)
        .with_prompt("Select recipient to read messages from")
        .items(items.as_slice())
        .interact()
        .context("Failed to query the buddy to send message to")?;
    let buddy = items[index];
    let buddy_id = *buddies.get(buddy).unwrap();
    let buddy_passport = state
        .passports()
        .get(&buddy_id)?
        .ok_or_else(|| anyhow!("No passport for {} ({})", buddy, buddy_id))?;

    if let Some((_, their_key)) = buddy_passport.shared_key() {
        let (_, our_key) = state.passport().unwrap().shared_key().unwrap();
        let topic = mk_topic(our_key, their_key);

        let messages = asmtp_storage::Message::open(state.db(), topic)?;
        let messages: BTreeMap<_, _> = messages.range_time(..).collect();
        let ids = messages.keys().collect::<Vec<_>>();

        let id = Select::with_theme(&style.dialoguer)
            .with_prompt("Select message to open")
            .items(&ids)
            .interact()
            .context("Failed to query message to open")?;
        let id = ids[id];
        let message = messages.get(id).unwrap();

        let output;

        {
            let master_key = state.key().unwrap();
            // retrieve the master shared key password
            let passphrase = Password::with_theme(&style.dialoguer)
                .with_prompt("Enter the shared key master password")
                .allow_empty_password(true)
                .interact()
                .context("Failed to query the master key password")?;

            let mut key = [0; 32];
            cryptoxide::blake2b::Blake2b::blake2b(&mut key, passphrase.as_bytes(), &[]);

            let seed = Seed::derive_from_key(&key, &[]);
            let encryption_key = state
                .passport()
                .unwrap()
                .unshield_shared_key(our_key, master_key, seed)?;

            let (r_key, m) = keynesis::noise::X::<_, Blake2b, _>::new(OsRng, &[])
                .receive(&encryption_key, &message)
                .context("Failed to encrypt message")?;
            if &r_key != their_key {
                let _ = Confirm::with_theme(&style.dialoguer)
                    .with_prompt("The message is not from this peer")
                    .interact()?;
            }
            output = m;

            // make sure we clear the scope of the encryption key as soon as possible
            std::mem::drop(encryption_key);
        }

        let message = String::from_utf8_lossy(&output);

        eprintln!(
            "{}",
            style.topic.apply_to("Start Decrypted message>>>>>>>>")
        );
        println!("{}", message);
        eprintln!(
            "{}",
            style.topic.apply_to("<<<<<<<<<<End Decrypted message")
        );
    }

    Ok(())
}

async fn send_messages(
    style: &Style,
    state: &mut State,
    connection: &mut Connection,
) -> Result<()> {
    let buddies = state.buddies().search(&[])?;
    let items: Vec<_> = buddies
        .iter()
        .filter_map(|(buddy, hash)| {
            if hash == &state.id().unwrap() {
                None
            } else {
                Some(buddy)
            }
        })
        .collect();
    ensure!(
        !items.is_empty(),
        "Sorry... it seems there is no passports available"
    );

    let index = Select::with_theme(&style.dialoguer)
        .with_prompt("Select recipient to send message to")
        .items(items.as_slice())
        .interact()
        .context("Failed to query the buddy to send message to")?;
    let buddy = items[index];
    let buddy_id = *buddies.get(buddy).unwrap();
    let buddy_passport = state
        .passports()
        .get(&buddy_id)?
        .ok_or_else(|| anyhow!("No passport for {} ({})", buddy, buddy_id))?;

    if let Some((_, their_key)) = buddy_passport.shared_key() {
        let (_, our_key) = state.passport().unwrap().shared_key().unwrap();
        let master_key = state.key().unwrap();
        let topic = mk_topic(our_key, their_key);

        // open editor
        let message = Editor::new()
            .require_save(true)
            .edit(&format!("Message to {} ({})", buddy, buddy_id))
            .context("Failed to collect message")?
            .ok_or_else(|| anyhow!("No message entered"))?;

        // now we can encrypt with the master key
        let mut output = Vec::new();

        {
            // retrieve the master shared key password
            let passphrase = Password::with_theme(&style.dialoguer)
                .with_prompt("Enter the shared key master password")
                .allow_empty_password(true)
                .interact()
                .context("Failed to query the master key password")?;

            let mut key = [0; 32];
            cryptoxide::blake2b::Blake2b::blake2b(&mut key, passphrase.as_bytes(), &[]);

            let seed = Seed::derive_from_key(&key, &[]);
            let encryption_key = state
                .passport()
                .unwrap()
                .unshield_shared_key(our_key, master_key, seed)?;

            let () = keynesis::noise::X::<_, Blake2b, _>::new(OsRng, &[])
                .send(&encryption_key, their_key, &message, &mut output)
                .context("Failed to encrypt message")?;

            // make sure we clear the scope of the encryption key as soon as possible
            std::mem::drop(encryption_key);
        }

        connection
            .send(Message::new_topic(topic, output))
            .await
            .context("Failed to send the encrypted message to peer")?;
    } else {
        bail!(
            "{} ({}) does not have a shared key to send messages to",
            buddy,
            buddy_id
        )
    }
    Ok(())
}

async fn connect(style: &Style, state: &mut State) -> Result<Connection> {
    let remote_address = state.settings().remote_address();
    let remote_id = state.settings().remote_id();

    eprintln!(
        "Connecting to {remote_id} ({remote_address})",
        remote_address = remote_address,
        remote_id = style.public_key.apply_to(remote_id),
    );

    let connection = Connection::connect_to(OsRng, state.key().unwrap(), remote_address, remote_id)
        .await
        .context("Failed to connect to ASMTP server")?;

    Ok(connection)
}

async fn submit_passport(
    style: &Style,
    state: &mut State,
    connection: &mut Connection,
) -> Result<()> {
    let remote_address = state.settings().remote_address();
    let remote_id = state.settings().remote_id();
    use futures::prelude::*;

    let id = state.id().unwrap();
    let blocks = state.blocks();

    let count = blocks.iter().count();

    eprintln!(
        "Submitting {count} blocks of passport {id} to {remote_id} ({remote_address})",
        remote_address = remote_address,
        remote_id = style.public_key.apply_to(remote_id),
        count = count,
        id = style.passport.apply_to(id),
    );

    connection
        .send(Message::new_put_passport(id, blocks.as_slice()))
        .await
        .unwrap();

    eprintln!(
        "passport {id} submitted to {remote_id} ({remote_address}) successfully",
        remote_address = remote_address,
        remote_id = style.public_key.apply_to(remote_id),
        id = style.passport.apply_to(id),
    );

    Ok(())
}

async fn submit_subscriptions(
    style: &Style,
    state: &mut State,
    connection: &mut Connection,
) -> Result<()> {
    let remote_address = state.settings().remote_address();
    let remote_id = state.settings().remote_id();
    use futures::prelude::*;

    let mut topics: Vec<Topic> = state
        .passports()
        .all_passports()?
        .into_iter()
        .map(|h| {
            let mut bytes = [0; Topic::SIZE];
            let topic = &mut bytes[(Topic::SIZE - Hash::SIZE)..];
            topic.copy_from_slice(h.as_ref());
            Topic::new(bytes)
        })
        .collect();
    topics.extend(state.messages().range(..));

    let count = topics.len();

    eprintln!(
        "Submitting {count} topics to {remote_id} ({remote_address})",
        remote_address = remote_address,
        remote_id = style.public_key.apply_to(remote_id),
        count = count,
    );

    for topic in topics {
        connection
            .send(Message::new_register_topic(topic))
            .await
            .unwrap();
    }

    eprintln!(
        "{count} topics submitted to {remote_id} ({remote_address}) successfully",
        remote_address = remote_address,
        remote_id = style.public_key.apply_to(remote_id),
        count = count,
    );

    Ok(())
}

async fn sync_new_messages(
    style: &Style,
    state: &mut State,
    connection: &mut Connection,
) -> Result<()> {
    for topic in state.messages().range(..) {
        sync_new_messages_for(style, state, connection, topic)
            .await
            .with_context(|| format!("Failed to load messages for {topic}", topic = topic))?;
    }

    let pb = ProgressBar::new_spinner().with_style(style.spinner.clone());
    pb.set_message("waiting for messages...");
    pb.enable_steady_tick(style.spinner_interval);

    let mut counter: usize = 0;

    while let Ok(Some((_peer, message))) =
        timeout(std::time::Duration::from_secs(2), connection.next()).await
    {
        let message = message?;
        counter += 1;

        if let Some((topic, m)) = message.topic_checked() {
            pb.println(format!(
                "incoming message for {topic}",
                topic = style.topic.apply_to(topic),
            ));

            let message = asmtp_storage::Message::open(state.db(), topic)?;
            message.insert(m)?;
        }
    }
    pb.finish_and_clear();

    if counter == 0 {
        eprintln!(
            "{prefix} You have no new messages",
            prefix = style.dialoguer.success_prefix,
        );
    } else {
        let plural = if counter > 1 { "s" } else { "" };
        eprintln!(
            "{prefix} You have {count} new message{plural}",
            prefix = style.dialoguer.success_prefix,
            count = counter,
            plural = plural,
        );
    }

    Ok(())
}

async fn sync_new_messages_for(
    style: &Style,
    state: &mut State,
    connection: &mut Connection,
    topic: Topic,
) -> Result<()> {
    eprintln!(
        "querying new messages on {topic}",
        topic = style.topic.apply_to(topic)
    );

    let message = asmtp_storage::Message::open(state.db(), topic)?;
    let time = message
        .last_message_id()?
        .map(|m_id| m_id.time())
        .unwrap_or_else(|| keynesis::passport::block::Time::from(0));
    connection
        .send(Message::new_query_topic_messages(topic, time))
        .await?;

    Ok(())
}
