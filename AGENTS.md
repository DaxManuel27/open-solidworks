# AGENTS.md

Guidance for AI coding agents and human contributors working in this repository.
Read [docs/plan.md](docs/plan.md) first for the vision, phases and architecture.

## Architecture

- `apps/desktop`: Electron + React + TypeScript UI. The renderer is sandboxed and reaches the
  outside world only through the preload API (`window.openforge`).
- `crates/*`: the Rust core, fully separate from the UI. The desktop app runs the `openforge`
  binary as `openforge serve` and talks to it over JSON-RPC 2.0 (`of-protocol`). The same binary
  exposes a CLI and an MCP server.
- Crate dependencies point one way: `openforge` → `of-assembly` → `of-model`, `of-mate`;
  `openforge` → `of-io`, `of-protocol`; `of-model` → `of-sketch`, `of-geom` → `of-kernel-sys`.
  `of-mate` (the assembly mate solver) depends only on `glam` and `nalgebra`. Lower crates
  never depend on higher ones.
- `of-kernel-sys` is the only crate that touches C++ (OpenCASCADE). All `unsafe` lives
  there. Everything above it is safe Rust. The sketch solver lives in `of-sketch`; the mate solver in `of-mate`.
- Topological naming: faces and edges get stable IDs in `of-model`. Features reference geometry
  through those IDs, never through raw kernel indices.

## Conventions

- Rust edition 2024. `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` must be clean.
- TypeScript: strict mode, Prettier (no semicolons, single quotes, width 100), ESLint clean.
- Units inside the core are millimetres and radians. Convert only at the UI boundary.
- Prefer small, focused crates and modules. Add a test with every behaviour change.
- Don't commit build output (`target/`, `out/`, `dist/`) or downloaded OpenCASCADE binaries.

## Branches

`main` is the release branch and only moves when a phase is complete. Work happens on the current
phase branch (`phase-1`, `phase-2`, …) or on short-lived branches off it. Merge the phase branch into
`main` and push only when the phase is done.

## Checks

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
pnpm lint && pnpm format:check && pnpm typecheck && pnpm build
```

## Clean-room rule

Never use SOLIDWORKS (or any proprietary CAD) code, binaries, file internals, names, icons or
screenshots. Build features from observed behaviour and public documentation only.
