// Sheet metal and weldment performance through the native core (Phase 5 budgets).
// Build first, then:
//   node scripts/phase-5-performance.mjs                measure and print
//   node scripts/phase-5-performance.mjs --set-budgets  measure 3 runs, write budgets
//   node scripts/phase-5-performance.mjs --check        measure, compare with budgets
// OPENFORGE_CORE may select a release binary. OPENFORGE_BUDGET_SCALE multiplies
// the budgets for slower machines (default 1). OPENFORGE_PERF_OUTPUT keeps the
// fixture parts and the flat DXF in that directory.
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
const budgetsFile = join(root, 'docs/phase-5-budgets.json')
const scale = Number(process.env.OPENFORGE_BUDGET_SCALE ?? 1)

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

async function time(f) {
  const t = performance.now()
  await f()
  return performance.now() - t
}

const T = 1.5
const sheet = { thickness: T, radius: 1.5, allowance: { type: 'k_factor', k: 0.44 } }
const near = (a, b) => Math.abs(a - b) < 1e-6

/**
 * The corpus enclosure through the core: a 200 × 120 base, four 40 mm
 * flanges, open hems on the long flanges, circular corner reliefs and three
 * louvers in the floor.
 */
async function enclosure(core, path) {
  await core.call('doc.new')
  const { id } = await core.call('sketch.new', { plane: { type: 'front_plane' } })
  await core.call('sketch.edit', { id, op: { op: 'rectangle', a: [0, 0], b: [200, 120] } })
  await core.call('sketch.close')
  await core.call('feature.add', { feature: { type: 'base_flange', sketch: id, sheet } })
  let topo = await core.call('doc.topology')
  const top = topo.edges
    .filter((e) => e.curve === 'line' && near(e.start[2], T) && near(e.end[2], T))
    .map((e) => e.ref)
  await core.call('feature.add', {
    feature: { type: 'edge_flange', edges: top, length: { type: 'blind', length: 40 }, gap: 0.5 },
  })
  topo = await core.call('doc.topology')
  const zmax = Math.max(...topo.edges.map((e) => Math.max(e.start[2], e.end[2])))
  const hems = topo.edges
    .filter(
      (e) =>
        e.curve === 'line' &&
        near(e.start[2], zmax) &&
        near(e.end[2], zmax) &&
        near(e.start[1], e.end[1]) &&
        (near(e.start[1], T) || near(e.start[1], 120 - T)),
    )
    .map((e) => e.ref)
  await core.call('feature.add', {
    feature: { type: 'hem', edges: hems, kind: { type: 'open', gap: 1 }, length: 8, gap: 0.5 },
  })
  await core.call('feature.add', { feature: { type: 'corner_relief', kind: 'circular' } })
  const pts = await core.call('sketch.new', { plane: { type: 'front_plane' } })
  for (const x of [60, 100, 140])
    await core.call('sketch.edit', { id: pts.id, op: { op: 'add_point', at: [x, 60] } })
  await core.call('sketch.close')
  topo = await core.call('doc.topology')
  const floor = topo.faces.find((f) => f.normal && f.normal[2] > 0.999 && near(f.centroid[2], T))
  const r = await core.call('feature.add', {
    feature: {
      type: 'form_tool',
      shape: { type: 'louver', length: 24, width: 5, height: 3 },
      face: floor.ref,
      sketch: pts.id,
      rotation: Math.PI / 2,
      punch_id: 'LV-24',
    },
  })
  for (const f of r.features) if (f.error) throw new Error(`${f.name}: ${f.error}`)
  await core.call('doc.save', { path })
}

/**
 * A frame of 40 members: a mitered 2000 × 1000 SHS 40x40x3 rectangle and 36
 * RHS 60x40x3 cross members, then the parts' cut list.
 */
async function frame(core, path) {
  await core.call('doc.new')
  await core.call('feature.add', { feature: { type: 'weldment' } })
  const { id } = await core.call('sketch.new', { plane: { type: 'front_plane' } })
  const rect = await core.call('sketch.edit', {
    id,
    op: { op: 'rectangle', a: [0, 0], b: [2000, 1000] },
  })
  const sides = rect.created.slice(4, 8)
  const cross = []
  for (let k = 1; k <= 36; k++) {
    const x = (2000 * k) / 37
    const r = await core.call('sketch.edit', {
      id,
      op: { op: 'add_line', a: { at: [x, 0] }, b: { at: [x, 1000] } },
    })
    cross.push(r.created[0])
  }
  await core.call('sketch.close')
  const seg = (entity) => ({ type: 'sketch_entity', sketch: id, entity })
  await core.call('feature.add', {
    feature: {
      type: 'structural_member',
      profile: { type: 'standard', family: 'rhs', size: 'SHS 40x40x3' },
      groups: [{ segments: sides.map(seg) }],
    },
  })
  const r = await core.call('feature.add', {
    feature: {
      type: 'structural_member',
      profile: { type: 'standard', family: 'rhs', size: 'RHS 60x40x3' },
      groups: cross.map((c) => ({ segments: [seg(c)] })),
    },
  })
  for (const f of r.features) if (f.error) throw new Error(`${f.name}: ${f.error}`)
  await core.call('doc.save', { path })
}

async function measure(dir) {
  const kept = process.env.OPENFORGE_PERF_OUTPUT ?? dir
  const part = join(kept, 'enclosure.ofp')
  let core = new Core()
  await enclosure(core, part)
  core.stop()
  const out = {}
  core = new Core()
  out.enclosure_rebuild_ms = await time(() => core.call('doc.open', { path: part }))
  out.flatten_ms = await time(() => core.call('feature.add', { feature: { type: 'flat_pattern' } }))
  out.flat_dxf_ms = await time(() =>
    core.call('export', { path: join(kept, 'enclosure-flat.dxf') }),
  )
  out.sheet_metal_info_ms = await time(() => core.call('doc.sheet_metal'))
  const info = await core.call('doc.sheet_metal')
  if (info.bodies[0].error) throw new Error(info.bodies[0].error)
  if (info.bodies[0].properties.forms !== 3) throw new Error('the louvers are missing')
  core.stop()
  // The weldment frame: rebuild on open, then the cut list.
  const framePart = join(kept, 'frame.ofp')
  core = new Core()
  await frame(core, framePart)
  core.stop()
  core = new Core()
  out.frame_rebuild_ms = await time(() => core.call('doc.open', { path: framePart }))
  out.cut_list_ms = await time(() => core.call('doc.cut_list'))
  const list = await core.call('doc.cut_list')
  const count = list.items.reduce((n, i) => n + i.quantity, 0)
  if (count !== 40) throw new Error(`the frame lists ${count} members, not 40`)
  core.stop()
  return out
}
const dir = mkdtempSync(join(tmpdir(), 'openforge-phase5-perf-'))
try {
  const runs = settingBudgets ? 3 : 1
  const results = []
  for (let i = 0; i < runs; i++) results.push(await measure(dir))
  const machine = `${cpus()[0]?.model ?? 'unknown'}, ${platform()} ${arch()}`
  console.log(`Phase 5 performance (sheet metal enclosure, 40-member frame), ${machine}`)
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
          fixture:
            'Enclosure 200 x 120 x 40 of 1.5 mm steel: base flange, four edge flanges, two open hems, circular corner reliefs, three louvers (rebuild on open, flatten, flat DXF, sheet metal properties); weldment frame of 40 members, a mitered SHS rectangle and 36 RHS cross members (rebuild on open, cut list)',
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
