# Anonymous Secure Mail Transfer Protocol (ASMTP)

ASMTP put together pub/sub peer to peer protocol [`poldercast`], blockchain technology
from [`keynesis`] and encryption protocol [`noise`] to build a secure and anonymous
network protocol as an alternative to [SMTP] (Simple Mail Transfer Protocol).

ASMTP was written as a tool to initially securely exchange messages with friends and
colleagues. Tools that provides End to End encryptions often keep hidden from their
users that they are still collecting metadata (such as who talks to whom and what
time). `Signal` is an interesting messaging tool. However they still requires you
to have a phone number to register on their platform. This is not something I am happy
with too. If the telephone company decides to block `Signal` from sending you
the authentication code you are screwed. They may even intercept that authentication
code to register in our stead. Scary.

ASMTP does not relies on any third party. Just like in the spirit of the old [SMTP],
messages are relayed from the mail server of the sender to the mail server
of the recipient and it may use an intermediate route to do so. (yes, [SMTP]
used to be decentralized by design until big companies started to offer
_"free"_ email services). However, unlike [SMTP], the sender and the recipient
identifier (email address for [SMTP]) is not known. Also unlike [SMTP] it is
not possible to receive messages from senders that are not "allowed" to.

# Overall components of the protocol

## poldercast: relaying messages

[`poldercast`] is a pub/sub peer to peer topology builder. Each nodes subscribes to
a list of **Topic** and publish it to the network. Each messages sent for the given
topic will be relayed through the network of nodes who subscribed for this topic.

## keynesis: passport of identity

[`keynesis`] defines a `Passport`. It is in fact a _blockchain_ owned and control
by the users and publicly shared. The users update their `Passport` with new keys.

Every updates are shared across the poldercast network. It only contains public keys.
Only the other peers who subscribed to receive notification about the passport
will receives the updates.

## Topic: anonymous message metadata

One of the main issue in secure messaging is that it is possible to access the metadata
of the message (who sent a message to whom). This can be problematic as it breach
the anonymity of the users and can lead to catastrophic situations.

ASMTP provides a way around that.
Each passport may contain a `SharedKey`. It is a Curve25519 `PublicKey`. The message's
`Topic` is derived from the `PublicKey` of the recipient and of the sender in such a way
that it is not possible to reverse and that it is hard to brute force (i.e. it is hard
to generate all the different topics of all the public keys that are on the network).

The derivation uses `pbkdf2 HMAC SHA512` with **10240** iterations. The `key` is the
smallest of the public key and the salt is the other one.

## Encrypted messages

Messages are encrypted with the [`X`] [`noise`] protocol message. This way the message
is encrypted and authenticated so only the recipient can decrypt it and the recipient
is the only one who can authenticate accurately the sender of the message. And the sender
should match the other key used to derive the `Topic`. Otherwise this is garbage and the
message can quickly be ignored.

## Network

The ASMTP network protocol is rather simple:

**First**: performs a protocol handshake upon establishing new connections (1 byte of version and
a few bytes of [`IK`] Noise protocol handshake).

During that step, it is possible to authenticate the peer we are talking to.

**Then**: then that's it. You have a [`noise`] transport state now and it is used to
encrypt/decrypt all the messages that goes through the network. After each successfully
encrypted/decrypted message we perform the noise transport's `rekey` so we have forward
secrecy on our side too.


# Disclaimer

ASMTP is a work in progress. It is a tool that is originally written to help me
send messages with my friends and colleagues. Please hear the following:

> It has not been audited and should be used at your own risk.

## License

This project is licensed under the [MIT] **OR** [Apache-2.0] dual license.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in ASMTP by you, shall be licensed as `MIT OR Apache-2.0` dual
license, without any additional terms or conditions.

[MIT]: https://github.com/primetype/asmtp/blob/master/LICENSE-MIT
[Apache-2.0]: https://github.com/primetype/asmtp/blob/master/LICENSE-APACHE
[`keynesis`]: https://github.com/primetype/keynesis
[`poldercast`]: https://github.com/primetype/poldercast
[`X`]: https://noiseexplorer.com/patterns/X/
[`IK`]: https://noiseexplorer.com/patterns/IK/
[SMTP]: https://tools.ietf.org/html/rfc5321
[`noise`]: https://noiseprotocol.org/
[`keynesis`]: https://github.com/primetype/keynesis
[`poldercast`]: https://github.com/primetype/poldercast