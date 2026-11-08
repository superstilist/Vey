# Player timing diagnostics

Set `VEY_DEBUG_PLAYER=1` before launching the native Vey executable to capture
player timings across the whole app window. The older `VEY_TRACE_LYRIC_FRAMES`
switch also enables capture.

PowerShell:

```powershell
$env:VEY_DEBUG_PLAYER = '1'
Start-Process -FilePath (Join-Path (Get-Location).Path 'target/release/vey.exe') `
  -RedirectStandardError (Join-Path (Get-Location).Path 'target/player-debug-stderr.log')
Remove-Item Env:VEY_DEBUG_PLAYER
```

The recorder is created before the native window. Play a real track, open its
synchronized lyrics, and leave the player visible for at least 30 seconds. Move
through Home, Library and Lyrics while playback continues to capture each
rendered route. The report path is printed once as
`[player-debug] report=...`; it is also saved under
`storage::data_dir()/debug/player-timing-<timestamp>.jsonl`. The file is local
and contains no track title, artist, lyric text, URL or account data.

The recorder reports one-second summaries and stores each sample in JSON Lines.
Each sample has `at_ms`, `route`, `event` and `duration_ms`. Summary records add
sample count, p50/p95/p99/maximum duration, counts above 16.7, 33, 100, 250 and
500ms, and the number of samples dropped if the bounded queue filled. A nonzero
drop count means the diagnostic writer could not keep up; it never blocks the
UI while trying to catch up.

Events currently include:

- `frame_interval`: elapsed time between render callbacks, grouped by active
  view and player tab. Estimated FPS is `1000 / average_interval_ms`.
- `renderer_callback`: time from Slint's `BeforeRendering` to `AfterRendering`.
- `lyrics_controller_update`: duration of the full lyric timer pass, including
  row/focus/camera bookkeeping and paint preparation scheduling.
- `fill_clock_projection`: duration of the lightweight intermediate update to
  the synchronized fill clock.
- `startup_storage_load`: elapsed time from app startup through the initial
  local store load and cleanup.
- `navigation_handler`: time spent in the synchronous navigation callback,
  including view state changes and its immediate refresh work.
- `app_ui_timer_interval` / `app_ui_timer_callback`: the 16ms app timer's
  scheduling interval and full callback duration. A large interval with a short
  callback means the UI thread was not running between timer callbacks; a large
  callback duration points at work inside the app timer.
- `native_lyrics_timer_callback` / `ambient_timer_callback`: full duration of
  those periodic UI-thread callbacks.
- `account_timer_callback`: account UI timer duration, including result
  application. Native periodic catalog preparation and session writes now run
  on workers and are not included in this UI-thread span.
- `player_status_poll`, `media_session_set_playback`, and
  `embedded_browser_set_bounds`: duration of specific synchronous calls inside
  the app timer, to attribute a slow app callback.

The `over_*` counters are exceedance counts. For `frame_interval` they mean
frame gaps; for callbacks and controller events they mean execution durations.

Interpret `frame_interval` as UI render-callback pacing. It does not measure
scanout or the monitor's physical presentation time. `renderer_callback` is the
duration Slint exposes around its render callback; it is not GPU execution time.
OS scheduling, drivers and external workload can still affect the intervals.
Keep the same renderer, window size, playback track and route order when
comparing runs. A 500ms entry in `frame_interval` is a half-second gap; compare
the matching `renderer_callback` and `lyrics_controller_update` samples to see
whether drawing or the lyric update occupied that time. If both are short, the
stall was outside those measured callback spans.
