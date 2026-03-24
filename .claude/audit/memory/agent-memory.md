# Agent Core — Audit Memory

## Route Inventory
- POST /ask (entry point for all questions)

## Key Modules
- orchestrator.rs — trigger classification (7 classes)
- worker.rs — priority queue, concurrency limits, tool timeouts (30s/60s)
- claude.rs — API client with retry, streaming support
- conversation.rs — 20 turns, 50K chars, 10min timeout
- agent.rs — system prompt loading, context building

## Component Summary
Components to be discovered in first audit cycle.

## Known Issues
To be populated during audit.
