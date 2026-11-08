# Player timing report — 2026-11-06

## Summary

A release build of Vey was launched on Windows with `VEY_DEBUG_PLAYER=1` and
played real audio while the lyrics panel showed timed word highlighting. The
first capture lasted about 30 seconds. A second, extended capture lasted
111.5 seconds (the app remained open while observations were collected) and
contained 14,808 complete individual samples at inspection. The extended
capture showed frame gaps over 500 ms on Home, Library and Lyrics. The largest
was 1,001.65 ms on Home; Lyrics had a 743.82 ms maximum. This confirms that
sub-second and one-second UI stalls can occur during a real playback session.
The cause is not yet identified.

The diagnostic UI was asynchronous and its bounded queue reported no dropped
samples. The lyric controller stayed below 5.64 ms in the extended Lyrics
capture (below 1.55 ms in the first run), making that measured pass an unlikely
source of the 743.82 ms Lyrics gap. Renderer callbacks reached 229.41 ms, so
the current evidence cannot isolate whether the stalls came from UI work inside
the callback, OS scheduling, image decoding, GPU/driver work, or monitor
presentation.

## Environment and method

- Host: Windows desktop, release executable `target/release/vey.exe`.
- Capture enabled by `VEY_DEBUG_PLAYER=1` before process launch.
- Workload: real audio playback and synchronized lyrics; navigated between
  Home, Library and Lyrics. The lyric UI exposed word-level timing during
  playback.
- Samples were written by a named background worker to
  `%APPDATA%\PlayerVF-Rust\debug\player-timing-<timestamp>.jsonl`.
- First capture: about 30 seconds, 4,693 samples, zero dropped.
- Extended capture: 111.5 seconds, 14,808 complete samples at inspection,
  with playback active and position advancing; zero dropped samples.
- No track, lyric, URL, or account content is included in the JSONL report.
- The Vey window was no longer present when the next UI inspection ran. The
  Application and Windows Error Reporting logs had no matching `vey.exe`
  crash record in the checked interval, and stderr ended on a normal periodic
  timing summary. Therefore the cause of process/window disappearance is
  unknown; this run does not establish a crash or a user-initiated close.

## Measured render cadence

Values are per-event distributions aggregated from the JSONL sample records.
For `frame_interval`, values are elapsed milliseconds between Slint render
callbacks. For other events, they are measured callback/update durations.
P95/P99 use nearest-rank-like zero-based indexing in the local aggregation;
the executable's one-second summaries use its own percentile calculation.

| Route | Event | Samples | P50 ms | P95 ms | P99 ms | Max ms | >33 ms | >100 ms | >250 ms | >500 ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Home / lyrics tab | Frame interval | 1,104 | 6.22 | 29.39 | 33.51 | **623.09** | 14 | 3 | **2** | **1** |
| Home / lyrics tab | Renderer callback | 1,105 | 1.10 | 1.83 | 8.23 | 101.21 | 5 | 1 | 0 | 0 |
| Library / lyrics tab | Frame interval | 593 | 5.61 | 29.02 | 30.27 | 192.57 | 6 | 1 | 0 | 0 |
| Library / lyrics tab | Renderer callback | 594 | 1.01 | 4.51 | 15.66 | 168.61 | 5 | 1 | 0 | 0 |
| Lyrics / lyrics tab | Frame interval | 472 | 5.99 | 14.98 | 20.58 | 176.59 | 3 | 2 | 0 | 0 |
| Lyrics / lyrics tab | Renderer callback | 473 | 2.00 | 4.47 | 8.00 | 159.97 | 3 | 2 | 0 | 0 |

## Extended real-playback capture

| Route | Event | Samples | P50 ms | P95 ms | P99 ms | Max ms | >33 ms | >100 ms | >250 ms | >500 ms |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Home / lyrics tab | Frame interval | 5,706 | 6.01 | 29.55 | 33.56 | **1,001.65** | 83 | 9 | 6 | **1** |
| Home / lyrics tab | Renderer callback | 5,708 | 1.19 | 2.85 | 4.63 | 120.95 | 8 | 1 | 0 | 0 |
| Library / lyrics tab | Frame interval | 1,127 | 6.08 | 29.71 | 33.05 | **738.50** | 13 | 2 | 1 | **1** |
| Library / lyrics tab | Renderer callback | 1,128 | 0.89 | 3.61 | 5.25 | 116.79 | 1 | 1 | 0 | 0 |
| Lyrics / lyrics tab | Frame interval | 873 | 7.02 | 18.40 | 28.41 | **743.82** | 7 | 3 | 1 | **1** |
| Lyrics / lyrics tab | Renderer callback | 874 | 2.58 | 7.48 | 12.38 | 229.41 | 2 | 2 | 0 | 0 |

Eight frame gaps above 500 ms appeared approximately 4.4, 19.1, 34.2, 49.3,
64.4, 79.2, 94.3 and 109.4 seconds after recorder start. They occurred on
Home, Library and Lyrics. The roughly 15-second spacing is a clue, not a
diagnosis. Windows Application and Windows Error Reporting logs had no
matching `vey.exe` crash record in the checked interval. These are render
callback intervals, not direct measurements of monitor scanout.

## Lyrics update timing

| Event (Lyrics route) | Samples | P50 ms | P95 ms | P99 ms | Max ms | >16.7 ms |
|---|---:|---:|---:|---:|---:|---:|
| `lyrics_controller_update` | 201 | 0.24 | 0.57 | 0.63 | 1.55 | 0 |
| `fill_clock_projection` | 151 | 0.17 | 0.42 | 0.51 | 0.54 | 0 |

The extended capture also measured `startup_storage_load` at 349.47 ms and
three `navigation_handler` calls at 2.16, 3.14 and 0.87 ms. These cover the
synchronous navigation callback and immediate refresh only; they exclude later
asynchronous content and artwork work. Playback state is not attached to each
sample, so the log cannot prove it was playing at every individual long-gap
timestamp.

## Interpretation and next work

1. **Reproduce the periodic long gaps.** Collect repeated release runs with
   the same audio, window size, backend and route order. Log app
   activation/minimize state and playback state with frame samples.
   Acceptance: classify the recurring roughly 15-second stalls and remove
   unexplained >250 ms gaps from repeated active-playback runs; report all
   >33 ms gaps rather than hiding outliers behind averages.
2. **Correlate navigation and artwork work.** Use the newly added synchronous
   `navigation_handler` samples and add async task/artwork queue timing if the
   Home gap repeats. Acceptance: identify which work overlaps each long gap;
   no UI-thread disk/network/decode work should be inferred without a trace.
3. **Separate renderer cost from scheduling/presentation.** Capture Windows
   ETW/GPU scheduling or a high-speed frame/presentation trace for a repeatable
   gap. Acceptance: classify a pause as application CPU work, renderer/driver
   work, or external scheduling/presentation delay with evidence.
4. **Repeat Lyrics under line-boundary pressure.** Continue timed-word playback
   across several line changes and compare update/fill/render distributions.
   Acceptance: no lyric update over 16.7 ms, no >100 ms render gap attributable
   to lyric work, and visual word progression remains synchronized.

## Limits

These are two manually operated Windows release runs, not a statistical
benchmark. They do not cover Android, web/WASM, software-renderer corner
artifacts, large-library stress, cold-cache artwork pressure, minimized
playback, or monitor scanout FPS. No Android device/emulator verification was
performed. The metrics are diagnostic evidence, not proof that freezes are
fixed.
