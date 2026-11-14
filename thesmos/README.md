# thesmos

Stateful workflow engine for supervising AI coding agents. Workflows are YAML DAGs. Each run keeps an explicit state history, and approval gates pause the run until a person decides.

Named for *thesmos* (θεσμός), Greek for an established law or ordinance: a workflow is a fixed order the agent must follow.

## Install

```bash
cargo install thesmos
```

Set up the local state and example workflows:

```bash
thesmos init
thesmos doctor
```

## Use

```bash
thesmos validate workflow.yaml          # check a workflow definition
thesmos run workflow.yaml --payload '{"task": "fix the parser"}'
thesmos status                          # current runs
thesmos log                             # event history
thesmos inspect <run-id>                # state at a point in a run
thesmos replay <run-id>                 # re-run a recorded history
```

Other commands: `serve` (dashboard and API), `mcp-serve` (MCP server for agent hosts), `agent`, `brain`, `event`, `update`.

Run state lives in SQLite. By default it is `~/.autonomic/logs/spine/state.db`. Pass `--db` to use another file.

## Works with tekmor

[tekmor](https://crates.io/crates/tekmor) routes skills and rules to the agent. thesmos runs the multi-step workflows. The two share the `~/.autonomic` home.

## Crates

| Crate | Purpose |
|-------|---------|
| `thesmos` | The engine, CLI, dashboard server and MCP server. |

## Docs

- [Repository](https://github.com/autonomic-ai-dev/thesmos#readme)
- [Changelog](https://github.com/autonomic-ai-dev/thesmos/blob/master/CHANGELOG.md)

## Build from source

```bash
cargo test -p thesmos
cargo run -p thesmos -- --help
```

## License

Apache-2.0
