# Releasing Shunno Browser

Every push to `main` builds `ShunnoBrowser.exe` on Windows, runs the tests and publishes it with
`latest.json` to the rolling `latest` release. Installed copies read `latest.json` daily.

## 1. Signed auto-update (free, do this once)

Until this is done the browser only *tells* users that a new version exists; it never installs one.

1. On a trusted computer with Python: `pip install cryptography`, then
   `python scripts/sign_update.py genkey`. (Only key generation uses Python. CI signs with the
   Rust tool `src/bin/sign_update.rs`, which refuses to sign when the key does not match
   `UPDATE_PUBLIC_KEY_HEX`, so a wrong secret fails the build instead of publishing an update that
   no installed copy accepts.)
2. GitHub, repo Settings, Secrets and variables, Actions, New repository secret:
   name `UPDATE_SIGNING_KEY`, value = the whole text of `update_signing_key.pem`.
3. Paste the printed public key hex into `UPDATE_PUBLIC_KEY_HEX` in `src/updater.rs` and push.
4. Store `update_signing_key.pem` offline (USB stick, password manager) and delete it from the computer.
   Never commit it. If it is lost, ship a release with a new public key while the old key still works.

From then on each release's manifest is Ed25519-signed; the browser downloads the EXE, checks the
signed SHA-256, swaps it in (the old EXE is kept as `ShunnoBrowser.old.exe` until the next start)
and offers **Restart to update**. A tampered manifest, EXE or download link is rejected.

## 2. Code signing (removes the SmartScreen "unknown publisher" warning; paid)

- Microsoft Artifact Signing (about USD 10/month) currently accepts individual developers only in the
  USA and Canada, so it is not available from Bangladesh.
- Alternative: an OV code-signing certificate from a commercial CA (typically USD 300 to 500 per year).
  Since 2023 these keys live in a hardware token or the CA's cloud vault, not a downloadable PFX.
  For CI, pick a CA with a cloud signing service and replace the `Sign EXE (Authenticode)` step with
  that CA's signing command. The current step works only if your CA issues a PFX
  (secrets `WINDOWS_PFX_BASE64`, `WINDOWS_PFX_PASSWORD`).
- Signing must run before the manifest step (it changes the EXE hash); the workflow already orders it so.

## 3. Before each release

- Bump `version` in `Cargo.toml` and `APP_VERSION` in `src/state.rs` (updates compare these numbers).
  The new number must be higher than every published one: installed copies ignore a lower version,
  so a renumbering such as 2.x to 1.0.0 would stop all of them from updating.
- Add a `CHANGELOG.md` entry.
- Generate third-party notices: `cargo install cargo-about && cargo about generate about.hbs > THIRD_PARTY_LICENSES.html`.
