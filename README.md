# OpenForge

Free, open-source parametric CAD for designing real parts.

> **Status: pre-alpha.** The repository is being scaffolded. Nothing is usable yet.
> OpenForge is a working name.

OpenForge is a desktop app with a Rust CAD core on top of [OpenCASCADE](https://dev.opencascade.org/),
and an Electron + React user interface. See [docs/plan.md](docs/plan.md) for the vision, roadmap and
architecture.

## Roadmap

The first public release covers core part modeling: a constraint-based 2D sketcher, extrude and
revolve, cuts, fillets, chamfers, shell, holes, patterns, a feature tree with rollback, and STEP,
IGES and STL export. Later phases add advanced modeling, assemblies, drawings, sheet metal and more.
See the [phases](docs/plan.md#scope-and-phases).

## Repository layout

| Path                   | What it is                                                      |
| ---------------------- | --------------------------------------------------------------- |
| `apps/desktop`         | Electron + React desktop app                                    |
| `crates/openforge`     | Core binary: backend for the desktop app, CLI and MCP server    |
| `crates/of-protocol`   | JSON-RPC protocol shared by the app, CLI and MCP server         |
| `crates/of-model`      | Document model, feature tree, rebuild, stable face/edge IDs     |
| `crates/of-sketch`     | 2D sketch entities, constraints and solving                     |
| `crates/of-geom`       | Safe geometry wrappers: shapes, booleans, fillets, tessellation |
| `crates/of-io`         | STEP, IGES and STL import and export                            |
| `crates/of-kernel-sys` | The only C++ bridge: OpenCASCADE and PlaneGCS                   |

## Building from source

Requirements: [Rust](https://rustup.rs/) (stable), [Node.js](https://nodejs.org/) 22+ and
[pnpm](https://pnpm.io/) (`corepack enable`).

```sh
pnpm install
cargo build
pnpm dev        # launch the desktop app
```

Checks run in CI:

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
pnpm lint && pnpm format:check && pnpm typecheck && pnpm build
```

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[code of conduct](CODE_OF_CONDUCT.md).

## License

OpenForge's own code is dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at
your option. Third-party libraries (OpenCASCADE, PlaneGCS, OndselSolver) keep their own licenses.

OpenForge is not affiliated with, endorsed by or derived from Dassault Systèmes or SOLIDWORKS.
