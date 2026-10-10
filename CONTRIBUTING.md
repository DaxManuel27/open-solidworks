# Contributing to OpenForge

Thanks for helping build free, open-source CAD. Bug reports, tests on real parts and pull requests
are all welcome.

## Getting started

1. Read [docs/plan.md](docs/plan.md) for the architecture and roadmap, and [AGENTS.md](AGENTS.md)
   for code conventions.
2. Build from source as described in the [README](README.md#building-from-source).
3. Pick an issue. Issues labelled `good first issue` are a good place to start.

## Branches

- `main` is the release branch. It only moves when a roadmap phase is complete, and every commit on
  it is a working build.
- Each phase is built on its own branch (`phase-1`, `phase-2`, …), which is merged into `main` when
  the phase ships.
- Contributors branch from the current phase branch and open pull requests against it.

## Pull requests

- Keep each pull request to one change. Open an issue first for anything large.
- Add or update tests for behaviour you change.
- Make sure the CI checks pass locally (listed in the README).
- Describe what changed and how you tested it.

## Clean-room rule

OpenForge is a clean-room reimplementation. Build features from how CAD tools behave and from public
documentation only.

- Never copy code, binaries, file-format internals, icons or screenshots from SOLIDWORKS or any other
  proprietary CAD product.
- Don't use SOLIDWORKS names or branding in the app or its documentation, beyond saying what
  OpenForge is compatible with.

## Licensing

By contributing, you agree that your contributions are dual-licensed under Apache-2.0 and MIT, as
described in the [README](README.md#license), without any additional terms or conditions.

## Conduct

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).
