# OpenForge Plan

Oct 10, 2026 · @dax

## Vision

OpenForge is a free, open-source parametric CAD desktop app that people use to design real parts.

Success in the first 3 months: people use it to design parts they actually print or machine.

## Target users

OpenForge targets all four groups below. Which one to win first is still open.

| Group | What they need from CAD |
| --- | --- |
| Hobbyists / 3D printing | Quick parts, STL export, easy to learn |
| Students | Free access that doesn't end at graduation, standard workflow |
| Small shops | Reliable dimensions, STEP export, 2D drawings for machining |
| Pro engineers | Stability, assemblies, drawings, file compatibility |

Open question: which group gets priority when their needs conflict?

## Positioning

The main reason to pick OpenForge over SolidWorks: it's free and open source. No license fees, ever.

## Scope and phases

OpenForge will implement every feature in the SolidWorks feature catalog (about 500 features in 25 areas), shipped in phases. Phases are ordered by how important each area is and how many people use it.

| Phase | Focus | What's in it |
| --- | --- | --- |
| 1 | Core part modeling (first public release) | Constraint-based 2D sketcher (lines, arcs, circles, rectangles, slots, polygons, splines, dimensions, relations, trim, extend, offset, convert, mirror, sketch fillet); reference planes, axes, points and coordinate systems; picking edges and faces; extrude and revolve with all end conditions and thin features; cuts; fillets and chamfers on selected edges; shell; Hole Wizard; linear, circular and mirror patterns; measure, mass properties, materials; units; feature tree with rollback; STEP, IGES and STL |
| 2 | Advanced part modeling | Sweep, loft, boundary, draft, rib, dome, wrap, threads and cosmetic threads, variable and face fillets, curves (helix, projected, split line, composite), 3D sketch, multibody tools (combine, split, move/copy body), equations, global variables, configurations |
| 3 | Assemblies | Insert components, standard, advanced and mechanical mates, move and rotate with collision detection, component patterns and mirror, interference detection, exploded views, assembly BOM, standard hardware library |
| 4 | Drawings | Sheets and title blocks, standard, projected, section, detail, auxiliary and break views, model dimensions, annotations, GD&T symbols, BOM, hole and revision tables, PDF and DXF/DWG export |
| 5 | Sheet metal and weldments | Flanges, hems, jogs, bends, flat patterns, forming tools, gauge tables; structural members, trim/extend, gussets, end caps, weld beads, cut lists |
| 6 | Surfaces, mold and direct editing | All surface features, mold tools (draft, undercut, parting line, core/cavity), move/delete/replace face, mesh and scan data |
| 7 | Quality, MBD and visuals | DimXpert, GD&T, 3D PDF and STEP 242 PMI; zebra, curvature, draft and thickness analysis; appearances, decals, scenes, rendering |
| 8 | Add-ins and automation | Basic stress analysis, motion studies, costing, routing, design checks, macros, scripting API, batch tasks |

Separately licensed SOLIDWORKS products (Simulation Premium, Flow, Plastics, Electrical, PDM, CAM, Composer) are out of scope until the core is complete.

Full feature list: [solidworks-feature-catalog.md](solidworks-feature-catalog.md).

Current implementation and remaining work: [Phase 1 status](phase-1-status.md).

Phase 2: [implementation plan](phase-2-plan.md) and [current status](phase-2-status.md).

AI direction, revised 2026-10-10: a conversational assistant built into the desktop,
with ChatGPT subscription, API-key and local-model connections. Setup and current
implementation limits are documented in [the assistant guide](assistant.md). See the [AI implementation plan](ai-implementation-plan.md)
for the shared live-session design, draft transactions and delivery sequence.
The existing MCP server remains an external automation interface; it is not the
built-in assistant and currently owns a separate modeling session. This AI track
does not change the CAD phase acceptance criteria above.

Open question: timeline per phase. The first release is targeted at 1–2 weeks; phases 2–8 are not yet scheduled.

## Tech stack

OpenForge is a clean-codebase restart: a Rust CAD core that calls OpenCASCADE, with an Electron + React desktop UI.

| Layer | Choice | Why |
| --- | --- | --- |
| Desktop shell | Electron | Bundled Chromium gives consistent 3D on Windows, macOS and Linux |
| UI | TypeScript + React | Feature tree, PropertyManager panels, ribbon, dialogs |
| 3D viewport | three.js (WebGL) | Fast rendering; custom renderer possible later |
| CAD core | Rust, running alongside the UI | Document model, feature tree, rebuilds, topological naming, file format, scripting API |
| Geometry kernel | OpenCASCADE (C++), called from Rust through a thin bridge | Only mature open-source B-rep kernel; solids, surfaces, booleans, fillets, STEP/IGES, hidden-line removal for drawings |
| Sketch solver | Native Rust solver in `of-sketch` | Damped least squares, constraint diagnostics and degrees-of-freedom analysis; retained by owner decision on 2026-10-10 |
| Assembly mate solver | OndselSolver (C++), via the same bridge | Open-source mate and kinematics solver |
| Simulation (later phases) | Gmsh + CalculiX | Open-source meshing and FEA |

How it fits together:

1. The UI process (Electron + React) handles everything the user sees and clicks.
2. It talks to the Rust core, which owns the document and rebuilds the model.
3. The core calls OpenCASCADE for geometry and solves sketches in Rust. OndselSolver is planned for assembly mates in Phase 3.
4. After each rebuild, the core sends meshes, edge lines and stable face/edge IDs back to the viewport.

Keeping face and edge references stable across edits (topological naming) is designed into the core from day one.

Rejected options:

- Pure-Rust kernel: Fornjot has shut down; truck/monstertruck lack shells, drafts, robust sweeps and drawing support.
- egui + wgpu (ArtCraft's stack) and Tauri: Electron + React chosen for UI polish and consistent 3D.

The earlier Electron + replicad prototype is a reference only.

Repo patterns adopted from ArtCraft:

- Crates workspace: the engine is split into small Rust crates, fully separate from the UI
- CLI + MCP server from day one, so scripts and AI assistants can build and edit models
- A web build is not planned for now

## Launch plan

The first release ships as a public GitHub repo in 1–2 weeks.

- What ships: the source code, public; people build and run it themselves
- Timeline: 1–2 weeks from 2026-10-10
- Feature scope: Phase 1 (core part modeling) of the phased roadmap

Announce on X and Reddit (r/SolidWorks, r/cad, r/3Dprinting, r/opensource).

## Team and contributors

The owner directs the project and Claude writes the code; outside contributors are welcome from launch.

| Role | Who | Responsibility |
| --- | --- | --- |
| Owner | Project owner | Product decisions, priorities, reviews, community |
| Engineering | Claude | Writes all code, tests and docs |
| Contributors | Open source community | Pull requests, bug reports, testing on real parts |

To make contributing easy from day one:

- CONTRIBUTING.md, issue and pull request templates, code of conduct
- AGENTS.md / CLAUDE.md describing the architecture and conventions
- Good-first-issue labels drawn from the feature catalog
- CI that builds and tests every pull request on Windows, macOS and Linux

## Name, license and legal

OpenForge's own code is dual-licensed Apache-2.0 / MIT and built as a clean-room reimplementation.

- License: Apache-2.0 / MIT for OpenForge code. OpenCASCADE (LGPL-2.1 with exception) and future OndselSolver integration keep their own licenses.
- Clean-room: features are built from how SolidWorks behaves and from public documentation, never from its code, binaries or file internals.
- Branding: no SolidWorks names, logos, icons or screenshots in the app or marketing.
- Name: OpenForge is a working name.

Name deferred: OpenForge stays the working name for now. Ruled out: anything with Forge (Autodesk Forge, ForgeCAD), Solid (SolidWorks), Iron (IronCAD), and Kerf (existing app). Shortlist so far: Partwright, Datumline. The final pick needs domain, GitHub and trademark checks.

## Risks and open questions

The biggest risk is scope: the full catalog is about 500 features, while Phase 1 is targeted at 1–2 weeks.

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Full-catalog scope vs timeline | Phases slip; launch feels unfinished | Ship Phase 1 as an alpha; publish the phased roadmap so users see what's coming |
| Topological naming (references breaking when the model changes) | Fillets and sketches fail after edits, the top complaint about open-source CAD | Stable face/edge IDs designed into the core from day one, with regression tests |
| OpenCASCADE robustness on fillets, shells and booleans | Features fail on real parts | Test corpus of real parts; clear error messages; contribute fixes upstream |
| Rust ↔ C++ bridge complexity | Slower builds, harder contributor setup | One thin bridge crate, prebuilt OpenCASCADE binaries in CI |
| Electron + Rust packaging on three platforms | Installers break on some systems | CI builds and smoke tests on Windows, macOS and Linux |
| Clean-room and trademark exposure | Legal pushback | No SolidWorks code, files, names or icons; final name checked before launch |
| Contributor load | Owner swamped by issues and pull requests | Templates, labels, AGENTS.md, clear roadmap |

Open questions:

- [ ] Final name (deferred)
- [ ] Timeline for phases 2–8
- [ ] Which user group gets priority when needs conflict
