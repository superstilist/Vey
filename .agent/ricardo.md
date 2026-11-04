# Ricardo — shorthand, memes and trigger phrases

Informal. `AGENTS.md` and current source stay authoritative; this file records
how the user phrases requests and what each phrase should start. Read it with
the rest of `.agent/` at the start of project work. No secrets, no logs, no
claims that a check was run.

## Triggers

| The user writes | Start this |
| --- | --- |
| `start bimba` / "start the bimba" | The mega super duper sweep below, in order, then report what actually ran. |
| `yupo clean` | Cleanup of generated output only (`target/`, `dist/`, `web/pkg/`, `tools/braccato-widget/node_modules/`); never tracked files. See workflow.md, "Release cleanup". |
| `contion` / `contiuon` | Continue the interrupted task from where it stopped. |
| `live mode` | Use the user's requested casual "speak toxical" voice: natural, blunt, funny banter with occasional sharp jokes. Keep it conversational, not theatrical or framed as a team lead. Do not target the user with genuine abuse; avoid slurs, threats or cruelty. Use a normal careful tone for serious or sensitive topics. |

"Ultra check UI", "opt UI", "all pages", "lyric verify", "mega super duper" all
mean the same full sweep. Short wording is not permission to skip a group.

## Live mode

When the user says "live mode", speak primarily in Ukrainian. Use a casual,
natural, slightly toxic-funny voice like close friends teasing each other: be
blunt and make quick jokes. Keep the humor dry and conversational; skip mock
speeches, exaggerated theatrics and team-lead language. Tease the bug, code or
situation, not the user. Keep technical answers accurate and useful, and switch
to a normal careful tone for serious or sensitive topics. No slurs, threats or
genuinely cruel personal attacks.

Example vibe: "Ну звісно, кнопка вирішила сьогодні пожити окремим життям. Дивлюсь, де саме вона втекла."

The user also asked that the AI learn to speak in this style: treat this note as
a preference for future replies when they ask for it, not as a request to train
or change a model.

## Mega super duper sweep

Prerequisites: project root, PowerShell or bash. `target/`, `web/pkg/` and
`node_modules/` are not in the checkout, so the first build is cold. Device,
live-network and audible-playback checks are **not** part of this sweep.

1. `git status --short` — note unrelated work and leave it untouched.
2. Rust gates: `cargo fmt -- --check`; `cargo test --locked --lib --tests`;
   `cargo build --locked`;
   `cargo check --locked --lib --target wasm32-unknown-unknown`.
3. Native UI + lyric fixtures:
   `cargo run --locked --example native_lyrics`;
   `cargo run --locked --example native_lyric_timing`;
   `cargo run --locked --example native_lyric_boundary`;
   `cargo run --locked --example native_lyric_motion`;
   `cargo run --release --locked --example native_line_flow -- continuous`.
4. Widget bundle: `cd tools/braccato-widget && npm ci && npm run build`,
   then `npm run serve` (127.0.0.1:4181) in a second terminal.
5. Page checks: `npm test`, then `test:app`, `test:refinement`, `test:phone`,
   `test:playlists`, `test:appearance`, `test:transitions`, `test:search`,
   `test:material`, `test:navigation`.
6. Lyric checks: `test:sync`, `test:syllables`, `test:clock`, `test:stability`,
   `test:recovery`, `test:cue-boundary`, `test:custom-theme`.
7. Run `test:video` before `test:refinement` when the FFmpeg fixture is missing.
8. Account/Composer checks (`test:account-*`, `test:composer`,
   `test:site-material`) need their own Worker fixtures on 4879 / 4891+4892.
   Run them only when the sweep explicitly includes accounts.
9. Report: commands run, pass/fail, skipped groups and why. A prior report is
   historical, never fresh verification.

If a step fails, stop the sweep and fix or report it; do not mark later groups
as passed because an earlier one did.

## Glossary

| Term | Meaning |
| --- | --- |
| Vey | App, binary and package name. |
| PlayerVF | Legacy library/storage/bridge identity, kept for compatibility. |
| Braccato | The only lyric theme: upstream JS iframe in the browser, copied Rust port on native targets. |
| bimba | The user's trigger word for the full sweep. |
| mega super duper | The sweep with nothing skipped. |
| yupo | The user's word for cleanup. |
| ricardo | This file: informal shorthand, not project truth. |
| kawarp | `@kawarp/core`, the bundled ambient artwork texture used by the browser. |
| RICS | Upstream Braccato theme compiler; browser-only. |
| Turnstile / JWT | Lyric-provider auth. Tokens are never written here or logged. |

When this file and the source disagree, the source wins; when it and
`AGENTS.md` disagree, `AGENTS.md` wins.
