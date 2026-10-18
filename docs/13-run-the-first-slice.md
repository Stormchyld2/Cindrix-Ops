# 13. Run the first slice

What works and what has been tested, so nothing is guessed.

## Tested (in the workspace, on Linux)
`core/` is a Rust crate with **no outside dependencies**. `cargo test` in `core/` runs 35 tests, all passing (the first slice had 25; the Plan group added the user folder, calendar and what-fails-next):
- the medication safety check against all 37 cases in `tests/safety/dosing.json`
- the answer path: dosing question never reaches the model; unknown topic gets "not found"; a model answer containing a dose is replaced; a model failure gives a plain message
- inventory CSV (round trip, days left, status, delete selected)
- card loading (a card can't claim to be reviewed without a reviewer and date)
- library search over `content/library-sample`
- user folder: safe saves with backups, a damaged file is never wiped
- what fails next: same supply in several places is added together

`crypto/` (optional passphrase lock: Argon2id + XChaCha20-Poly1305) has 6 tests that run on GitHub only, because it needs outside crates my workspace can't download. They pass there.

Every push also builds screenshots of each screen and publishes them on the repository's `screenshots` branch.

## Built by GitHub Actions (compiles, not yet run by a person)
Every push runs three jobs: the core and safety tests, the interface type check and build, and a Windows build of the whole app. The first run passed all three. That proves the code compiles and type-checks on Windows. It does **not** prove the window opens and behaves, because nobody has run it yet.

Download the result: open the repository on GitHub, go to **Actions**, open the latest run, and download the **Cindrix-windows-portable** artifact. Unzip it on a Windows PC and run `cindrix-ops.exe`. The `content` folder must sit next to it.

## To run it from source on Windows (optional)
1. Install: Rust (rustup), Node.js 22, Visual Studio Build Tools (C++), and WebView2 (already on Windows 11).
2. In the repository folder:
   ```
   npm install
   npx tauri icon src-tauri/icon.svg
   set CINDRIX_ROOT=%CD%
   npm run tauri dev
   ```
3. Try: "How long do I boil water?" (answers from the sample library, with a source), "How many units of insulin should I take?" (fixed reply plus the Low blood sugar card), "How do I tune a violin?" ("not found").

Until llama.cpp is connected there is no model. The reply shows the best matching library passage and says so.

## Test the core by itself
```
cd core
cargo test
```

## Still to do in this slice
- Connect llama.cpp (Windows first) behind the `Model` trait in `core/src/pipeline.rs`.
- Android hello-world build.
- Portable build check: copy the exe and `content/` to a USB folder and run it.


## Update: the chat engine

The Windows download now also contains:

- `engines/` with `llama-server.exe` (llama.cpp, MIT) and Kiwix's `kiwix-serve.exe`. Both listen on 127.0.0.1 only and are stopped when the app closes.
- `models/` (empty). Put a Qwen3.5 `.gguf` file here. The app picks the size from your memory (2B under 12 GB, 4B from 12 GB). Ask shows "Model ready" when it has loaded.
- `library/` (empty). Put Kiwix `.zim` files here, or run `scripts/build-drive.ps1 -Target <folder>` to download them. Answers then draw on them as well as on the small built-in library.

Honest status: the code is tested with fake servers and builds on GitHub, but nobody has run it with a real model or real archives yet. The Kiwix search output format is parsed from documentation, so it is the most likely thing to need a fix on first real use.
