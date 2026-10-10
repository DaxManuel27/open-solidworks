# Built-in assistant

Open **AI Assistant → Settings** in the desktop. Choose one connection:

- **ChatGPT subscription**: choose **Continue with ChatGPT**, authorize OpenForge in
  your browser, choose a discovered model, then **Save & Test connection**. Eligible
  Plus and Pro accounts can grant plan usage; limits and credits are managed through
  **Manage usage**. Identity-only consent does not enable inference.
- **API key (Anthropic)**: enter an API key and a tool-capable model ID. Optionally
  remember the key using the operating system's encrypted credential storage.
- **Local model (Ollama)**: start Ollama, install a model that supports tools, and enter
  its model ID and loopback endpoint (default `http://127.0.0.1:11434`). OpenForge does
  not download models or silently switch to a cloud provider.

**Make changes** builds a private draft and applies successful work as one undoable
change. **Preview only** displays the draft until **Apply preview** or **Discard
preview**. **Ask / explain** exposes inspection tools only. Stop cancels network
work; a native geometry operation already executing finishes before its result can
be discarded. Changes to the source document invalidate pending tasks/previews.
Finish an open manual editor before asking the assistant to mutate geometry.

Selection and the active configuration accompany each request; uncheck **Use
selection** to omit selected references. The model can inspect features, topology,
sketches, parameters, equations and configurations, and create ordinary editable
features. Chat stays in memory and is not part of the saved `.ofp` document.
The Undo button uses the normal document history. In Make changes mode, “Undo the
last change” and “Apply preview” also work directly in the composer. Save/export
use the existing CAD controls.

Cloud connections send bounded part context and tool results to the selected
provider. Mesh payloads, file paths, saved credentials and arbitrary files are not
included. API keys and OAuth tokens stay in the main process, outside the renderer.
Saved secrets use Electron `safeStorage`; if protected storage is unavailable,
credentials remain session-only. ChatGPT connections retain their installation host
ID and separate client registrations through sign-out. Sign-out attempts refresh-token
revocation and reports if remote revocation could not be confirmed.

The coordinator permits at most 40 model turns, 100 tool calls, 100,000 reported
tokens and five minutes per task. Each response has a 90-second timeout and a 2 MB
stream limit. Drafts expire after ten minutes. Geometry validation establishes that
features rebuild; it cannot establish that a model interpreted every instruction
correctly. Check dimensions and the resulting part.

## Verification

- `cargo test`: core draft isolation, guarded/idempotent commits, invalid geometry,
  document generations and one-step undo, alongside the CAD regression suite.
- `node scripts/assistant-test.mjs`: simulated Anthropic/Ollama/Responses streams,
  completion guards, encrypted reasoning history, signed JWT validation, OAuth
  callback state/PKCE flow, account isolation, rotating refresh and revocation.
  This test binds a loopback port but makes no external requests.
- Build the desktop, then `node apps/desktop/node_modules/electron/cli.js
  scripts/desktop-smoke.mjs`: real renderer, preload, coordinator and Rust geometry,
  using a deterministic local model fixture. Set `OPENFORGE_SMOKE_ASSISTANT_ONLY=1`
  for the assistant subset.

A real ChatGPT authorization and inference call, a live Anthropic task, and a live
Ollama modeling benchmark still require configured accounts/models. The fixtures
verify integration behavior, not model quality. Remaining design work includes
richer per-reference context chips/highlights, a user-facing operation record, fully
generated nested tool schemas and the broader model-quality regression corpus.

## ChatGPT integration contract

This uses OpenAI's documented public-client flow with browser authorization,
loopback callback, PKCE, state, nonce and verified OpenID identity. It does not read
credentials from ChatGPT or Codex installations. Responses use the public API,
`store: false`, `stream: true`, client-held history and namespaced CAD tools.

Official references (checked 2026-10-10):

- [Open-source integration guide](https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt)
- [Registration and sign-in](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
- [Accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)
- [Models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
- [Preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
