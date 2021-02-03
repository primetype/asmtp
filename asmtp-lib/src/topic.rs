use cryptoxide::{hmac::Hmac, pbkdf2::pbkdf2, sha2::Sha512};
use keynesis::key::curve25519::PublicKey;
use poldercast::Topic;

const ITERATIONS: u32 = 10 * 1_024;

/// create a new `Topic` between the 2 public keys
///
/// This function is rather slow as the `ThreadId` is derived from
/// a PBKDF2 HMAC SHA512 with ten of thousands of iterations.
/// So once the ThreadId has been generated, it is recommended to
/// keep it stored somewhere for reuse rather than re-creating it.
///
/// This is a scrubbed composition of the public keys of the 2 participants
/// in a topic exchange
///
/// The `Topic` is derived from the public keys of both participants.
/// We use `PBKDF2 HMAC SHA512 10240` to derive the `Topic`
/// from the keys.
///
/// This way we have a deterministic way to generate the `Topic` for the recipient
/// and the sender without the need to negotiate it.
///
/// This also prevents malicious sender to contact users without the recipient actively
/// accepting to receive messages from the sender since the recipient will need to
/// subscribe to receiving messages on that given `Topic`.
///
/// The other advantage of this is that it is hard to brute force the message's metadata.
/// I.e. it is virtually impossible to establish who are the sender and the recipient
/// of the message by just looking at the `Message`'s content and `Topic`.
pub fn mk_topic(key1: &PublicKey, key2: &PublicKey) -> Topic {
    let (key, salt) = if key1 < key2 {
        (key1, key2)
    } else {
        (key2, key1)
    };

    let mut mac = Hmac::new(Sha512::new(), key.as_ref());
    let mut bytes = [0; Topic::SIZE];

    pbkdf2(&mut mac, salt.as_ref(), ITERATIONS, &mut bytes);

    Topic::new(bytes)
}
