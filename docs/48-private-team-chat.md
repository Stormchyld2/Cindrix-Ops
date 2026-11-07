# 48. Our own encrypted "Discord": how to build it without inventing crypto (2026-11-05)

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

---

# Verdict (2026-11-05): Cindrix Comms on Matrix, serverless, and a custom client

Scope: text chat only. Group voice is out for now.

## 1. Matrix as the base: viable. Do this.
- Continuwuity (Rust Matrix server) plus Element / Element X is the base already chosen. **Check Continuwuity's licence in its own repo before shipping it** (I could not confirm it from its website; it descends from Conduit, which is permissive, but verify).
- Federation can stay off; the group is invite-only. Messages and E2EE keys live on the members' devices, so the server mostly sees metadata.

## 2. The "serverless" idea: not as one thing. Split it.
**A true no-server Matrix does not exist in usable form** (the peer-to-peer Matrix experiments were never finished). Matrix rooms need one authoritative server per room to stay consistent.

What *is* viable, in order of safety:
1. **Failover, not serverless (recommended).** One Comms server name. The PC is primary. The Note10 Hub keeps a regularly copied snapshot of the database. If the internet or PC is down, the Note10 starts as that **same** server from the latest copy; members join its Wi-Fi hotspot and carry on. When the PC returns, copy the Note10's database **back** to it (one direction, one writer at a time) before the PC serves again. Never run both at once. Messages sent during the outage exist only on the Note10 until that copy-back, so back it up before handing over.
2. **True serverless fallback: use Briar, do not build one.** Briar is an audited, off-grid, end-to-end encrypted messenger that syncs over Bluetooth, Wi-Fi and Tor and keeps data only on members' phones. It already does what "serverless" means. It is GPL-3.0 (Android) and AGPL (desktop), so we **run it beside Comms as a separate app** and may not copy its code into ours. Android and desktop only, no iPhone.
3. **Our own serverless engine: not viable for now.** Discovery, offline delivery, group keys (MLS), history sync and key backup would all be ours to write and test. That is a multi-year, security-critical project for a four-person group, and the first rule is that we do not invent crypto.

## 3. Your "both the phone and the server hold it, only one delivers" idea
Matrix clients already queue unsent messages and the server stores sent ones, with unique event IDs, so a message is delivered once and duplicates are discarded. Test it rather than build it. The extra Cindrix piece is a **status line** ("on PC server / on Note10 / offline, queued").

## 4. A custom Cindrix chat client: viable, but not first
- Viable: matrix-rust-sdk (Apache-2.0, to be re-verified) does the hard parts: sync, rooms, E2EE (via vodozemac), the local send queue. Element X is itself built on it. A Tauri/Svelte client on it is realistic for desktop; Android is harder, because it means Rust through Tauri mobile or a separate Kotlin shell.
- Cost: months, mostly UI: verification flows, key backup/recovery, notifications on Android (the awkward part with no Google push), media, read receipts.
- Recommendation: **ship with Element / Element X first.** Put the Cindrix features around it: alert room, a download-links room with the Hub's address, a join-by-QR page served from the Note10, a bot for alerts. Build our own client only for the features Element cannot give us.

## 5. Order of work
1. Run Continuwuity on the PC; make an invite-only space with Alerts, General, Manuals and Downloads rooms.
2. Put the same server on the Note10; script the snapshot copy and the one-way hand-back; test on the hotspot.
3. Join-by-QR page on the Note10.
4. Add Briar to the Hub's download list as the no-infrastructure fallback.
5. Decide on a custom client after a month of real use.
