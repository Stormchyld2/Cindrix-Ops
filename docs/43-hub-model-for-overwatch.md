# 43. The Hub's model also serves Cindrix Overwatch

Status: planned. The Hub (Cindrix Ops on Android) is not built yet, so nothing here runs today.

## Why

Cindrix Overwatch on a phone cannot run an AI model. It has an optional box, Settings > AI helper, where a person types the
address of a model server. The Hub phone is where the model files will live (the 4 TB drive, `models/`), so the Hub should be
that server. Overwatch uses it only to name the place a story is about (for map pins). Its brief and its translation never use it.

## What the Hub has to do

- Run `llama-server` and answer the same chat requests the Windows program already sends (`POST /v1/chat/completions`).
- Listen on a **fixed port, 8089**, instead of the random free port `core/src/modelhost.rs` picks today. Overwatch's
  "Use the Cindrix Hub on this phone" button fills in `http://127.0.0.1:8089`.
- On the same phone only (127.0.0.1) by default. Other phones reaching it over the hotspot is a separate switch, off at every
  start like sharing is (doc 34), and it must need the guest PIN. A model server with no PIN must never be open to the network.
- Start the model when the first request comes and stop it when idle, because memory on the phone is the limit (a 2B or 4B model).

## Not part of this

- No change to the safety rules: the assistant still never gives medicine or insulin doses. Overwatch asks only "which place".
- Overwatch never needs the internet through the Hub, and the Hub never needs Overwatch.

## Checks when it is built

- Overwatch on the Hub phone, address `http://127.0.0.1:8089`: a pin appears with "(placed by AI)".
- With the model switched off the Hub answers nothing and Overwatch simply shows no AI pins.
