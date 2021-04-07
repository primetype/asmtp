Since the Coronavirus Pandemic that started end of 2019 the world has accelerated
even more the use of decentralized workforce. Students have been confined to study
from home. Yet we managed the stay connected. Connected with our colleagues, with
other students and teachers, with our communities. This is an amazing opportunity
for technology innovation.

However this is also an opportunity for organized crime or for authoritarian
governments to tighten their grip on their victims. Entire databases with
millions of users have been leaked[\\(^{1}\\)]. This is worrisome as these
leaks contains information linking to our identities: email addresses, phone
numbers, address of residence, education, etc. From these information it is
possible to social engineer identity theft.

The only way is to improve the tools we have, to start respecting ourselves and
care more for our online identity, our privacy.

# The cloak of dignity

We are not the only one to have seen how it is crucial to address the problems
we are facing. Signal messaging application offers one of the most secure and
privacy preserving messaging protocol you can get. And for free! Or is it, really?

It seems that the goal of `Signal` has shifted from providing secure messaging
services to cryptocurrency[\\(^{2}\\)]. While this may seem okay for some it
has hurt some technologists around the world[\\(^{3}\\)]. Signal has also so
many flows, one of them is to rely on mobile operators to play nice as it still
relies heavily on the phone number of its users to authenticate them.

It comes down to the same problem as usual: how to authenticate users? Signal
and WhatsApp are relying on phone numbers, Facebook and many others are relying
on email addresses and passwords. We all know this is flawed in so many way.
Database are leaked. We are making ourselves dependent on Apple, Facebook, Google
or Microsoft to be the guardians of our digital identity. What if Apple decides
you should not use a specific service anymore? How ethical is it for them to
decide what their users should use or not.

# Fear not for there is hope

We have designed a new set of protocols to address these problems.

## [`Passport`]: Digital identity

[`keynesis`] is protocol to offer digital identity. It is leveraging modern
cryptography and the secure enclave we find on most devices to offer the
most secure way to manage your digital identity.

[`keynesis`] defines all the necessary cryptographic protocols to maintain
what is going to identify you as an individual: [`Passport`].

Unlike Signal or Facebook, the authentication of users does not rely on
a third party. If you can access the device you have in your hands you can
authenticate yourself. Also the [`Passport`] is completely anonymous. It does
not contains any information that may be used to identify an individual.
The only way to know a password is the password of Nicolas is if
Nicolas told you it is his and that he provided you the proof he control
keys registered in that [`Passport`].

## [`ASMTP`]: Secret and private messaging

[`ASMTP`] stands for Anonymous and Secure Message Transfer Protocol. It is
one fo the many transport layer that can be used to leverage the use of
[`keynesis`]'s [`Passport`]. It defines a message service that can be used
to propagate [`Passport`]'s updates and messages between [`Passport`]s.

One of the interesting aspect of [`ASMTP`] is that it offers anonymous message
passing between nodes of the network. Messages are passed nodes to nodes and
are propagated through the network following [`poldercast`]: the peer to peer
(P2P) pub/sub algorithm.

[\\(^{1}\\)]: https://www.theguardian.com/technology/2021/apr/03/500-million-facebook-users-website-hackers
[\\(^{2}\\)]: https://signal.org/blog/help-us-test-payments-in-signal/
[\\(^{3}\\)]: https://www.stephendiehl.com/blog/signal.html
[`keynesis`]: ./keynesis.md
[`Passport`]: ./keynesis.md#passport
[noise protocol]: ./noise.md
[`ASMTP`]: ./asmtp.md
[`poldercast`]: ./poldercast.md