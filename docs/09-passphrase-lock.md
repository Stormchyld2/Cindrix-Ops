# 9. Optional passphrase lock

Decision 2: **optional passphrase.** Off by default. Covers the Family page only (names, conditions, medications, allergies, contacts). The library, maps, and inventory open without a password.

## Behavior
- Settings, "Lock the Family page", asks the user to choose a passphrase.
- When locked, opening the Family page asks for the passphrase. Other screens that show family data (Binder, Start progress) ask once per session or show it blank.
- The Binder can include a plain copy only after the passphrase is entered, and it warns that the printout is not protected.
- **There is no recovery.** If the passphrase is forgotten, the data cannot be opened. The setup screen says this in plain words and offers to print a plain copy first.
- Unlocking on a shared computer: the app locks again on exit and after 10 minutes idle.

## File format
Still JSON, so the five-format rule holds. `user/family.json` becomes:
```json
{ "version": 1, "encrypted": true,
  "kdf": { "name": "argon2id", "memory_kib": 65536, "iterations": 3, "parallelism": 1, "salt": "<base64>" },
  "cipher": { "name": "xchacha20poly1305", "nonce": "<base64>" },
  "ciphertext": "<base64>" }
```
Unlocked files are the normal JSON described in doc 2.

## Implementation notes (Rust core)
- Argon2id for the key, XChaCha20-Poly1305 for the data, both from audited Rust crates (RustCrypto family). No homemade crypto.
- A new random nonce on every save. Safe-write rules from doc 2 still apply, and the previous version in `backups/` is encrypted too.
- The key lives in memory only while unlocked and is cleared on lock.
- Argon2 settings must run acceptably on the 8 GB laptop and the phone (**TO TEST**).
- Not protected against someone who can run software on the same computer while it is unlocked. The screen says what the lock does and doesn't do.
