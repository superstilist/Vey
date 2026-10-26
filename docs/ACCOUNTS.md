# Vey accounts

Vey uses the deployed Cloudflare API at
https://blueberry-melody.superstilist.workers.dev. Settings > Vey account >
Continue with Vey opens the system browser and asks you to confirm the matching
code. An existing website session can be reused without entering the password
again. Confirm or cancel explicitly; signing in never silently approves a device.
The browser tab closes after completion when the browser permits it. If a popup
is blocked, use Open sign-in page. Email or another server opens the manual form.
No client secret or provider password belongs in the application source.

Choose QR sign-in to display a locally generated QR without opening a browser.
On your signed-in phone, open the website Account page (or /connect), choose
Scan QR with camera, and confirm the matching code. The PC signs in automatically.
Camera decoding stays on the phone; it stops after a scan, cancellation, account
change or hiding the page. QR image upload and manual eight-character code entry
are fallbacks. Codes expire after ten minutes and can be used once. QR payloads
contain only the public consent URL, never the device secret or login token.

The website also offers Sign in with QR when logged out. Its fixed public client
`vey-web` uses the same consent flow, then establishes the normal website session.
No Google Cloud project or embedded application secret is needed.

Basic accounts sync public video references, likes, playlists, listening history,
player settings, lyric offsets and per-song lyric choices. Local filesystem paths,
provider credentials and lyric caches are excluded. An unuploaded local song stays
on its device; its audio and local-only references are not part of Basic sync.

Sign in with your Vey email/password, or choose website login and approve the
matching code in your signed-in site account. When Turnstile is enabled, use the
website flow. A new registration requires a password of at least 10 characters.
After signing in, explicitly choose Save this device or Restore from cloud once.
Automatic sync then checks every 15 seconds while Vey is open. A conflicting
revision pauses automatic sync and asks which copy to keep. Restore preserves
local-only tracks and playlist entries, but replaces synchronized preferences.

Premium is an administrator-issued developer/tester invitation. No checkout or
purchase exists. Refresh account access after an invitation. Select a local song
already imported into Vey and upload it, or enable automatic upload while Vey is
open. Uploads use 5 MiB chunks; maximum file size is 512 MiB. The default invited
quota is 1 GiB. Cancelling a transfer removes its pending cloud upload.

Refresh cloud files and download a song onto each device before playing it.
Native downloads are cached under the account's private application-data folder.
Browser imports and downloads use temporary Blob URLs; reselect/download after
reloading the page. Automatic upload runs while the app/page is open, not as an
Android background service. Revoking Premium stops new uploads; users retain
access to their already stored files.

The native opaque login token is separate in `account-session.json` beside the
application state, excluded from library exports. Unix permissions are 0600;
Windows uses the current profile's filesystem access controls. This is not an OS
credential vault. Browser tokens use sessionStorage. Passwords are cleared after
sign-in and never stored in Store or synced. Logout clears the device session
immediately and attempts server revocation; password reset invalidates opaque
Vey sessions on the server.

Backend setup, migration and invitation instructions:
`C:/Users/superstilist/WebstormProjects/blueberry_melody/docs/VEY-ACCOUNTS.md`.

## Development checks

Start the isolated Worker fixture using its backend documentation, then run:

```powershell
$env:VEY_FIXTURE_API='http://127.0.0.1:4879'
cargo test --locked --lib account::tests::native_client_worker_contract -- --ignored --test-threads=1
```

With a debug web build and `node serve.mjs` running in tools/braccato-widget:

```powershell
node tools/braccato-widget/account-site-test.mjs
node tools/braccato-widget/account-oauth-test.mjs
node tools/braccato-widget/account-qr-test.mjs
node tools/braccato-widget/account-web-qr-test.mjs
node tools/braccato-widget/account-transfer-test.mjs
node tools/braccato-widget/account-wasm-test.mjs
```

These tests use isolated fixture accounts and Edge headless, never a personal
library. The native contract test exercises the actual Rust DTO, Premium denial,
invitation, multipart upload and downloaded bytes against Miniflare. Browser
checks cover mobile site controls and the compiled WASM account controller.
Native layout fixtures simulate phone insets; they are not Android device proof.

The sign-in protocol is OAuth 2.0 Device Authorization Grant (RFC 8628), with the
public client_id `vey` and scope `vey`. Discovery is at
`/.well-known/oauth-authorization-server`; tokens stay in Authorization headers.
Polling respects authorization_pending and slow_down. There is no Google Cloud
project, embedded client secret, authorization-code callback or payment provider.

On 2026-10-23 both QR browser suites passed against the public Worker: actual
app/site QR pixels decoded, separate phone sign-in and account switching,
camera pipeline with a simulated stream, explicit consent, automatic PC sign-in,
website reload, manual grouped code, image fallback, denial and cancellation.
Windows debug executable and signed Android arm64 QR preview were built. These
checks do not replace a physical Android camera/device acceptance test.

New account sessions issued from 2026-10-23 have no automatic time limit on the
server. Sign in again once to replace an older timed token. App logout/revocation
and password-reset invalidation still apply to opaque Vey tokens; QR authorization
codes still expire after ten minutes. Browser WASM storage remains sessionStorage,
while website login uses its existing localStorage persistence.
