# 6. Stack decision

**Rust, Tauri 2, Svelte, TypeScript.** Chosen by the author.

## How the pieces fit
| Layer | Technology | Job |
|---|---|---|
| Interface | Svelte + TypeScript | All screens. System fonts only. Dark theme from the mockups. |
| App core | Rust (Tauri commands) | File access, user data, library search, safety check, map and export logic, encryption. |
| Shell | Tauri 2 | One codebase for the Windows app and the Android app. Uses the system web view. |
| Model | llama.cpp | Windows: run as a separate program the core starts. Android: linked into the app as a native library. |
| Library | Kiwix libzim | Read and search the archives from Rust. |

This replaces the earlier idea of a local web server. The interface talks to the Rust core directly, so no port is opened at all, which makes the "this app only talks to itself" statement simpler and stronger.

## Why this fits
- Small download and low memory use, which matters on an 8 GB machine that also has to run the model.
- Rust is good for safe file handling, encryption and speed.
- One interface code base for Windows and Android.

## Known risks (**TO TEST** early, in the first slice)
1. **Tauri on Android is newer than Tauri on Windows.** Plan on trying a hello-world APK on the Galaxy Note 20 before building anything on top.
2. **Running llama.cpp on Android.** A phone app can't reliably start a separate program, so llama.cpp has to be built as a library inside the APK. This is the hardest single piece.
3. **Windows web view.** Tauri uses Microsoft WebView2. It is part of Windows 11 and most updated Windows 10, but a bare older machine may lack it. The portable build must carry an offline installer or a fallback.
4. **Reading ZIM files from Rust.** libzim is C++, so we need bindings. If they cause trouble, we run Kiwix's own server as a helper on Windows.
5. **Map drawing in a web view.** Vector maps in a mobile web view can be slow. Test on the phone early.
6. **Portable on Windows.** Tauri apps normally install. We need a build that runs straight from the stick and keeps everything in the Cindrix folder, not AppData.

## Layout in this repository
```
src-tauri/        Rust core
src/              Svelte + TypeScript interface
engines/          llama.cpp build scripts
manifest/         what goes on the drive
tests/            safety tests and others
docs/  mockups/
```
