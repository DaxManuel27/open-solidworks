// Drawing performance through the native core (Phase 4 budgets).
// Build first, then:
//   node scripts/phase-4-performance.mjs                measure and print
//   node scripts/phase-4-performance.mjs --set-budgets  measure 3 runs, write budgets
//   node scripts/phase-4-performance.mjs --check        measure, compare with budgets
// OPENFORGE_CORE may select a release binary. OPENFORGE_BUDGET_SCALE multiplies
// the budgets for slower machines (default 1). OPENFORGE_PERF_OUTPUT keeps the
// exported PDF and DXF in that directory.
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
const budgetsFile = join(root, 'docs/phase-4-budgets.json')
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

/** A part from one front-plane sketch extruded `depth`, with more steps after. */
async function part(core, path, ops, depth, more = async () => {}) {
  await core.call('doc.new')
  const { id } = await core.call('sketch.new', { plane: { type: 'front_plane' } })
  for (const op of ops) await core.call('sketch.edit', { id, op })
  await core.call('sketch.close')
  await core.call('feature.add', {
    feature: { type: 'extrude', sketch: id, end1: { type: 'blind', depth } },
  })
  await more()
  await core.call('doc.save', { path })
}

/**
 * The gearbox stand-in: a 240 × 120 × 90 housing shelled 6 mm open at the
 * top, with three shaft bores and 16 bolt holes in its rim; three shafts,
 * three gear blanks and 16 bolts placed in it (23 components).
 */
async function fixture(dir) {
  const core = new Core()
  const housing = join(dir, 'housing.ofp')
  await part(core, housing, [{ op: 'rectangle', a: [0, 0], b: [240, 120] }], 90, async () => {
    const topo = await core.call('doc.topology')
    const top = topo.faces.find((f) => f.normal && Math.abs(f.normal[2] - 1) < 1e-9).ref
    await core.call('feature.add', { feature: { type: 'shell', faces: [top], thickness: 6 } })
    // Bores along y through the long walls, seen from the front of the box.
    const { id: bores } = await core.call('sketch.new', { plane: { type: 'top_plane' } })
    for (const [x, r] of [
      [50, 15],
      [120, 12],
      [190, 10],
    ])
      await core.call('sketch.edit', {
        id: bores,
        op: { op: 'add_circle', center: { at: [x, -45] }, radius: r },
      })
    await core.call('sketch.close')
    await core.call('feature.add', {
      feature: {
        type: 'extrude',
        sketch: bores,
        cut: true,
        end1: { type: 'through_all' },
        end2: { type: 'through_all' },
      },
    })
    // Bolt holes in the rim.
    const rim = (await core.call('doc.topology')).faces.find(
      (f) => f.normal && Math.abs(f.normal[2] - 1) < 1e-9,
    ).ref
    const { id: holes } = await core.call('sketch.new', { plane: rim })
    for (let i = 0; i < 8; i++)
      for (const y of [3, 117])
        await core.call('sketch.edit', {
          id: holes,
          op: { op: 'add_circle', center: { at: [15 + i * 30, y] }, radius: 1.6 },
        })
    await core.call('sketch.close')
    await core.call('feature.add', {
      feature: { type: 'extrude', sketch: holes, cut: true, end1: { type: 'blind', depth: 12 } },
    })
  })
  const shaft = join(dir, 'shaft.ofp')
  await part(core, shaft, [{ op: 'add_circle', center: { at: [0, 0] }, radius: 9 }], 160)
  const gear = join(dir, 'gear.ofp')
  await part(core, gear, [{ op: 'add_circle', center: { at: [0, 0] }, radius: 30 }], 20)
  const bolt = join(dir, 'bolt.ofp')
  await part(core, bolt, [{ op: 'add_circle', center: { at: [0, 0] }, radius: 1.5 }], 20)
  await core.call('asm.new')
  await core.call('asm.insert', { path: housing })
  // Shafts and gears lie along y: turn +z onto -y.
  const turn = [Math.SQRT1_2, 0, 0, Math.SQRT1_2]
  for (const [x, i] of [
    [50, 0],
    [120, 1],
    [190, 2],
  ]) {
    await core.call('asm.insert', {
      path: shaft,
      transform: { translation: [x, 140, 45], rotation: turn },
    })
    await core.call('asm.insert', {
      path: gear,
      transform: { translation: [x, 40 + i * 25, 45], rotation: turn },
    })
  }
  for (let i = 0; i < 8; i++)
    for (const y of [3, 117])
      await core.call('asm.insert', {
        path: bolt,
        transform: { translation: [15 + i * 30, y, 80], rotation: [0, 0, 0, 1] },
      })
  const gearbox = join(dir, 'gearbox.ofa')
  await core.call('doc.save', { path: gearbox })
  core.stop()
  return { housing, gearbox }
}

async function measure(dir) {
  const { housing, gearbox } = await fixture(dir)
  const out = {}
  let core = new Core()
  // Sheet 1: the housing, standard 3 view and isometric, plus a section.
  await core.call('drawing.new', { size: { type: 'a2' } })
  out.standard_views_ms = await time(async () => {
    await core.call('drawing.views.standard', { sheet: 1, model: housing })
    await core.call('drawing.render', { sheet: 1, summary: true })
  })
  const info = await core.call('drawing.info')
  const front = info.drawing.sheets[0].views[0]
  out.section_view_ms = await time(async () => {
    await core.call('drawing.view.add', {
      sheet: 1,
      view: {
        type: 'section',
        parent: front.id,
        line: {
          points: [
            [-10, 45],
            [260, 45],
          ],
        },
      },
      origin: [front.origin[0], 40],
    })
    await core.call('drawing.render', { sheet: 1, summary: true })
  })
  // Sheet 2: the gearbox, front, top, isometric and exploded isometric,
  // with an indented BOM, auto balloons and a revision table.
  const sheet2 = (await core.call('drawing.sheet.add', {})).id
  const views = (await core.call('drawing.views.standard', { sheet: sheet2, model: gearbox })).views
  await core.call('drawing.annotation.add', {
    sheet: sheet2,
    annotation: { type: 'table', table: 'bom', view: views.front, bom: 'indented', at: [20, 400] },
  })
  await core.call('drawing.balloons.auto', { sheet: sheet2, view: views.isometric })
  await core.call('drawing.annotation.add', {
    sheet: sheet2,
    annotation: {
      type: 'table',
      table: 'revision',
      rows: [{ description: 'Released' }],
      at: [584, 410],
      anchor: 'top_right',
    },
  })
  const file = join(dir, 'gearbox.ofd')
  await core.call('doc.save', { path: file })
  core.stop()

  // A fresh core: open, then a cold rebuild of the gearbox sheet.
  core = new Core()
  out.open_ms = await time(() => core.call('doc.open', { path: file }))
  out.gearbox_sheet_ms = await time(() =>
    core.call('drawing.render', { sheet: sheet2, summary: true }),
  )
  const kept = process.env.OPENFORGE_PERF_OUTPUT ?? dir
  out.pdf_export_ms = await time(() =>
    core.call('drawing.export', { path: join(kept, 'gearbox.pdf') }),
  )
  out.dxf_export_ms = await time(() =>
    core.call('drawing.export', { path: join(kept, 'gearbox.dxf') }),
  )
  // Every view drew: timings of failed views would mean nothing.
  for (const sheet of [1, sheet2]) {
    const render = await core.call('drawing.render', { sheet, summary: true })
    for (const v of render.views) if (v.error) throw new Error(`view ${v.id}: ${v.error}`)
  }
  core.stop()
  return out
}

const dir = mkdtempSync(join(tmpdir(), 'openforge-phase4-perf-'))
try {
  const runs = settingBudgets ? 3 : 1
  const results = []
  for (let i = 0; i < runs; i++) results.push(await measure(dir))
  const machine = `${cpus()[0]?.model ?? 'unknown'}, ${platform()} ${arch()}`
  console.log(`Phase 4 performance (housing drawing, 23-component gearbox sheet), ${machine}`)
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
            'Shelled 240 x 120 x 90 housing with 3 bores and 16 bolt holes (A2 sheet: standard 3 view, isometric, full section); gearbox of 23 components (front, top, side, isometric, indented BOM, auto balloons, revision table)',
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
