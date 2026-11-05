# 45. Our own encrypted "Discord": how to build it without inventing crypto (2026-11-05)

**Short answer: yes, it can be done, and the safe way is to assemble vetted parts, not to write your own encryption.**

## Three levels, from least to most work
1. **Run Matrix ourselves (days).** A Matrix server on our own box (a Conduit-type, Rust, lightweight homeserver) plus the Element app. End-to-end encrypted rooms (Olm and Megolm), invite-only, no federation if we turn it off. This is the "encrypted Discord" already, with our name on the server.
2. **A Cindrix Chat client (months).** Our own Svelte or Tauri app built on the **matrix-rust-sdk** (Rust; includes the encryption library *vodozemac*). Same encrypted rooms, but looking and behaving like the other Cindrix apps, with channels, roles and invite links. We write the screens; the SDK does the cryptography.
3. **Fully serverless inside the Hub or a mesh (a long project).** Peers talk directly using *libp2p* or the *Noise* handshake, with group keys through *MLS* (the IETF group-messaging standard; the Rust implementation is *OpenMLS*). Good for no-internet situations, but you must solve discovery, offline delivery and history yourself.

## Voice and video
- Text is the easy part. **Group voice is hard to encrypt end to end.** The usual route is a self-hosted *LiveKit* server (an SFU) with its end-to-end encryption mode; test it carefully before trusting it.
- A cheap fallback for small groups: peer-to-peer calls (as Meshenger does) with no server in the middle.

## Rules we keep
- **Never write our own cipher, handshake or key exchange.** Use libraries that others have audited.
- **Metadata still leaks:** who talks to whom, and when. A server admin sees it. Hosting the server ourselves, behind Tor if we wish, narrows who can see it.
- **Verify keys in person.** Each person's device fingerprint should be compared by QR or by reading it aloud.
- **Plan for lost devices:** key backup is the hardest usability problem in encrypted chat.
- Check every dependency's current licence before adopting it (the ones named here were permissive or copyleft as of our last check, which must be re-verified at adoption time). The server software matters most: some Matrix servers are AGPL, which would affect how we ship it.

## Suggested path
Start with level 1 for the Nightwolves now, learn what we actually miss, then decide whether level 2 is worth building.
