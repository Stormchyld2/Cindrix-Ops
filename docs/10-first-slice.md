# 10. First slice

Goal: prove the stack on real hardware before building the rest. Small, runnable, testable by the author on Windows.

## Scope
1. A Tauri 2 app that opens the dark shell (header, grouped menu, status bar) using the mockup styling and system fonts.
2. **Emergency cards** shown from Markdown files, with no model. Start with the Low blood sugar card as a draft.
3. **Safety check** in the Rust core, run against `tests/safety/dosing.json`.
4. **Chat** wired to llama.cpp with the 2B model, answering from a small test library, with sources shown and a "not found in the library" reply.
5. **User data folder** reading and writing a test `inventory.csv` safely.
6. A portable Windows build that keeps everything in its own folder.
7. A hello-world Android APK from the same code, installed on the Note 20, to test the riskiest part early.

## Out of scope for the slice
Maps, drawing, satellite downloads, drills, binder, the Family page lock.

## Done means
- Runs from a folder on the author's Windows PC with the network off.
- All safety test cases pass in an automatic test run.
- Chat answers a test question from the library and cites it.
- Memory and speed are measured on both Windows machines.
- The APK installs and opens on the phone.
- Anything that fails goes in a list in the repository.

## Who tests what
Claude writes the code and the tests, and can run the Rust tests that don't need big downloads. The author runs the Windows build and the phone install, since model files and the Android toolchain are too large for the workspace here.
