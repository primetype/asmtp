use std::str::FromStr;

use anyhow::{bail, Context as _, Result};
use keynesis::{
    key::ed25519::PublicKey,
    passport::{block::Hash, PassportBlocks, PassportBlocksSlice},
};
use poldercast::Topic;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

#[derive(sqlx::FromRow)]
pub struct Contact {
    pub id: i64,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub updated_at: chrono::DateTime<chrono::Local>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Passport {
    pub id: Vec<u8>,
    #[sqlx(default)]
    pub alias: Option<String>,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub updated_at: chrono::DateTime<chrono::Local>,
    pub blocks: Vec<u8>,
}

#[derive(sqlx::FromRow, Debug)]
pub struct Key {
    pub key: Vec<u8>,
    #[sqlx(default)]
    pub alias: Option<String>,
    pub created_at: chrono::DateTime<chrono::Local>,
    pub updated_at: chrono::DateTime<chrono::Local>,
}

#[derive(sqlx::FromRow)]
pub struct Message {
    pub id: i64,
    pub thread: Vec<u8>,
    pub content: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Local>,
    #[sqlx(default)]
    pub read_at: Option<chrono::DateTime<chrono::Local>>,
}

#[derive(sqlx::FromRow)]
pub struct Thread {
    pub topic: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Local>,
}

pub enum StorageOptions {
    Sqlite { uri: String },
}

#[derive(Clone)]
pub struct Storage {
    backend: SqlitePool,
}

impl Storage {
    pub async fn new(config: StorageOptions) -> Result<Self> {
        match config {
            StorageOptions::Sqlite { uri } => {
                let options = SqliteConnectOptions::from_str(uri.as_str())
                    .with_context(|| format!("Failed to parse SQLite options from URI: {}", uri))?
                    .foreign_keys(true)
                    .create_if_missing(true);

                let backend = SqlitePool::connect_with(options).await.with_context(|| {
                    format!("Failed to open SQLite storage with URI: \"{}\"", uri)
                })?;

                // only run the migration script on testing for now
                sqlx::migrate!()
                    .run(&backend)
                    .await
                    .context("Failed to run migration script")?;

                Ok(Self { backend })
            }
        }
    }

    /// list all the contacts present in the database in alphabetical order
    pub async fn contacts(&self) -> Result<Vec<Contact>> {
        sqlx::query_as::<_, Contact>(
            r#"
                SELECT id, name, created_at, updated_at
                FROM contact
                ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all contacts")
    }

    /// create a new contact and returns the contact id
    pub async fn new_contact<N>(&self, name: N) -> Result<i64>
    where
        N: AsRef<str>,
    {
        sqlx::query(
            r#"
            INSERT INTO contact (name)
            VALUES ( ?1 )
            "#,
        )
        .bind(name.as_ref())
        .execute(&self.backend)
        .await
        .context("Cannot insert new contact")
        .map(|p| p.last_insert_rowid())
    }

    /// update the name of a contact
    pub async fn update_contact<N>(&self, id: i64, name: N) -> Result<()>
    where
        N: AsRef<str>,
    {
        sqlx::query(
            r#"
            UPDATE contact
            SET name       = ?1,
                updated_at = DATETIME('now')
            WHERE id = ?2
            "#,
        )
        .bind(name.as_ref())
        .bind(id)
        .execute(&self.backend)
        .await
        .context("Failed to update contact contact")
        .map(|_| ())
    }

    /// delete a contact from the database
    pub async fn delete_contact(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM contact
            WHERE contact.id = ?1
            "#,
        )
        .bind(id)
        .execute(&self.backend)
        .await
        .context("Failed to delete contact")
        .map(|_| ())
    }

    pub async fn passports(&self) -> Result<Vec<Passport>> {
        sqlx::query_as::<_, Passport>(
            r#"
                SELECT id, alias, created_at, updated_at, blocks
                FROM passport
                ORDER BY alias ASC NULLS LAST
            "#,
        )
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all passports")
    }

    pub async fn new_passport(&self, blocks: PassportBlocksSlice<'_>) -> Result<Hash> {
        let id = if let Some(block) = blocks.iter().next() {
            block.header().hash()
        } else {
            bail!("Needs at least one block in a passport")
        };

        sqlx::query(
            r#"
            INSERT INTO passport (id, blocks)
            VALUES ( ?1, ?2)
            "#,
        )
        .bind(id.as_ref())
        .bind(blocks.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to store the new passport")?;

        Ok(id)
    }

    pub async fn update_passport(&self, blocks: PassportBlocksSlice<'_>) -> Result<Hash> {
        let id = if let Some(block) = blocks.iter().next() {
            block.header().hash()
        } else {
            bail!("Needs at least one block in a passport")
        };

        sqlx::query(
            r#"
            UPDATE passport
            SET blocks = ?1,
                updated_at = DATETIME('now')
            WHERE id = ?2
            "#,
        )
        .bind(blocks.as_ref())
        .bind(id.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to store the new passport")?;

        Ok(id)
    }

    pub async fn get_passport(&self, passport: &Hash) -> Result<Option<PassportBlocks<Vec<u8>>>> {
        let blocks: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT blocks FROM passport WHERE id = ?1")
                .bind(passport.as_ref())
                .fetch_optional(&self.backend)
                .await
                .context("Failed to find passport in the DB")?;

        if let Some(blocks) = blocks {
            Ok(Some(PassportBlocks::try_from(blocks)?))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_passport(&self, passport: &Hash) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM passport
            WHERE passport.id = ?1
        "#,
        )
        .bind(passport.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to delete passport")
        .map(|_| ())
    }

    pub async fn passports_of_contact(&self, contact: i64) -> Result<Vec<Passport>> {
        sqlx::query_as::<_, Passport>(
            r#"
                SELECT passport.id, passport.alias, passport.created_at, passport.updated_at, passport.blocks
                FROM passport
                INNER JOIN contact_passport
                WHERE contact_passport.contact = ?1 AND contact_passport.passport = passport.id
                ORDER BY passport.alias ASC NULLS LAST
            "#,
        )
        .bind(contact)
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all passports for contact")
    }

    pub async fn link_passport_to_contact(&self, contact: i64, passport: &Hash) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO contact_passport (passport, contact)
            VALUES ( ?1, ?2)
            "#,
        )
        .bind(passport.as_ref())
        .bind(contact)
        .execute(&self.backend)
        .await
        .context("Failed to store the link between passport and contact")?;

        Ok(())
    }

    pub async fn verify_contact_passport_link(
        &self,
        contact: i64,
        passport: &Hash,
        verified: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO contact_passport (passport, contact, verified, verified_at)
            VALUES ( ?1, ?2, ?3, DATETIME('now'))
            "#,
        )
        .bind(passport.as_ref())
        .bind(contact)
        .bind(verified)
        .execute(&self.backend)
        .await
        .context("Failed to store the verified link between passport and contact")?;

        Ok(())
    }

    pub async fn keys(&self) -> Result<Vec<Key>> {
        sqlx::query_as(
            r#"
                SELECT key, alias, created_at, updated_at
                FROM public_key
                ORDER BY alias ASC NULLS LAST
            "#,
        )
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all keys")
    }

    pub async fn key(&self, key: &PublicKey) -> Result<Option<Key>> {
        sqlx::query_as(
            r#"
                SELECT key, alias, created_at, updated_at
                FROM public_key
                WHERE key = ?1
            "#,
        )
        .bind(key.as_ref())
        .fetch_optional(&self.backend)
        .await
        .context("Failed to list find key")
    }

    pub async fn new_key(&self, key: &PublicKey) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO public_key (key)
            VALUES ( ?1 )
            "#,
        )
        .bind(key.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to insert the key in the storage")
        .map(|_| ())
    }

    pub async fn new_key_alias<A>(&self, key: &PublicKey, alias: A) -> Result<()>
    where
        A: AsRef<str>,
    {
        sqlx::query(
            r#"
            UPDATE public_key
            SET alias = ?1
            WHERE key = ?2"#,
        )
        .bind(alias.as_ref())
        .bind(key.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to set alias to the key")
        .map(|_| ())
    }

    pub async fn passports_of_key(&self, key: &PublicKey) -> Result<Vec<Passport>> {
        sqlx::query_as::<_, Passport>(
            r#"
                SELECT passport.id, passport.alias, passport.created_at, passport.updated_at, passport.blocks
                FROM passport
                INNER JOIN passport_key
                WHERE passport_key.public_key = ?1 AND passport_key.passport = passport.id
                ORDER BY passport.alias ASC NULLS LAST
            "#,
        )
        .bind(key.as_ref())
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all passports for key")
    }

    pub async fn keys_of_passport(&self, passport: &Hash) -> Result<Vec<Key>> {
        sqlx::query_as(
            r#"
                SELECT public_key.key, public_key.alias, public_key.created_at, public_key.updated_at
                FROM public_key
                INNER JOIN passport_key
                WHERE passport_key.passport = ?1 AND passport_key.public_key = public_key.key
                ORDER BY public_key.alias ASC NULLS LAST
            "#,
        )
        .bind(passport.as_ref())
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all keys")
    }

    pub async fn link_key_to_passport(&self, key: &PublicKey, passport: &Hash) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO passport_key (public_key, passport)
            VALUES ( ?1, ?2 )
            "#,
        )
        .bind(key.as_ref())
        .bind(passport.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to link key and passport in the storage")
        .map(|_| ())
    }

    pub async fn contains_tread(&self, topic: &Topic) -> Result<bool> {
        let opt = sqlx::query(
            r#"
                SELECT topic
                FROM thread
                WHERE topic = ?1
            "#,
        )
        .bind(topic.as_ref())
        .fetch_optional(&self.backend)
        .await
        .context("Failed to list all threads")?;

        Ok(opt.is_some())
    }

    pub async fn threads(&self) -> Result<Vec<Thread>> {
        sqlx::query_as(
            r#"
                SELECT topic, created_at
                FROM thread
                ORDER BY created_at ASC NULLS LAST
            "#,
        )
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all threads")
    }

    pub async fn new_thread(&self, topic: &Topic) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO thread (topic)
            VALUES ( ?1 )
            "#,
        )
        .bind(topic.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to create new topic thread")
        .map(|_| ())
    }

    pub async fn delete_thread(&self, topic: &Topic) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM thread
            WHERE thread.topic = ?1
        "#,
        )
        .bind(topic.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to delete thread")
        .map(|_| ())
    }

    pub async fn threads_of_key(&self, key: &PublicKey) -> Result<Vec<Thread>> {
        sqlx::query_as(
            r#"
                SELECT thread.topic, thread.created_at
                FROM thread
                INNER JOIN thread_key
                WHERE thread_key.key = ?1 AND thread_key.thread = thread.id
                ORDER BY thread.created_at ASC NULLS LAST
            "#,
        )
        .bind(key.as_ref())
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all threads")
    }

    pub async fn messages(&self) -> Result<Vec<Message>> {
        sqlx::query_as(
            r#"
                SELECT thread, content, created_at, read_at
                FROM message
                ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all messages")
    }

    pub async fn new_message<M>(&self, thread: &Topic, message: M) -> Result<i64>
    where
        M: AsRef<[u8]>,
    {
        sqlx::query(
            r#"
            INSERT INTO message (thread, content)
            VALUES ( ?1, ?2 )
            "#,
        )
        .bind(thread.as_ref())
        .bind(message.as_ref())
        .execute(&self.backend)
        .await
        .context("Failed to store message")
        .map(|p| p.last_insert_rowid())
    }

    pub async fn mark_message_read(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE message
            SET read_at = DATETIME('now')
            WHERE id = ?1"#,
        )
        .bind(id)
        .execute(&self.backend)
        .await
        .context("Failed to set the read_at message time")
        .map(|_| ())
    }

    pub async fn mark_message_unread(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE message
            SET read_at = NULL
            WHERE id = ?1"#,
        )
        .bind(id)
        .execute(&self.backend)
        .await
        .context("Failed to unset the read_at message time")
        .map(|_| ())
    }

    pub async fn delete_message(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM message WHERE id = ?1")
            .bind(id)
            .execute(&self.backend)
            .await
            .context("Failed to delete message from storage")
            .map(|_| ())
    }

    pub async fn messages_of_thread(&self, id: &Topic) -> Result<Vec<Message>> {
        sqlx::query_as(
            r#"
                SELECT id, thread, content, created_at, read_at
                FROM message
                WHERE thread = ?1
                ORDER BY created_at ASC
            "#,
        )
        .bind(id.as_ref())
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all messages for topic")
    }

    pub async fn messages_of_thread_since(
        &self,
        id: &Topic,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>> {
        sqlx::query_as(
            r#"
                SELECT id, thread, content, created_at, read_at
                FROM message
                WHERE thread = ?1 AND created_at > ?2
                ORDER BY created_at ASC
            "#,
        )
        .bind(id.as_ref())
        .bind(since)
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all messages for topic")
    }

    pub async fn messages_of_key(&self, key: &PublicKey) -> Result<Vec<Message>> {
        sqlx::query_as(
            r#"
                SELECT message.id, message.thread, message.content, message.created_at, message.read_at
                FROM message
                INNER JOIN thread_key
                WHERE thread_key.thread = message.thread AND thread_key.key = ?1
                ORDER BY message.created_at ASC
            "#,
        )
        .bind(key.as_ref())
        .fetch_all(&self.backend)
        .await
        .context("Failed to list all messages for topic")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use keynesis::{key::ed25519::SecretKey, Seed};
    use rand_chacha::ChaChaRng;
    use std::collections::BTreeMap;

    #[derive(Clone)]
    struct Sim {
        id: i64,
        name: String,
        secret_key: SecretKey,
        passphrase: Seed,
        passport: keynesis::passport::Passport,
    }

    struct Sims {
        rng: ChaChaRng,
        sims: BTreeMap<String, Sim>,
    }

    impl Sim {
        pub fn blocks(&self) -> PassportBlocks<Vec<u8>> {
            let mut blocks = PassportBlocks::new();
            for block in self.passport.blocks() {
                blocks.push(block);
            }
            blocks
        }
    }

    impl Sims {
        pub fn new() -> Self {
            let rng = Seed::from([0; Seed::SIZE]).into_rand_chacha();
            let sims = BTreeMap::new();
            Self { rng, sims }
        }

        pub async fn commit_all(&mut self, storage: &Storage) -> Result<()> {
            for sim in self.sims.values_mut() {
                sim.id = storage.new_contact(&sim.name).await?;
                let hash = storage.new_passport(sim.blocks().as_slice()).await?;
                storage.link_passport_to_contact(sim.id, &hash).await?;
                for key in sim.passport.active_master_keys() {
                    storage.new_key(key.as_ref()).await?;
                    storage.link_key_to_passport(key.as_ref(), &hash).await?;
                }
                if let Some((_, key)) = sim.passport.shared_key() {
                    storage.new_key(key).await?;
                    storage.link_key_to_passport(key, &hash).await?;
                }
            }

            Ok(())
        }

        pub fn sim(&self, name: &str) -> Result<&Sim> {
            self.sims
                .get(name)
                .ok_or_else(|| anyhow!("No sim named {}", name))
        }

        pub fn populate_sim<I, T>(&mut self, names: I) -> Result<()>
        where
            I: IntoIterator<Item = T>,
            T: ToString,
        {
            for name in names {
                let key = name.to_string();
                let name = name.to_string();
                let secret_key = SecretKey::new(&mut self.rng);
                let passphrase = Seed::generate(&mut self.rng);
                let passport = keynesis::passport::Passport::create(
                    &mut self.rng,
                    &name,
                    &secret_key,
                    passphrase.clone(),
                )
                .with_context(|| format!("Failed to create passport for {}", name))?;

                let sim = Sim {
                    id: 0,
                    name,
                    secret_key,
                    passphrase,
                    passport,
                };
                self.sims.insert(key, sim);
            }
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn contacts() {
        let storage = Storage::new(StorageOptions::Sqlite {
            uri: ":memory:".to_owned(),
        })
        .await
        .expect("Create the storage");

        let list = storage.contacts().await.expect("Get contact list");
        assert!(list.is_empty());

        let alice_id = storage
            .new_contact("alice")
            .await
            .expect("New contact created");
        let bob_id = storage
            .new_contact("Bob")
            .await
            .expect("New contact created");

        storage
            .update_contact(alice_id, "Alice")
            .await
            .expect("Update contact");

        let list = storage.contacts().await.expect("Get contact list");
        assert!(list.len() == 2);
        assert!(list[0].name == "Alice");
        assert!(list[1].name == "Bob");

        storage.delete_contact(bob_id).await.expect("Delete bob");
        let list = storage.contacts().await.expect("Get contact list");
        assert!(list.len() == 1);
        assert!(list[0].name == "Alice");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn passport() {
        let mut sims = Sims::new();
        sims.populate_sim(&["Alice", "Bob"])
            .expect("Create the initial sims");

        let storage = Storage::new(StorageOptions::Sqlite {
            uri: ":memory:".to_owned(),
        })
        .await
        .expect("Create the storage");

        let passports = storage.passports().await.expect("Get list of passports");
        assert!(passports.is_empty());

        let alice_id = storage
            .new_passport(
                sims.sim("Alice")
                    .expect("alice's Sim profile")
                    .blocks()
                    .as_slice(),
            )
            .await
            .expect("Store passport to storage");
        assert_eq!(alice_id, sims.sim("Alice").unwrap().passport.id());
        let bob_id = storage
            .new_passport(
                sims.sim("Bob")
                    .expect("bob's Sim profile")
                    .blocks()
                    .as_slice(),
            )
            .await
            .expect("Store passport to storage");
        assert_eq!(bob_id, sims.sim("Bob").unwrap().passport.id());
        let passports = storage.passports().await.expect("Get list of passports");
        assert!(passports.len() == 2);
        assert!(passports[0].id.as_slice() == alice_id.as_ref());
        assert!(passports[1].id.as_slice() == bob_id.as_ref());

        storage
            .delete_passport(&bob_id)
            .await
            .expect("delete bob's passport");
        let passports = storage.passports().await.expect("Get list of passports");
        assert!(
            passports.len() == 1,
            "{:#?} does not contain {:?}",
            passports.iter().map(|i| &i.id).collect::<Vec<_>>(),
            bob_id.as_ref()
        );
        assert!(passports[0].id.as_slice() == alice_id.as_ref());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn passport_advanced() {
        let mut sims = Sims::new();
        sims.populate_sim(&["Alice", "Bob", "Colette", "Donald"])
            .expect("Create the initial sims");

        let storage = Storage::new(StorageOptions::Sqlite {
            uri: ":memory:".to_owned(),
        })
        .await
        .expect("Create the storage");

        sims.commit_all(&storage)
            .await
            .expect("Commit all the sims to the storage");

        let alice = sims.sim("Alice").expect("Alice Sim").clone();
        let alice_passports = storage.passports_of_contact(alice.id).await.unwrap();
        assert!(alice_passports.len() == 1);
        assert!(alice_passports[0].id.as_slice() == alice.passport.id().as_ref());
        let alice_keys = storage
            .keys_of_passport(&alice.passport.id())
            .await
            .expect("Failed to list Alice's keys");
        assert!(alice_keys.len() == 2);

        storage.delete_contact(alice.id).await.unwrap();
        let passports = storage.passports_of_contact(alice.id).await.unwrap();
        assert!(passports.is_empty());

        let bob = sims.sim("Bob").expect("Bob Sim").clone();
        storage.delete_passport(&bob.passport.id()).await.unwrap();
        let passports = storage.passports_of_contact(bob.id).await.unwrap();
        assert!(passports.is_empty());
    }
}
