# Playlist import and connected accounts

Home and Library have an **Import playlist** action. Paste a public YouTube
Music/YouTube or Spotify playlist URL; neither public import needs a service
account. The desktop app uses its public catalog transport; the browser calls
the same-origin Vey backend (`vey --web-server`). Static hosting alone cannot
fetch playlists.

YouTube Music import fetches every page. Spotify public import reads the
service's unauthenticated embed-page metadata, then matches every track to a
YouTube Music song using exact normalized title/artist and a duration difference
under eight seconds. Both preserve source order and repeated songs and update a
stable local playlist identity. Spotify tracks are saved as `yt://VIDEO_ID`, so
normal Vey/YouTube Music playback handles them; Spotify audio is never played.
Unknown rows, ambiguous or missing matches, pagination cycles and import limits
fail without replacing a playlist. Local edits conflict with later remote changes
and remain intact. Remote deletions never delete Vey playlists. On upgrade, one
unambiguous playlist created by the retired Spotify account importer keeps its
existing local ID, avoiding a duplicate; ambiguous histories are left untouched.

Spotify's public embed currently exposes at most 100 rows and gives no total count.
If it returns 100, Vey refuses the import because it cannot prove the list is
complete. Public playlists shorter than that are supported; private playlists
and Spotify account playlists are not. This is an internal, undocumented page
format and can change without notice.

## Choose an account

Open **Connected accounts** from the import dialog or Settings → Account.
Choose Vey or YouTube Music. Connections are independent: connecting YouTube
does not sign in to Vey, and Vey does not require a YouTube account. Spotify
has no account connection; it is a public playlist import source only.

- **Vey** uses the existing browser/QR login and cloud library synchronization.
  Imported video references and playlists use the deployed cloud schema. Enable
  its automatic sync to carry them to another device. Local files remain local.
- **YouTube Music** opens Google’s normal `/v3/signin/accountchooser` in an
  installed Chrome/Edge browser. Choose your Google account there; Vey verifies
  the newly opened tab after it reaches YouTube Music. It then pulls saved/private
  playlists through the signed-in site, including Liked Songs when listed.
  This does not mirror subscriptions, listening history or every library object.
- **Spotify public playlists** are read without an account or developer app.
  Metadata is matched to YTM IDs, then stored as ordinary YouTube Music tracks.
  An unmatched track, podcast/other non-track row or ambiguous match leaves the
  playlist unchanged with an error. No private account data is read.

YouTube Music has an automatic pull switch. When enabled, it pulls after
login/startup and every 15 minutes while Vey is running. Vey's own automatic
cloud sync is a separate switch. Nothing writes back to YouTube or Spotify.
Disconnecting YouTube retains imported playlists. Cancel invalidates the result
and aborts the native request task.

## Connect YouTube Music on desktop

Select YouTube Music → Connect. No Google Cloud project, Client ID, client secret
or OAuth consent-app setup is needed. Google’s normal account chooser/sign-in
runs in a visible installed Chrome/Edge window; Vey never handles the password.
After the chosen tab reaches YouTube Music, the account name is verified using
`account/account_menu`, and its playlist library uses `FEmusic_liked_playlists`.
Requests are read-only. Unknown formats, account changes and pagination cycles
stop the pull instead of silently replacing a library with incomplete data.

Settings also offers **Open YouTube Music in the default browser**. That opens
Google's sign-in page through the operating system, using the browser and profile
already selected as default. It does not grant Vey access to that browser's tabs
or create a private-library sync connection. Private playlist sync below uses a
separate Vey-owned profile; Vey never reads or copies cookies from the everyday
browser profile.

Vey creates a dedicated persistent browser profile at
`DATA_DIR/youtube-browser/`. It does not inspect or copy your ordinary browser
profiles. The connection uses loopback-only Chrome DevTools transport, validates
the profile endpoint, and accepts completion only from its newly created tab.
The helper executes only at `https://music.youtube.com`, never on Google's login
or password pages. Session cookies and request authorization remain in that tab;
only normalized account/playlist metadata reaches Vey. The browser-owned profile
is excluded from Store backups and Vey cloud sync.

Keep that YouTube Music tab/window open for automatic pulls. If it is closed or
signed out, select Connect again; the browser profile remembers its website
session. If you switch Google accounts inside the browser, Vey requires a new
Connect before accepting the new account. Disconnect stops Vey synchronization
and retains imported playlists; it does not sign the browser out of Google.
Cancel stops Vey waiting for sign-in while leaving the normal browser available.

Chrome/Edge is detected in standard installation locations. Set
`VEY_YOUTUBE_BROWSER` to the browser executable path for another installation.
This integration currently targets desktop; browser WASM and Android retain
public playlist import and Vey cloud restore. No external-provider callback is
implemented in those hosts.

The YouTube website interface is undocumented and may change. References:
[Chrome profile isolation](https://developer.chrome.com/blog/remote-debugging-port),
[DevTools Runtime](https://chromedevtools.github.io/devtools-protocol/tot/Runtime/)
and [ytmusicapi’s library implementation](https://github.com/sigma67/ytmusicapi/blob/main/ytmusicapi/mixins/library.py).

### Spotify public playlist import

Paste an `https://open.spotify.com/playlist/...` URL into **Import playlist**.
Vey reads the public embed page without an account, OAuth, Spotify Development
Mode, or developer credentials. Each Spotify track is searched in YouTube Music;
only an unambiguous exact title/artist match with a duration difference under
eight seconds is accepted. All rows must match before Vey saves anything.
Matching uses the YTM song shelf, at most three concurrent lookups and a
three-minute import deadline; a cancelled or timed-out import does not change the
playlist. Independently verified track-ID matches may be cached for a retry.

The current embed response has no total-count field and stops at 100 rows. Vey
therefore rejects a response with 100 rows as potentially incomplete. Split the
source into shorter public playlists before importing. The page format is
undocumented, so parser changes may be required if Spotify changes its website.
An unauthenticated probe on 2026-11-04 read 50, 100, 50 and 75 track rows from
four public editorial playlists; the embedded playlist entity had no total-count
field. This is evidence for the guard, not a guarantee that every playlist has
the same cap.
The local `spotify-import-matches.json` cache stores only Spotify track IDs and
their matched YouTube video IDs; delete that file to force matching again. It is
not uploaded or included in Store backups.

Spotify Development Mode was evaluated and is not used here. Its Client
Credentials flow requires an app Client Secret, and the current Development
Mode playlist-items endpoint is limited to playlists the authorized user owns
or collaborates on. That does not cover arbitrary public playlist links without
a Spotify account connection. The selected public embed route avoids account
credentials but is undocumented and can change. See Spotify's [Client
Credentials flow](https://developer.spotify.com/documentation/web-api/tutorials/client-credentials-flow)
and [Development Mode migration guide](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).

Track matching currently uses YouTube Music's song search to obtain YTM video
IDs. It does not use the YouTube Data API: that API requires a project API key
for public search, and `search.list` has a separate daily quota. Do not embed a
private project key in desktop/mobile builds. See Google's [authorization
guide](https://developers.google.com/youtube/data/guides/authentication) and
[quota costs](https://developers.google.com/youtube/data/determine_quota_cost).

## Storage and compatibility

The `music-accounts.json` file stores only the YouTube Music browser connection
identity and tab ID; its website session remains in the dedicated browser
profile. On startup, older files are rewritten without the retired Spotify OAuth
record. The Spotify match cache contains public IDs only. Both files are excluded
from Store backups and Vey cloud requests. Existing verified track ID matches are
copied from the retired account file into the local cache during migration. The
browser has no access
to desktop sessions; the same-origin import endpoint handles public playlist
links only. Unix files use mode 0600; Windows uses the current user's data ACL.
Google passwords are entered on Google's page, not in a Vey form.

The deployed Vey backend allowlists older settings. Cloud serialization now
sends only those accepted fields; restores retain local layout, custom fonts,
theme CSS/scope and navigation blur. These appearance extensions stay local
until a coordinated backend schema update supports them.
The current cloud backend deduplicates repeated playlist references; repeated
entries are preserved in the local import, but do not survive a cloud round trip.

## Home and search

Six Home compositions are available with diagram previews: Editorial, Gallery,
Compact, Spotlight, Library and Discovery. A random option changes composition
on Home entry, never on a playback tick. The selection persists per device.

Home also loads a live YouTube Music recommendation shelf. It combines the
current song, recent history and liked tracks as radio seeds; local tracks first
resolve through the public catalog by title and artist. Shared results rank
higher, while the 12 most recent plays and liked tracks are filtered out and
repeated artists are limited on the first screen. If personalized radio cannot
return usable songs, Home shows public YouTube Music picks. Selecting a song
starts playback and queues the rest of the returned batch. The browser needs the
Vey same-origin backend for catalog and radio requests; no account connection is
required for this public data.

Search normalizes local metadata when it changes, matches query terms in any
order, and caps local results at 500. On Search, edits show local matches and
debounce the catalog request by 280 ms. Typing on another page does not navigate.
Enter submits immediately; native requests abort on replacement/navigation.
A bounded 24-query, five-minute cache avoids repeated catalog requests. Browser
generation checks reject superseded responses.

## Checks

Run with isolated state for application checks (`PLAYERVF_DATA_DIR`).

```powershell
cargo test --locked --lib -- --skip catalogs_have_all_languages_and_unique_keys
cargo check --locked --lib --target wasm32-unknown-unknown
cargo run --locked --example home_library -- target/NEW_HOME_FIXTURE
cargo run --locked --example home_library -- target/NEW_PHONE_FIXTURE --mobile
# Opt-in public network probe; prints counts only:
cargo run --locked --example playlist_import -- "https://music.youtube.com/playlist?list=PUBLIC_PLAYLIST_ID"
```

Browser search/input checks use the rebuilt WASM and fixture host on 4181:
`npm run test:search` in `tools/braccato-widget`. The read-only YouTube browser helper has an isolated fixture:
`npm run test:youtube-browser`. Live Google sign-in/private-library acceptance
requires the user to finish normal browser login. Spotify import is public-only;
private playlist behavior is intentionally unsupported.

### Verification on 2026-10-30

- Windows native debug binary/examples and rebuilt WASM passed. Rust library and
  integration suites: 264 passed, eight environment-dependent tests ignored;
  the existing `catalogs_have_all_languages_and_unique_keys` failure was excluded.
- All six Home compositions, picker, import form and Vey/YouTube/Spotify choices
  were captured at 1440×900 and 360×780 with isolated fixture data; inspected
  under `target/home-library-{desktop,mobile}-verified/`.
- Browser public import, storage/reload without account login, account selection
  and a real pointer selection of Compact passed:
  `target/library-import-browser-final-verified/`. Search debounce/cache/input
  passed at 1440×900, 320×640, 390×844, 430×932 and 844×390.
- Opt-in live public YouTube Music imports returned 7 and 552 tracks. The latter
  exercised pagination and unavailable entries with known video IDs. No
  personal account, private playlist or live Vey cloud round trip was used.
- Focused formatting and whitespace checks passed. Existing renderer warnings
  remain. Initial disk exhaustion was resolved by preserving the generated
  incremental cache at `F:/CodexBuildCache/vey-incremental-20261030`; final builds
  used `CARGO_INCREMENTAL=0`. Source and application state were not moved.

### Normal browser login correction, 2026-10-30

YouTube uses the requested normal v3 account chooser flow without a Client
ID/secret. Google redirects
the chooser to `/v3/signin/identifier` when the dedicated profile has no account.

- Native/WASM builds passed; Rust library/integration suites passed 267 tests
  with the same eight environment skips and existing locale test exclusion.
- `npm run test:youtube-browser` passed identity extraction, same-origin and
  account guards, refusal of write operations, private-library/LM parsing,
  pagination/repeats, malformed rows, rate limits and credential redaction.
- Native CDP transport tests passed response/event correlation and error
  redaction. Normal-login dialogs were inspected at desktop and phone sizes
  under `target/youtube-v3-{desktop,mobile}-check/`.
- `youtube_browser_login` opened installed Chrome and Google's real sign-in
  page in isolated state. The visible page was inspected with Computer Use;
  no email/password/2FA was entered. The pending check completed without marking
  an account connected. Authenticated private-library acceptance remains untested.

Opt-in interactive launch check (does not persist a Vey account connection):

```powershell
$env:PLAYERVF_DATA_DIR = Join-Path $PWD 'target/NEW_LOGIN_CHECK'
cargo run --locked --example youtube_browser_login
```

The ordinary **Connect** action in Vey persists a connection only after the
newly opened browser tab returns a verified YouTube Music identity.

### Spotify account retirement and public import (2026-11-04)

The Spotify account/OAuth connection was removed. Spotify URLs now use the
public embed-page import and every accepted row is mapped to a YouTube Music
`yt://` track. The 100-row response ceiling is treated as ambiguous and rejected
to prevent silent truncation. Native, browser and Android behavior still needs
fresh end-to-end acceptance; the older 2026-10-30 screenshots and reports above
describe the retired Spotify account UI and must not be treated as current.
