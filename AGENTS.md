# Coding agent entry point

Read every Markdown file in `.agent/` at the start of project work, including
`project.md`, `workflow.md`, `status.md`, `architecture.md`, `issues.md`,
`tasks.md` and `ricardo.md`. Treat these notes as context; current source and
the instructions below take precedence where they differ. Then inspect only
the source and documentation relevant to the task.
Remember and apply the user's established wording, preferences and trigger
phrases recorded in `.agent/ricardo.md` in future project conversations.

For bug fixes read the matching issue and task first. Suggested backlog order
is guidance, not user authorization to start unrelated implementation work.

Version 0.1.27 (`.agent` notes refreshed 2026-10-27). `target/`, `dist/`,
`web/pkg/` and `node_modules/` are not in the checkout; the first build is cold.

- Check `git status --short` before editing; preserve unrelated work.
- This is standalone Rust/Slint Vey, formerly PlayerVF. Keep it separate from
  the original Tauri project. The binary/package is `vey`; the library remains
  `playervf` for compatibility.
- Current source and task instructions take precedence over cached notes.
  Older PlayerVF naming and visual notes can be stale.
- Keep `.agent` concise. When a task changes architecture, commands or project
  constraints, update the relevant note rather than appending a transcript.
- Do not store credentials, personal library contents, full logs, generated
  output or speculative completion claims in `.agent`.
