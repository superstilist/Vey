# Pending work and acceptance criteria

Planning snapshot: 2026-10-20. This documentation request is complete when the
notes/index are written and checked. The implementation backlog below is
suggested, not work currently in progress. Follow the next user's task first.
Issue IDs refer to `issues.md`; do not mark a task done from an old report.

## First: establish reliable platform behavior

| Task | Issues | Where to start | Done when |
| --- | --- | --- | --- |
| Broaden public playback acceptance | I-01 | `src/native.rs`, `src/server.rs`; `docs/PLAYBACK-RECOVERY.md` | Reported IDs now cache and decode fully. Check more catalog entries and audible/device playback separately; preserve bounded retry and actionable failures |
| Android device acceptance | I-10 | `docs/ANDROID-BUILD.md`, `src/lib.rs`, build script | Install on a real device/emulator; check launch, local playback, seek/rate/volume, pause/resume, rotation, settings persistence and accessible paths; record environment |
| Android storage integration (partial) | I-03 | `src/android.rs`, `src/dialogs.rs`, source/import/export callbacks | Device scanning now uses permission and MediaStore (0.1.5); verify grant/deny/revoke on device. Remaining SAF selection and import/export must round trip actual files without desktop path assumptions |
| Linux build/runtime acceptance | I-11 | CI workflow dependencies; native platform gates | Fresh build/test and launch/audio/lyrics smoke on working Linux; record fallback limits and actual CI result if checked |
| Platform lyric authentication | I-04 | `src/challenge.rs`, `src/lyrics_service.rs` | Supported platform challenge/token acquisition and cache/retry work end to end without Google session capture or secret logging |

## Next: finish user-facing feature gaps

| Task | Issues | Completion criteria |
| --- | --- | --- |
| Inline clip renderer | I-02 | Define platform scope; real video renders in player, follows audio seek/pause/rate, handles unavailable clips and releases resources |
| Browser local metadata/artwork | I-05 | Extract supported tags/covers from selected file bytes; update real title/artist/album/duration; preserve stable identity and handle malformed/large files |
| Browser reselection UX | I-06 | Explain unavailable session files and provide reliable relinking while preserving likes/playlists; do not promise unsupported permanent file access |
| Public details and pagination | I-07 | Audit existing service APIs first; implement actual album/artist/playlist navigation plus continuation, loading/error/empty states and cancellation of stale requests |
| Download progress/cancellation | I-07 | Progress reflects transferred data, cancel stops work, partial files are handled and failures are recoverable |
| Settings/interaction parity audit | I-07, I-09 | Compare original reference and current controls; inventory supported, deliberately excluded and missing items before adding callbacks |
| Lyric dialect fixes | I-09 | Start with a failing real fixture; parser/provider conversion preserves translations, romanization and backing vocals across native/web output |
| Web shell accessibility plan | I-12 | Assess current Slint support and alternative accessible UI approach; validate keyboard/screen-reader flows for the chosen scope |

## Maintenance

- I-13: reconcile README/parity/integration text with current UI and host failure
  behavior. Keep historical verification reports dated; do not overwrite them
  with new claims. Verify against current code and, for appearance, fresh visuals.
- For fallback work (I-08), describe exact platform support instead of promising
  all upstream effects. Keep upstream vendored files unchanged.
- For performance work, measure before/after on the same workload/environment;
  retain save flushing, bounded artwork/event work, hidden-frame suspension and
  interruption behavior. Avoid unmeasured global speedup/FPS claims.
- Before release, use current source version, rebuild affected widget/WASM/native
  assets, run relevant checks, inspect UI and verify package checksums. Android
  development signing is documented; production signing requires a deliberate
  release decision and the appropriate existing key.

## Updating task state

For newly assigned work record only: task, status (planned/in progress/blocked/
done), affected paths, next concrete step and verification evidence. Keep this
file a backlog, not a running conversation log. Link substantive evidence in
`docs/` and move fixed issues out of the open list when fresh checks support it.
