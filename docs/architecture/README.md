# agent-spine architecture documentation

This series explains **why agent-spine exists**, **how it is built**, and **what alternatives were considered** for each major design decision.

**Primary audience:** senior engineers and principal engineers evaluating adoption, extending the workflow engine, or reviewing design trade-offs. Contributors and power users can read the same material; operators who only need commands should start with [README.md](../../README.md).

The README tells you *how to run* agent-spine. These documents tell you *why it works the way it does*.

## How to read this series

Each article follows a consistent shape:

1. **What we built** — concrete behavior you can verify in the repo
2. **Why this way** — constraints and goals that drove the design
3. **Alternatives considered** — options we rejected or deferred (with reasons)
4. **Trade-offs** — what you give up, and known limitations
5. **For senior engineers / PEs** — invariants, failure modes, evaluation questions, scale evolution

### Reading paths

| If you are… | Start here | Then |
|-------------|------------|------|
| **PE / architect** deciding adopt vs build | [01](01-problem-and-design-goals.md), [04](04-concurrency-and-state.md) | [02](02-workflow-execution-model.md) |
| **Senior dev** integrating or debugging | [02](02-workflow-execution-model.md) | [03](03-brain-router-integration.md) |
| **Contributor** adding new node types | [02](02-workflow-execution-model.md), [04](04-concurrency-and-state.md) | Source code in `src/workflow/` |

| # | Article | Topics |
|---|---------|--------|
| 1 | [Problem and design goals](01-problem-and-design-goals.md) | Why DAG > script, determinism, audit trail, HITL |
| 2 | [Workflow execution model](02-workflow-execution-model.md) | YAML schema, node types, state machine, retries |
| 3 | [BrainRouter integration](03-brain-router-integration.md) | MCP bridge to agent-brain, context per node |
| 4 | [Concurrency and state](04-concurrency-and-state.md) | Fan-out/fan-in, immutable snapshots, state stores |
