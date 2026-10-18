# 13. Run the first slice

What works and what has been tested, so nothing is guessed.

## Tested (in the workspace, on Linux)
`core/` is a Rust crate with **no outside dependencies**. `cargo test` in `core/` runs 25 tests, all passing:
- the medication safety check against all 37 cases in `tests/safety/dosing.json`
- the answer path: dosing question never reaches the model; unknown topic gets "not found"; a model answer containing a dose is replaced; a model failure gives a plain message
- inventory CSV (round trip, days left, status, delete selected)
- card loading (a card can't claim to be reviewed without a reviewer and date)
- library search over `content/library-sample`

## Written but NOT tested
The workspace can't download Rust crates or npm packages, so these have never been built:
- `src/` (Svelte + TypeScript interface: Ask with sources, Library with emergency cards, grouped menu)
- `src-tauri/` (Tauri 2 shell exposing `list_cards` and `ask`)

Expect small errors on the first build. Send me the messages and I'll fix them.

## To run it on Windows
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
