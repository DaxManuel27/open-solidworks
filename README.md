# OpenForge

Free, open-source parametric CAD for designing real parts.

> **Status: Phase 1 source alpha.** Core part modeling is implemented and locally tested.
> See [Phase 1 status](docs/phase-1-status.md) for coverage and release validation.
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
| `crates/of-kernel-sys` | The only C++ bridge: OpenCASCADE                                |

## Building from source

Requirements: Rust stable, Node.js 24+, pnpm (the version in `package.json`), a C++17
compiler, and OpenCASCADE 7.9. On macOS install the Xcode command-line tools; on
Windows install Visual Studio Build Tools with Desktop development with C++; on
Linux install your distribution's C++ build tools.

On macOS, install the geometry kernel with Homebrew:

```sh
brew install opencascade
```

Alternatively, on any supported platform, use the same Conda environment as CI:

```sh
conda env create -f environment.yml
conda activate openforge
```

Keep that environment active when building and running. On Linux, also expose its
shared libraries to the runtime loader:

```sh
export LD_LIBRARY_PATH="$CONDA_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```

The build discovers OpenCASCADE from the active Conda environment, Homebrew, or
system paths. Custom installs can set `OCCT_INCLUDE_DIR` (containing `Standard.hxx`)
and `OCCT_LIB_DIR`. Their shared libraries must also be on the OS loader path.

Run these commands from the repository root (the directory containing `Cargo.toml`):

```sh
corepack enable
pnpm install --frozen-lockfile
cargo build
pnpm dev
```

If Corepack is unavailable, install it with `npm install -g corepack` first. If its
shims are unavailable, `corepack pnpm --filter @openforge/desktop dev` launches the
app directly. `OPENFORGE_CORE` can select a particular core executable; otherwise
the app finds the packaged binary or a local Cargo build.

To make a first part, double-click **Front Plane**, draw a **Corner Rectangle**, then
choose **Features → Extruded Boss/Base**. Enter a depth and press the green check.
Use **File → Save** for an editable `.ofp` document and **File → Export** for STEP,
IGES or STL. The CLI is also available with `cargo run -- --help`.

Validation:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
pnpm lint
pnpm format:check
pnpm typecheck
pnpm build
pnpm --filter @openforge/desktop smoke
```

The desktop smoke test launches the actual app with an isolated temporary profile,
draws and extrudes a sketch, checks undo/redo and save/reopen, exports all three
formats, and builds a revolve with independent directions. Set
`OPENFORGE_SMOKE_SCREENSHOT` to save a screenshot. Headless Linux CI uses Xvfb.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[code of conduct](CODE_OF_CONDUCT.md).

## License

OpenForge's own code is dual-licensed under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at
your option. Third-party libraries keep their own licenses; the sketch solver is native Rust.

OpenForge is not affiliated with, endorsed by or derived from Dassault Systèmes or SOLIDWORKS.
