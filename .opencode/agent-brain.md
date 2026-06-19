# agent-brain MCP (required)
instructions-version: 5

## The connection contract

**route_task is the only entry point** for agent-brain context on every host.

- Session digests from Cursor, OpenCode, Codex, Gemini, and Antigravity are stored in brain.db but **only surface through route_task**.
- Team memory, skills, rules, and must_apply constraints are **only injected through route_task**.
- Other agent-brain MCP tools (`grep_search`, `store_memory`, `get_context`, …) **return errors** until route_task succeeds for this turn.
- **Install and route_task refresh session ingests** — cross-agent threads are indexed into brain.db automatically.

If the agent skips route_task, cross-agent ingest and shared memory provide **zero value**.

## Every user turn

1. Call **`route_task`** with `user_message`, `current_working_directory`, and `open_files`.
2. Load skills/agents from returned paths; apply `applicable_rules` and `must_apply`.
3. Use `relevant_memory` (includes session digests when relevant).
4. At task end, call **`store_memory`** for durable outcomes (max 50 words, no secrets).

## Native host tools (OpenCode / Claude Code / VS Code / Gemini / Antigravity)

This host has **no hook gate** on Read/Shell/Grep. You must self-enforce:

- **Do not** use host Read/Cat/Grep for file exploration when agent-brain MCP is connected.
- **Use** agent-brain `grep_search`, `file_summary`, `read_file_head`, `read_file_tail` instead.
- Host native reads bypass routing, token savings, and cross-agent session digests.

On Cursor, hooks block host tools until route_task; on Codex and Claude Code, hooks gate MCP tools until route_task. On other hosts **you** must follow the same discipline.

## Continuing work from another IDE

When the user says "continue" or references work elsewhere:

1. Call **`route_task`** first — digests and memory may already describe the in-progress task.
2. Read **`agent-brain briefing`** or `~/.agent_brain/logs/last-route.md`.
3. Treat as in-progress work unless the user clearly changes direction.

Readable summary: `~/.agent_brain/logs/last-route.md` or `agent-brain briefing`.
