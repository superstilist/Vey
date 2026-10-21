# Coding agent entry point

Read [.agent/project.md](.agent/project.md) first. Read
[.agent/workflow.md](.agent/workflow.md) for commands and validation relevant to
the task. These files are a compact index; open only the linked source or docs
needed for the current change.

Detailed notes are loaded on demand:
- [.agent/status.md](.agent/status.md): capabilities, platform matrix and recent work.
- [.agent/architecture.md](.agent/architecture.md): runtime flows, storage and integration contracts.
- [.agent/issues.md](.agent/issues.md): bugs, limitations, evidence and verification gaps.
- [.agent/tasks.md](.agent/tasks.md): suggested backlog with completion criteria.

For bug fixes read the matching issue and task first. Suggested backlog order
is guidance, not user authorization to start unrelated implementation work.

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
