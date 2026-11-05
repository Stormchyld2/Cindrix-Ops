# 43. Comms: lessons from WiFi Direct Cable and Meshenger (2026-11-05)

Design reference only for the Hub, AirLink and Deploy work. Written from the projects' public READMEs and protocol notes.

## WiFi Direct Cable (MIT, jingcjie/wdcable_flutter)
Android app plus a Windows companion that chat, send files and carry voice over a Wi-Fi Direct link with **no router**.
- **Protocol v2 separates two jobs:** who is the Wi-Fi Direct *group owner or client* (a radio-layer role) from who is the *TCP listener or connector* (an application role). Do not let one decide the other, or reconnecting after a drop breaks.
- A small **UDP rendezvous** step lets each side announce itself before the TCP link opens.
- Voice is **RTP/RTCP over UDP with Opus**, separate from chat and files.
- Takeaway for AirLink: keep radio role, session role and media path as three separate layers with their own reconnect logic.
- Licence is MIT, so ideas **and code** may be borrowed if the copyright notice is kept. Read the repo before borrowing any code.

## Meshenger (GPL-3.0, design reference only)
Serverless voice calls between phones on one network; contacts are exchanged as **QR codes**; WebRTC underneath.
- Known weak spots, straight from its issue list: calls through a phone hotspot are flaky, no NAT or firewall traversal, and contacts break when a phone's address changes.
- Takeaway for the Hub hotspot: do not trust IP addresses as identity. Identify a person by a **key pair** (shown as a QR code at pairing), and look the current address up by name each time.
- GPL means **no code**; ideas only.

## What this suggests building
1. **Pair by QR** at the Hub or in person: each device shows a key fingerprint; scanning adds the contact.
2. **Address by key, not IP**: the Hub announces "key X is at address Y" on the local network.
3. **Three layers** as above, each reconnecting on its own.
4. **Test the hotspot case first**, because that is the Hub's default setup.
