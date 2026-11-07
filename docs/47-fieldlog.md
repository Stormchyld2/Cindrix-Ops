# 47. FieldLog: recording where you went and what you saw (2026-11-05)

A design for a field notebook tied to the map viewer.

## Ideas to borrow
- **Trackbook (MIT, y20k/trackbook):** a GPX recorder. Its useful idea is an **accuracy threshold**: ignore points whose reported accuracy is worse than a set limit, so a track does not zigzag when the signal is poor. Code may be reused with the MIT notice kept.
- **JPSTrack (BSD):** records a track and attaches **photo, voice and text notes** to points on it.
- **Barometer logging:** if a phone has a pressure sensor, log pressure with each point. Falling pressure is a weather warning, and pressure also gives a better altitude than GPS alone.

## Proposed FieldLog
- A **track** is a list of points (time, lat, lon, accuracy, optional pressure) saved as plain **GPX**, so any other program can read it.
- A **note** is attached to a point or a track: text, photo or voice clip. It is saved beside the GPX as a small file.
- **Offline first:** everything stays on the device; export is a folder copy.
- Shown on the existing map viewer as a line with pins.
- Weather: a simple "pressure fell by X over Y hours" hint, with the usual caution that it is a hint, not a forecast.

## Not decided
Battery cost of logging, how often to sample, and whether voice notes need transcription (the app already has a local model).
