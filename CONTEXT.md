# agent-spine — Context for agents

Deterministic workflow DAG engine for Autonomic. Part of [agent-body](https://github.com/autonomic-ai-dev/agent-body).

| Field | Value |
|-------|-------|
| Role | Workflow engine (Argo / Temporal analog) |
| Binary | `agent-spine` |
| HTTP | `:3001` API · `:3100` event bus / dashboard relay |
| Version | v1.0.0-beta.2 |
| Docs Hub | [https://autonomic-ai.dev/docs/workflows](https://autonomic-ai.dev/docs/workflows) · [Architecture](https://autonomic-ai.dev/docs/architecture) |

## When to use

Multi-step procedures with retries, snapshots, and HITL gates — not improvised prompt loops. Submit YAML DAGs via CLI or HTTP; pause on `ApprovalGate`.

## Key verbs

- `agent-spine run` / `init --with @workflow`
- `agent-spine serve` — gRPC/event bus
- Register with nerves; hydrate nodes via brain `get_context_for_node`

## Verification

Orchestrator nodes should record exits with:

```bash
agent-brain verify --source orchestrator -- <allowlisted-cmd>
```

Do not use MCP `record_verify_outcome` for learning credit.

## CI

Path filter on `agent-spine/**` + Cargo/Makefile/scripts/`.github`. Docs-only PRs skip the compose pipeline.

## Related

Operator usage: [https://autonomic-ai.dev/docs/workflows](https://autonomic-ai.dev/docs/workflows) · Architecture: [https://autonomic-ai.dev/docs/architecture](https://autonomic-ai.dev/docs/architecture) · Brain (routing/verify) · muscle (execute) · mouth (approvals) · heart (budget before LLM nodes)
