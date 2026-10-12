// OpenForge-Bench harness (docs/mcp-plan.md, M0). Runs the real assistant
// coordinator and provider code against `openforge serve`, then asks the core
// to check the finished part (`openforge check`). No model judges the result.
//
//   node scripts/bench.mjs fixtures                    rebuild bench/fixtures/*.ofp
//   node scripts/bench.mjs replay [label] [task-id…]   replay bench/transcripts/<label>
//   node scripts/bench.mjs live --provider anthropic|ollama --model M [--label L] [task-id…]
//
// Replay serves recorded tool calls from a local Ollama-compatible endpoint, so
// the provider, coordinator, draft and commit paths are exercised for real.
// Live mode needs ANTHROPIC_API_KEY or a running Ollama endpoint
// (OPENFORGE_OLLAMA, default http://127.0.0.1:11434) and records transcripts.
import { spawn, spawnSync } from 'node:child_process'
import { createServer } from 'node:http'
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { pathToFileURL } from 'node:url'
import ts from 'typescript'

const root = resolve(import.meta.dirname, '..')
const bench = join(root, 'bench')
const exe =
  process.env.OPENFORGE_CORE ??
  join(root, 'target/debug', process.platform === 'win32' ? 'openforge.exe' : 'openforge')

class Core {
  constructor() {
    this.child = spawn(exe, ['serve'], { stdio: ['pipe', 'pipe', 'inherit'] })
    this.pending = new Map()
    this.next = 1
    createInterface({ input: this.child.stdout }).on('line', (line) => {
      const msg = JSON.parse(line)
      const p = this.pending.get(msg.id)
      this.pending.delete(msg.id)
      if (msg.error) p.reject(new Error(msg.error.message))
      else p.resolve(msg.result)
    })
  }
  call(method, params = {}) {
    const id = this.next++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
    })
  }
  close() {
    this.child.stdin.end()
  }
}

let compiled
/** The real coordinator; its connection comes from `globalThis.benchProvider`. */
async function compileAssistant(dir) {
  if (compiled) return compiled
  const compile = (name, rewrite = (s) => s) => {
    const source = readFileSync(join(root, `apps/desktop/src/main/assistant/${name}.ts`), 'utf8')
    const code = ts.transpileModule(source, {
      compilerOptions: { target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext },
    }).outputText
    writeFileSync(join(dir, `${name}.mjs`), rewrite(code))
  }
  compile('provider')
  // The Electron settings store is replaced by the harness's fixed connection.
  writeFileSync(
    join(dir, 'settings.mjs'),
    `export const connection = () => structuredClone(globalThis.benchProvider)
export const connectionKey = async (c) => c.key\n`,
  )
  compile('coordinator', (s) =>
    s
      .replace("from './provider'", "from './provider.mjs'")
      .replace("from './settings'", "from './settings.mjs'"),
  )
  compiled = (await import(pathToFileURL(join(dir, 'coordinator.mjs')).href)).AssistantCoordinator
  return compiled
}

/** An Ollama-compatible endpoint that answers with the recorded steps in order. */
async function replayServer(steps) {
  const queue = [...steps]
  const server = createServer(async (req, res) => {
    for await (const chunk of req) void chunk
    const step = queue.shift()
    const calls = step?.calls ?? []
    res.writeHead(200, { 'content-type': 'application/x-ndjson' })
    res.end(
      JSON.stringify({
        message: {
          role: 'assistant',
          content: step?.text ?? (step ? '' : 'Transcript ended.'),
          tool_calls: calls.map((c) => ({ function: { name: c.name, arguments: c.args } })),
        },
        done: true,
        prompt_eval_count: 0,
        eval_count: 0,
      }) + '\n',
    )
  })
  await new Promise((r) => server.listen(0, '127.0.0.1', r))
  return { server, endpoint: `http://127.0.0.1:${server.address().port}` }
}

function tasks(filter) {
  const dir = join(bench, 'tasks')
  return readdirSync(dir)
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => ({ file: join(dir, f), ...JSON.parse(readFileSync(join(dir, f), 'utf8')) }))
    .filter((t) => !filter.length || filter.includes(t.id))
}

async function runTask(task, provider, temp) {
  const core = new Core()
  const tools = new Map()
  const log = []
  try {
    if (task.start) await core.call('doc.open', { path: resolve(bench, 'tasks', task.start) })
    for (const t of (await core.call('assistant.tools')).tools) tools.set(t.method, t.name)
    const existing = Object.fromEntries(
      (await core.call('doc.info')).features.filter((f) => f.error).map((f) => [f.id, f.error]),
    )
    // Record every tool call the coordinator forwards, with its outcome. The
    // coordinator's own context reads happen before it loads the catalog.
    let modelCalls = false
    const call = async (method, params) => {
      if (method === 'assistant.tools') modelCalls = true
      const forwarded = method === 'draft.apply' ? params.method : method
      const tool = modelCalls ? tools.get(forwarded) : undefined
      const entry = tool ? { tool, args: method === 'draft.apply' ? params.params : params } : null
      try {
        const result = await core.call(method, params)
        if (entry && 'ok' in (result ?? {})) {
          // A tool result envelope (draft calls).
          entry.ok = result.ok
          if (result.error) entry.error = `${result.error.code}: ${result.error.message}`
          entry.new_errors = result.feature_errors
          entry.existing_errors = Object.keys(result.existing_errors).length
          log.push(entry)
        } else if (entry) {
          // A raw core result (read-only calls); errors are not split by age.
          const errors = Object.entries(result?.feature_errors ?? {})
          entry.new_errors = Object.fromEntries(errors.filter(([id, e]) => existing[id] !== e))
          entry.existing_errors = errors.length - Object.keys(entry.new_errors).length
          log.push(entry)
        }
        return result
      } catch (e) {
        if (entry) log.push({ ...entry, error: e.message })
        throw e
      }
    }
    globalThis.benchProvider = provider
    const Coordinator = await compileAssistant(temp)
    const events = []
    const coordinator = new Coordinator(call, (e) => {
      if (e.type !== 'text') events.push(e)
    })
    const info = await core.call('doc.info')
    await coordinator.run({
      prompt: task.prompt,
      mode: task.mode ?? 'edit',
      generation: info.generation,
      revision: info.revision,
      selection: task.selection ?? [],
      view: 'isometric',
      editing: false,
    })
    const final = events.findLast((e) => ['committed', 'done', 'error', 'preview'].includes(e.type))
    const part = join(temp, `${task.id}.ofp`)
    await core.call('doc.save', { path: part })
    const check = spawnSync(exe, ['check', task.file, part], { encoding: 'utf8' })
    const report = JSON.parse(check.stdout || '{"passed":false,"checks":[]}')
    // A change must be committed; a question or refusal is a plain reply.
    const want = task.expect.outcome === 'change' ? 'committed' : 'done'
    const outcome = final?.type ?? 'none'
    report.checks.unshift({
      name: 'outcome',
      ok: outcome === want,
      detail: `${outcome} (want ${want})`,
    })
    report.passed &&= outcome === want
    return {
      id: task.id,
      outcome,
      message: final?.text ?? '',
      calls: log,
      report,
    }
  } finally {
    core.close()
  }
}

function summarize(results) {
  for (const r of results) {
    const failed = r.calls.filter(
      (c) => c.ok === false || c.error || Object.keys(c.new_errors ?? {}).length,
    )
    const checks = r.report.checks.filter((c) => !c.ok).map((c) => `${c.name}: ${c.detail}`)
    console.log(
      `${r.report.passed ? 'PASS' : 'FAIL'} ${r.id}: ${r.calls.length} tool calls, ` +
        `${failed.length} failed, outcome ${r.outcome}${r.message ? ` (${r.message})` : ''}`,
    )
    for (const c of failed) console.log(`    ${c.tool}: ${c.error ?? JSON.stringify(c.new_errors)}`)
    for (const c of checks) console.log(`    check ${c}`)
  }
  const passed = results.filter((r) => r.report.passed).length
  console.log(`${passed}/${results.length} tasks passed`)
  return passed === results.length
}

async function replay(label, filter) {
  const temp = mkdtempSync(join(tmpdir(), 'openforge-bench-'))
  const results = []
  try {
    for (const task of tasks(filter)) {
      const transcript = JSON.parse(
        readFileSync(join(bench, 'transcripts', label, `${task.id}.json`), 'utf8'),
      )
      const { server, endpoint } = await replayServer(transcript.steps)
      try {
        const provider = { settings: { provider: 'ollama', model: 'replay', endpoint }, key: '' }
        results.push({ ...(await runTask(task, provider, temp)), steps: transcript.steps.length })
      } finally {
        server.close()
      }
    }
  } finally {
    rmSync(temp, { recursive: true, force: true })
  }
  writeFileSync(join(bench, 'results', `${label}.json`), JSON.stringify(results, null, 2) + '\n')
  return summarize(results)
}

async function live(args, filter) {
  const option = (name, fallback) => {
    const i = args.indexOf(`--${name}`)
    return i >= 0 ? args.splice(i, 2)[1] : fallback
  }
  const kind = option('provider')
  const model = option('model')
  const label = option('label', `${kind}-${model}`)
  if (!['anthropic', 'ollama'].includes(kind) || !model)
    throw new Error('live needs --provider anthropic|ollama --model <id>')
  const key = kind === 'anthropic' ? process.env.ANTHROPIC_API_KEY : ''
  if (kind === 'anthropic' && !key) throw new Error('Set ANTHROPIC_API_KEY.')
  const endpoint = process.env.OPENFORGE_OLLAMA ?? 'http://127.0.0.1:11434'
  const provider = { settings: { provider: kind, model, endpoint }, key }
  const temp = mkdtempSync(join(tmpdir(), 'openforge-bench-'))
  const out = join(bench, 'transcripts', label)
  mkdirSync(out, { recursive: true })
  const results = []
  try {
    for (const task of tasks([...filter, ...args])) {
      const result = await runTask(task, provider, temp)
      results.push(result)
      // Recorded per forwarded tool call; replay sends one call per model step.
      const steps = result.calls.map((c) => ({ calls: [{ name: c.tool, args: c.args }] }))
      if (result.outcome === 'committed' || result.outcome === 'preview')
        steps.push({
          calls: [
            {
              name: 'finish_task',
              args: { summary: result.message, preview_only: result.outcome === 'preview' },
            },
          ],
        })
      else steps.push({ text: result.message })
      writeFileSync(
        join(out, `${task.id}.json`),
        JSON.stringify({ task: task.id, source: label, steps }, null, 2) + '\n',
      )
    }
  } finally {
    rmSync(temp, { recursive: true, force: true })
  }
  writeFileSync(join(bench, 'results', `${label}.json`), JSON.stringify(results, null, 2) + '\n')
  return summarize(results)
}

/** Build the starting parts. Each is ordinary editable history. */
async function fixtures() {
  const dir = join(bench, 'fixtures')
  mkdirSync(dir, { recursive: true })
  const box = async (core, w, h, depth) => {
    const sketch = (await core.call('sketch.new', { plane: { type: 'front_plane' } })).id
    const rect = await core.call('sketch.edit', {
      id: sketch,
      op: { op: 'rectangle', a: [0, 0], b: [w, h] },
    })
    // `created` lists the four corners (from `a`), then bottom, right, top and left lines.
    const [corner, , , , bottom, right] = rect.created
    const ops = [
      { op: 'constraint', constraint: { type: 'coincident', a: corner, b: 0 } },
      { op: 'dimension', constraint: { type: 'length', line: bottom, value: 0 }, value: w },
      { op: 'dimension', constraint: { type: 'length', line: right, value: 0 }, value: h },
    ]
    for (const op of ops) await core.call('sketch.edit', { id: sketch, op })
    await core.call('sketch.close')
    await core.call('feature.add', {
      feature: { type: 'extrude', sketch, end1: { type: 'blind', depth } },
    })
  }
  const parts = {
    'block.ofp': (core) => box(core, 40, 30, 20),
    'plate.ofp': (core) => box(core, 60, 40, 5),
    'plate-failed-fillet.ofp': async (core) => {
      await box(core, 60, 40, 5)
      const topo = await core.call('doc.topology')
      // The long edge on the top face (z = 5, y = 0): a 6 mm round is thicker than the plate.
      const edge = topo.edges.find(
        (e) => e.start[2] === 5 && e.end[2] === 5 && e.start[1] === 0 && e.end[1] === 0,
      )
      const r = await core.call('feature.add', {
        feature: { type: 'fillet', edges: [edge.ref], radius: 6 },
      })
      if (!r.features.at(-1).error) throw new Error('fixture fillet unexpectedly succeeded')
    },
  }
  for (const [name, build] of Object.entries(parts)) {
    const core = new Core()
    try {
      await build(core)
      await core.call('doc.save', { path: join(dir, name) })
      console.log(`wrote bench/fixtures/${name}`)
    } finally {
      core.close()
    }
  }
}

const [command = 'replay', ...rest] = process.argv.slice(2)
let ok = true
if (command === 'fixtures') await fixtures()
else if (command === 'replay') ok = await replay(rest[0] ?? 'reference', rest.slice(1))
else if (command === 'live') ok = await live(rest, [])
else throw new Error(`unknown command ${command}`)
process.exitCode = ok ? 0 : 1
