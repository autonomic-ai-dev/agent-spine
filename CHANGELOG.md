# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-19

### Added
- **Single-command install**: `install.sh` auto-detects OS/arch, downloads the correct binary from GitHub releases — `curl -fsSL https://raw.githubusercontent.com/aeswibon/agent-spine/main/install.sh | bash`
- **opencode config**: Project-level opencode configuration with agent-brain MCP integration

### Changed
- **README**: Replaced multi-step binary install with a single one-liner at the top

## [0.1.0] - 2026-06-19

### Added
- **Stateful Execution Engine**: A robust graph traversal engine that natively supports cyclic execution loops and state machines without explicitly managing LangChain/CrewAI boilerplates.
- **Time-Travel Debugging**: Immutable, append-only `FileStateStore` that dumps `StateSnapshot` payloads into a local `.jsonl` file to instantly replay loops and identify exact failing iterations.
- **Native IDE Supervisor**: A lightweight, pausing orchestrator that acts as an IDE hook via an embedded gRPC `SupervisorService`.
- **Confidence Router**: Intelligent execution routing that detects iteration ceilings (e.g., 5 continuous failures) and automatically injects an `escalation_required` flag into the state snapshot to escalate local API execution to a frontier model.
- **CI/CD Pipeline**: GitHub Actions orchestrator setup using `pipeline-compose` for linting, testing, building, and releasing.
- **agent-brain MCP Bridge**: `McpBridge` JSON-RPC 2.0 client over child-process stdio — handshake, tool calls, server request handling, with wrappers for `route_task`, `store_memory`, `store_trajectory`, `get_context`, `list_memory`.
- **BrainRouter**: Lazy-connect MCP bridge with graceful fallback to `ConfidenceRouter`, payload enrichment via agent-brain, trajectory logging at execution milestones, and transition routing.
- **Executor brain integration**: Phase 1 payload enrichment, Phase 4 trajectory logging, Phase 5 brain-backed routing with `ConfidenceRouter` fallback.
- **`brain` CLI subcommand**: `health`, `route`, `status` for interacting with agent-brain at runtime.
- **Async Send safety**: Extracted `MutexGuard`-holding operations into non-async helpers to satisfy `tokio::spawn` Send bounds. Added Send compile-time checks for `Executor`, `BrainRouter`, `McpBridge`, `ConfidenceRouter`.

### Changed
- **Executor::run()**: Now enriches agent/checkpoint payloads via BrainRouter, logs trajectories at init/complete/node milestones, and routes transitions through BrainRouter with fallback.
- **README**: Updated architecture diagrams, core modules table, and capability list to reflect MCP bridge integration.

### Fixed
- **Send-bound compilation**: `Executor<InMemoryStateStore>` not `Send` due to `MutexGuard` across async boundary — resolved by moving lock operations into non-async `append_snapshot` helper.
- **Test timing**: Functional tests failing because `McpBridge::connect(None)` would succeed when agent-brain is on PATH, causing slow LLM-based routing. Fixed by skipping MCP connect when `brain_cwd` is `None`.

### Removed
- Debug `#[tracing::instrument]` from `Executor::run()` — propagated Send issues; replaced with manual `tracing` calls.
