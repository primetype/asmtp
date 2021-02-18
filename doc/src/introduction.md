# Anonymous Secure Mail Transfer Protocol

**We all deserve privacy. And not only about what we say but to whom
we say it**.

1. state of the cryptographic protocol. Each messages allow for
   mutual authentication, identity hiding, forward secrecy and
   zero round-trip encryption.
2. Mutual consent. You only receive messages from those you already
   vetted yourself. No more cold call.
3. Anonymity of the conversations. Nodes on the network cannot know
   who is talking to whom. The conversations as well as the metadata
   are cryptographically scrubbed

## Technical overview

The protocol is rather simple and is mainly based upon two existing
protocols: `poldercast` and `noise`.

### Poldercast

Poldercast is a Peer-to-Peer (P2P) topic-based Pub/Sub protocol. It allows to
build and disseminate messages across a decentralized network.

A ASMTP-server is a poldercast node. It subscribes to specific topics and
publishes its subscriptions to it. The topics identifies a conversation
thread between two users. Each nodes
disseminate the subscription to their respective topics to other nodes.
It is called: _gossiping_.

Based on the unique identifier of the nodes and the topic preferences
the node is capable of determining to whom to send or forward messages of a
specific topic.

**TODO: make whole chapter about poldercast**

### Noise

Noise is a protocol to establish secure and authenticated encrypted _"connections"_
between two peers. It has the advantage of being very secure, to have been
formally proven to be secured and peer reviewed. It is used by [`WireGuard`]
for example.

Using `noise` we guarantee that the nodes know who they are talking to based
on their respective list of nodes. Each nodes are signing their _gossips_
which contains the details of how to interact with them (IP Addresses and
Public Keys). When connecting to a peer, the node will first perform the `noise`
handshake. Establishing an authenticated encryption state.

**TODO: make whole chapter about noise**
