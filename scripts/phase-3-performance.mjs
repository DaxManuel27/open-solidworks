// Assembly performance through the native core (Phase 3 budgets).
// Build first, then:
//   node scripts/phase-3-performance.mjs                measure and print
//   node scripts/phase-3-performance.mjs --set-budgets  measure 3 runs, write budgets
//   node scripts/phase-3-performance.mjs --check        measure, compare with budgets
// OPENFORGE_CORE may select a release binary. OPENFORGE_BUDGET_SCALE multiplies
// the budgets for slower machines (default 1).
import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { arch, cpus, platform, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { performance } from 'node:perf_hooks'

const root = resolve(import.meta.dirname, '..')
const binary =
  process.env.OPENFORGE_CORE ??
  join(root, 'target/debug', process.platform === 'win32' ? 'openforge.exe' : 'openforge')
const argv = process.argv.slice(2)
const checking = argv.includes('--check')
const settingBudgets = argv.includes('--set-budgets')
const budgetsFile = join(root, 'docs/phase-3-budgets.json')
const scale = Number(process.env.OPENFORGE_BUDGET_SCALE ?? 1)
const COMPONENTS = 60
const SAMPLES = 5

class Core {
  constructor() {
    this.next = 0
    this.pending = new Map()
    this.child = spawn(binary, ['serve'], { stdio: ['pipe', 'pipe', 'inherit'] })
    createInterface({ input: this.child.stdout }).on('line', (line) => {
      const r = JSON.parse(line)
      const p = this.pending.get(r.id)
      this.pending.delete(r.id)
      if (r.error) p.reject(new Error(r.error.message))
      else p.resolve(r.result)
    })
  }
  call(method, params = {}) {
    const id = ++this.next
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
    })
  }
  stop() {
    this.child.kill()
  }
}

const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)]
async function time(f) {
  const t = performance.now()
  await f()
  return performance.now() - t
}

/** A stack of blocks, each seated on the one below; the top one slides. */
async function fixture(dir) {
  const core = new Core()
  const block = join(dir, 'block.ofp')
  await core.call('doc.new')
  const sketch = (await core.call('sketch.new', { plane: { type: 'front_plane' } })).id
  await core.call('sketch.edit', { id: sketch, op: { op: 'rectangle', a: [0, 0], b: [10, 10] } })
  await core.call('sketch.close')
  await core.call('feature.add', {
    feature: { type: 'extrude', sketch, end1: { type: 'blind', depth: 10 } },
  })
  await core.call('doc.save', { path: block })
  await core.call('asm.new')
  const keys = []
  for (let i = 0; i < COMPONENTS; i++) {
    const r = await core.call('asm.insert', {
      path: block,
      transform: { translation: [i * 0.3, -i * 0.2, i * 10.5], rotation: [0, 0, 0, 1] },
    })
    keys.push(r.key)
  }
  // Face references by geometry from the first component's topology.
  const faces = (await core.call('asm.topology', { component: [keys[0]] })).faces
  const face = (n, at) =>
    faces.find(
      (f) =>
        f.surface === 'plane' &&
        f.normal.every((v, k) => Math.abs(v - n[k]) < 1e-9) &&
        Math.abs(f.centroid[n.findIndex((v) => v !== 0)] - at) < 1e-9,
    ).ref.entity
  const top = face([0, 0, 1], 10)
  const bottom = face([0, 0, -1], 0)
  const left = face([-1, 0, 0], 0)
  const front = face([0, -1, 0], 0)
  const r = (key, entity) => ({ component: [key], entity })
  for (let i = 1; i < COMPONENTS; i++) {
    const [a, b] = [keys[i - 1], keys[i]]
    await core.call('asm.mate.add', {
      mate: { type: 'coincident', a: r(a, top), b: r(b, bottom) },
    })
    if (i < COMPONENTS - 1) {
      await core.call('asm.mate.add', {
        mate: { type: 'coincident', a: r(a, left), b: r(b, left) },
      })
      await core.call('asm.mate.add', {
        mate: { type: 'coincident', a: r(a, front), b: r(b, front) },
      })
    }
  }
  const file = join(dir, 'stack.ofa')
  await core.call('doc.save', { path: file })
  core.stop()
  return { file, top: keys[COMPONENTS - 1] }
}

async function measure(dir) {
  const { file, top } = await fixture(dir)
  const core = new Core()
  const out = {}
  out.open_ms = await time(() => core.call('doc.open', { path: file }))
  const samples = async (f) => {
    const xs = []
    for (let i = 0; i < SAMPLES; i++) xs.push(await time(() => f(i)))
    return median(xs)
  }
  out.full_solve_ms = await samples(() => core.call('asm.reload'))
  const drag = (i, collision) =>
    core.call('asm.move', {
      component: [top],
      motion: { mode: 'delta', translation: [i % 2 ? 1 : -1, 0, 0] },
      phase: 'move',
      ...(collision ? { collision: {} } : {}),
    })
  out.drag_step_ms = await samples((i) => drag(i, false))
  out.collision_drag_step_ms = await samples((i) => drag(i, true))
  out.interference_ms = await samples(() => core.call('asm.interference', {}))
  out.bom_ms = await samples(() => core.call('asm.bom', { type: 'indented' }))
  core.stop()
  return out
}

const dir = mkdtempSync(join(tmpdir(), 'openforge-phase3-perf-'))
try {
  const runs = settingBudgets ? 3 : 1
  const results = []
  for (let i = 0; i < runs; i++) results.push(await measure(dir))
  const machine = `${cpus()[0]?.model ?? 'unknown'}, ${platform()} ${arch()}`
  console.log(
    `Phase 3 performance (${COMPONENTS} components, ${3 * COMPONENTS - 4} mates), ${machine}`,
  )
  for (const [k, v] of Object.entries(results[0])) console.log(`  ${k.padEnd(24)} ${v.toFixed(1)}`)
  if (settingBudgets) {
    const budgets = {}
    for (const k of Object.keys(results[0])) {
      const worst = Math.max(...results.map((r) => r[k]))
      budgets[k] = Math.max(50, Math.ceil(worst * 3))
    }
    writeFileSync(
      budgetsFile,
      JSON.stringify(
        {
          recorded_at_utc: new Date().toISOString(),
          machine,
          build_profile: binary.includes('release') ? 'release' : 'debug',
          fixture: `${COMPONENTS} stacked blocks, ${3 * COMPONENTS - 4} mates, top block sliding`,
          rule: 'Each budget is three times the slowest of three runs (at least 50 ms). Scale with OPENFORGE_BUDGET_SCALE on slower machines.',
          budgets,
          observed: results,
        },
        null,
        2,
      ) + '\n',
    )
    console.log(`wrote ${budgetsFile}`)
  }
  if (checking) {
    if (!existsSync(budgetsFile)) throw new Error('no budgets recorded; run with --set-budgets')
    const { budgets } = JSON.parse(readFileSync(budgetsFile, 'utf8'))
    const over = Object.entries(budgets).filter(([k, b]) => results[0][k] > b * scale)
    for (const [k, b] of over)
      console.error(
        `over budget: ${k} ${results[0][k].toFixed(1)} ms > ${(b * scale).toFixed(0)} ms`,
      )
    if (over.length) process.exit(1)
    console.log('within budgets')
  }
} finally {
  rmSync(dir, { recursive: true, force: true })
}
