# Releasing Xerra

Releases are built by GitHub Actions and published to
[github.com/aleh11/xerra/releases](https://github.com/aleh11/xerra/releases).
The in-app updater reads `latest.json` from the latest published release.

## One-time setup

1. **Enable Actions** on the fork (Actions tab -> enable workflows).
2. **Add the updater signing key** under Settings -> Secrets and variables ->
   Actions -> New repository secret:
   - `TAURI_SIGNING_PRIVATE_KEY`: the full contents of the private key file
     (the one matching the public key in `src-tauri/tauri.conf.json`
     `plugins.updater.pubkey`). Never commit this file.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the key password. Leave it empty or
     omit the secret if the key has no password.
3. No other repository settings are needed: the workflow requests
   `contents: write` itself and uses the built-in `GITHUB_TOKEN`.
4. _(Optional, later)_ **macOS notarization** (removes the Gatekeeper warning).
   Needs a paid Apple Developer account. Add secrets `APPLE_CERTIFICATE`
   (base64 of the Developer ID `.p12`), `APPLE_CERTIFICATE_PASSWORD`,
   `KEYCHAIN_PASSWORD` (any random string), and for notarization `APPLE_ID`,
   `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`.
5. _(Optional)_ **Windows signing** via Azure Trusted Signing: secrets
   `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` and repository
   variables `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`,
   `AZURE_SIGNING_PROFILE`.

Signing is conditional. Missing Apple/Azure secrets mean unsigned builds, never
a failed workflow. If `TAURI_SIGNING_PRIVATE_KEY` is missing the build still
succeeds but produces no updater artifacts or `latest.json`, and logs a
warning.

## Cutting a release

1. Bump the version in **all three** places (the Release workflow fails if they
   disagree):
   - `src-tauri/tauri.conf.json` (`version`)
   - `package.json` (`version`)
   - `src-tauri/Cargo.toml` (`[package] version`; run `cargo check` so
     `Cargo.lock` follows)
2. Optionally add user-facing notes in `src/content/release-notes/<version>.md`
   (shown in the app after the update).
3. Merge to `main`.
4. Actions -> **Release** -> Run workflow (branch `main`). Alternatively push a
   tag `v<version>`; an existing draft for that tag is reused, never
   duplicated. The tag must equal `v` + the app version.
5. Wait for all matrix jobs (a cold build takes a while; jobs are independent,
   so one failing platform does not cancel the others).
6. Open the **draft release** `v<version>`, check the assets, edit the notes,
   and **Publish**. Publishing creates the git tag.

### What the draft contains

- macOS Apple Silicon: `.dmg` and `.app.tar.gz` (+ `.sig`); macOS Intel too
- Windows x64 and ARM64: NSIS `.exe` and `.msi` (+ `.sig`)
- Linux x64 and ARM64: `.AppImage`, `.deb`, `.rpm` (+ `.sig` where applicable)
- `latest.json`: the updater manifest

Published releases cannot be re-run with the same version; bump the version
instead.

## How the updater finds a release

`tauri.conf.json` points the updater at
`https://github.com/aleh11/xerra/releases/latest/download/latest.json`. GitHub
resolves `latest` to the newest **published, non-draft, non-prerelease**
release, so the update only becomes visible after you publish the draft. The
app downloads the platform bundle listed in `latest.json` and verifies its
signature against the public key embedded in the app. A release signed with a
different key will be rejected by already-installed apps, so keep the key safe:
losing it means users must reinstall manually.

## macOS Gatekeeper (unsigned builds)

Without an Apple Developer certificate the macOS build is only ad-hoc signed
and not notarized. On first launch macOS may say the app is damaged or from an
unidentified developer. Either right-click `Xerra.app` -> **Open**, or run:

```bash
xattr -dr com.apple.quarantine /Applications/Xerra.app
```

Then grant **Microphone** and **Accessibility** permissions when prompted.
Because ad-hoc signatures change on every build, macOS may ask you to re-grant
Accessibility after each update until the app is signed with a stable
certificate.

## Other workflows

- `main-build.yml` builds all platforms on every push to `main` (artifacts kept
  30 days). It is expensive on macOS runners; disable it in the Actions tab if
  you do not need it.
- `build-test.yml` / `pr-test-build.yml` are manual test builds.
- `test.yml`, `code-quality.yml`, `playwright.yml`, `nix-check.yml` run on
  pushes and PRs and need no secrets (`nix-check` can use an optional
  `CACHIX_AUTH_TOKEN`).
