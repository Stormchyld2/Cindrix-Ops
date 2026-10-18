# 3. Safety and licenses

## Medical safety
- The assistant never gives insulin or other medication doses, or changes to them. A check runs **before** the model, in code. Dosing questions get a fixed reply: use your doctor's written plan, and here is the Low blood sugar card.
- Glucagon appears only as "as directed on the package", plus call 911.
- Emergency cards are one-screen and need no model.
- All Diabetes Pack content is marked **draft** until a clinician has reviewed it. After review, the card shows the reviewer and review date. Nothing is sold as reviewed before that.
- The inventory counts medicine but never suggests amounts.
- Footer on medical screens: reference only, not medical advice, call 911 in an emergency.

## Honest interface
- Never claim to turn off radios. State what the app actually does.
- Position on desktop is set by hand and labeled as approximate. Live GPS appears only where hardware provides it (phones, optional USB receiver).
- "What fails next" is labeled as an estimate.
- Satellite photos show the survey year.

## Content rules
Only free and open content ships.

| Source | License | Notes |
|---|---|---|
| Wikipedia, WikiMed, WikEM | CC BY-SA 4.0 | Credit and link, share-alike |
| US Army, FEMA, CDC, FDA documents | Public domain (US government) | Record edition and date |
| Project Gutenberg titles | Public domain in the US | Check each title |
| OpenStreetMap | ODbL | "© OpenStreetMap contributors" shown on every map |
| USGS NAIP | Public domain | Optional, downloaded by the user |
| Qwen3.5 | Apache 2.0 | |
| llama.cpp | MIT | |
| Kiwix | GPL | See below |
| Our own and the author's SERE material | Ours | Reviewed before shipping |

Excluded: DeepL, Meta Seamless, iFixit (non-commercial license), any archive with an unclear license, and any copyrighted book.

## What GPL means for selling
Shipping Kiwix components makes the app a GPL work. So the app is **open source** (this repository), with source links in the app. What is sold is the curated, tested and loaded drive, the user guide, updates, and support. This is a working assumption, not legal advice. **OPEN:** short review by a lawyer before the first sale, which can also cover the name.

## Name
"Cindrix" resembles "Citrix" (software company, many registered marks). A clearance check by a trademark attorney is recommended before selling. Fallback names discussed: Cindrel, Cindar, Cinderon.
