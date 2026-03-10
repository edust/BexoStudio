# AGENTS.md

## Language
- Always reply in Simplified Chinese.

## Project Identity
- Product name: `Bexo Studio`
- Product type: `Rust + Tauri v2` desktop vibe coding toolbox
- Current stage: blueprint initialized, implementation not started

## First Read Order
Before making any meaningful code or architecture change, read these files in order:

1. `README.md`
2. `docs/product-requirements.md`
3. `docs/technical-architecture.md`
4. `docs/ui-system.md`
5. `docs/implementation-roadmap.md`
6. `task_plan.md`
7. `findings.md`
8. `progress.md`

## Product Goal
Bexo Studio exists to restore and orchestrate developer working state, not to be a generic chat client.

The v1 product must focus on:

- workspaces
- projects
- Codex profiles
- snapshots
- restore plans
- terminal / IDE / Codex orchestration
- tray and desktop integration

## Design Direction
- Layout should follow Cherry Studio style information architecture:
  left primary rail + middle section sidebar + right content area.
- Settings visual quality should be closer to CC Switch:
  grouped cards, clear toggles, compact desktop form rhythm.
- Do not produce generic dashboard UI.

## Locked Technical Direction

### Desktop / Runtime
- `Tauri v2`
- `Rust`

### Frontend
- `React`
- `TypeScript`
- `Vite`
- `@vitejs/plugin-react-swc`

### UI / Styling
- `Tailwind CSS v4`
- `Radix UI`
- `shadcn/ui`
- `lucide-react`
- `motion`

### State / Data
- `Zustand`
- `TanStack Query`
- `react-hook-form`
- `zod`
- `TanStack Virtual`

Do not switch to Electron, Ant Design, Redux, or styled-components without a written architectural reason in `findings.md`.

## Architecture Rules

### Rust Owns System Control
All system-sensitive operations must live in Rust:

- process spawn
- terminal orchestration
- IDE launch
- path validation
- `CODEX_HOME` injection
- snapshot restore execution

The frontend must not execute arbitrary shell commands directly.

### Adapter Pattern Is Mandatory
Use explicit adapters for:

- terminals
- IDEs
- Codex
- desktop integration

Windows-first implementation is acceptable, but adapter boundaries must preserve future macOS / Linux support.

### Persistence
Use structured persistence:

- SQLite for domain data
- local store for lightweight app preferences
- structured log files for restore runs

## Reliability Rules

- Every external command must have timeout support.
- Retries are allowed only for idempotent detection or probe actions.
- Restore execution must be batch-based and traceable.
- Validation errors must be explicit and user-visible.
- Sensitive information must never appear in logs.

## UI Rules

- Large lists must support virtualization.
- Heavy routes must be lazy loaded.
- Long-running actions must show progress and failure state.
- Tray mode and window mode must share consistent state.
- New pages must fit the three-pane shell instead of inventing isolated layouts.

## Workflow Rules

### Planning With Files
For any multi-file change or non-trivial feature:

1. Update `task_plan.md`
2. Record important discoveries in `findings.md`
3. Log work and verification in `progress.md`

Also maintain `scripts/work/<date>-<task>/` files when the task is feature-sized.

### Documentation Sync
When changing contracts, data model, UX flow, or architecture:

- update the relevant file under `docs/`
- update `README.md` if onboarding meaning changes
- update planning files if the plan changed

## Code Quality Bar

- No placeholder implementations
- No silent failures
- No unbounded retries
- No hidden state mutation without logging or explicit intent
- Keep modules small and responsibility-driven

## Preferred Initial Module Breakdown

### Frontend
- `src/app`
- `src/layouts`
- `src/pages`
- `src/features`
- `src/components`
- `src/stores`
- `src/queries`
- `src/lib`
- `src/types`

### Rust
- `src-tauri/src/commands`
- `src-tauri/src/domain`
- `src-tauri/src/services`
- `src-tauri/src/adapters`
- `src-tauri/src/persistence`
- `src-tauri/src/logging`
- `src-tauri/src/error`

## What To Build First

1. Project bootstrap
2. App shell
3. Workspace domain and persistence
4. Profiles domain
5. Snapshot / restore planner
6. Windows adapters
7. Tray and diagnostics

## Handoff Expectation
Every substantial turn should leave the repo in a state where another Codex can resume from the files on disk without relying on chat memory.
