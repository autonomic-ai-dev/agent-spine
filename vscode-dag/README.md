# Autonomic Spine DAG (VS Code Extension)

Phase 4 visualization layer for `~/.autonomic/logs/spine/executions/*.dag.json`.

## Features

- **Explorer sidebar** — lists recent DAG summary files
- **Webview panel** — tabular view of transitions and payload keys per snapshot

## Development

```bash
cd vscode-dag
npm install
npm run compile
```

Press F5 in VS Code to launch the Extension Development Host, then run **Autonomic: Refresh DAG View**.

## Data source

Reads from `~/.autonomic/logs/spine/executions/` (written by `agent-spine` on workflow completion).
