# 3. BrainRouter integration

## What we built

BrainRouter is an optional MCP bridge that connects agent-spine to agent-brain. When enabled, each workflow node can request context routing from agent-brain before execution, and log the execution trajectory back to agent-brain memory after completion.

### How it works

1. Before executing a node, agent-spine calls `agent-brain route_task` with the node description
2. agent-brain returns the most relevant skills, rules, and memory for that specific node
3. agent-spine injects the context into the node's input payload
4. After execution, agent-spine calls `agent-brain store_memory` with the trajectory

This means each node gets **task-specific context** rather than the same generic system prompt for every step.

### Configuration

BrainRouter is configured per-workflow or per-node:

```yaml
nodes:
  - id: lint
    kind: Agent
    brain:
      route: true           # Request context from agent-brain
      store_trajectory: true # Log execution to agent-brain memory
      max_context_tokens: 500
```

## Why this way

Without per-node context, every workflow step receives the same prompt — which means the last node gets the same context as the first, even though it's doing completely different work. BrainRouter ensures that:

- A "lint" node gets linting rules and project conventions
- A "deploy" node gets deployment checklists and infrastructure policies
- A "review" node gets code review guidelines and team standards

## Alternatives considered

| Option | Why rejected |
|--------|-------------|
| **Fixed context for entire workflow** | Simpler but wasteful — deploy doesn't need lint rules |
| **LLM determines context each step** | Unreliable and costly — the model might request irrelevant context |
| **No context at all** | Would require all knowledge in the base prompt, causing bloat |

## Trade-offs

- **BrainRouter adds latency per node.** Each `route_task` call takes ~1ms (warm cache). For workflows with hundreds of nodes, this adds up. Disable routing for high-throughput nodes.
- **MCP connectivity is required.** If agent-brain is not running or MCP is misconfigured, BrainRouter nodes fail. Use the `optional: true` flag for non-critical nodes.
- **Trajectory storage grows with executions.** Each `store_memory` call adds a fact. Configure retention in agent-brain to prevent unbounded growth.
