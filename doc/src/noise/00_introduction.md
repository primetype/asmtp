# Noise

The [Noise Protocol] is a framework that we use for our cryptographic
protocols. It defines how to establish encrypted sessions between
senders and recipients.

> Noise protocols support mutual and optional authentication, identity hiding,
> forward secrecy, zero round-trip encryption, and other advanced features.

* _mutual authentication_: it means all the participants are proving to
  their identity to each others;
* _identity hiding_: means that no outside observer can find out the
  identity if the participants;
* _forward secrecy_: it means that even if the long term keys of the participants
  are compromised the session keys are protected as well as protecting the past
  keys if the recent keys are compromised. In other words: we are protecting
  the past communications.
* _zero round-trip encryption_: it is possible to send encrypted application
  data to a recipient without prior exchanged messages.

How the encrypted sessions are established is called Handshake Patterns. These
are the first sequence of messages the participants are to exchanged in order
to establish the encrypted session.

We are leveraging different of these handshake pattens in our protocols.

## Pattern: `Npsk0`

More details on the [`Noise's explorer Npsk0`](https://noiseexplorer.com/patterns/Npsk0/)

* [x] Mutual authentication
* [x] Zero round trip encryption
* [/] Identity hiding
* [ ] Forward secrecy

We are using this pattern in the passport to encrypt the shared secret key
between all the _registered keys_ of the passport. All it uses
is the `pre-shared key` (psk) with the public key of the recipient.

Using this pattern allows us to have a fairly small encrypted data
in the passport that requires only the extra ephemeral public key
to be stored along with the encrypted data.

[Noise Protocol]: https://noiseprotocol.org

## Pattern: `IK`

More details on the [`Noise's explorer IK`](https://noiseexplorer.com/patterns/IK/)

* [x] Mutual authentication
* [ ] Zero round trip encryption
* [x] Identity hiding
* [x] Forward secrecy

In only one round trip we can establish the most secure form of encrypted
session available to us. The only prerequisite to establish it is to know
in advanced the public key of the remote peer.

ASMTP uses this scheme to establish the peer to peer encryption between
nodes in the network. The public keys are expecting to be the public keys
of the nodes that are also present in the node's gossips (more info
about [poldercast's gossips here](../poldercast/00_introduction.md)).

## Pattern: `X`

More details on the [`Noise's explorer X`](https://noiseexplorer.com/patterns/X/)

* [/] Mutual authentication
* [x] Zero round trip encryption
* [x] Identity hiding
* [ ] Forward secrecy

This pattern allows for 0 round trip encryption. Yet it allows to authenticate
both the sender to the receiver (the sender's public key is transmitted in the
message -- encrypted).

This pattern is used to send the encrypted messages between 2 passports with the
long term keys being the passport's shared keys. Because we are generating a new
ephemeral keys for each messages we have guarantees that the messages can only
be decrypted with the recipient's private key.
