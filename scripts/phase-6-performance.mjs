// Surfacing, direct editing, mould and mesh performance through the native core
// (Phase 6 budgets).
// Build first, then:
//   node scripts/phase-6-performance.mjs                measure and print
//   node scripts/phase-6-performance.mjs --set-budgets  measure 3 runs, write budgets
//   node scripts/phase-6-performance.mjs --check        measure, compare with budgets
// OPENFORGE_CORE may select a release binary. OPENFORGE_BUDGET_SCALE multiplies
// the budgets for slower machines (default 1). OPENFORGE_PERF_OUTPUT keeps the
// fixture parts and STL files in that directory.
import { spawn } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { arch, cpus, platform, tmpdir } from 'node:os'
import { Buffer } from 'node:buffer'
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
const budgetsFile = join(root, 'docs/phase-6-budgets.json')
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

const near = (a, b, tol = 1e-6) => Math.abs(a - b) < tol
const check = (snap) => {
  for (const f of snap.features) if (f.error) throw new Error(`${f.name}: ${f.error}`)
  return snap
}

/** A binary STL of a UV sphere of radius `r` with about `2 nu nv` triangles. */
function sphereStl(path, r, nu, nv) {
  const rings = []
  for (let j = 1; j < nv; j++) {
    const v = (Math.PI * j) / nv
    const ring = []
    for (let i = 0; i < nu; i++) {
      const u = (2 * Math.PI * i) / nu
      ring.push([r * Math.sin(v) * Math.cos(u), r * Math.sin(v) * Math.sin(u), r * Math.cos(v)])
    }
    rings.push(ring)
  }
  const top = [0, 0, r]
  const bottom = [0, 0, -r]
  const tris = []
  for (let i = 0; i < nu; i++) {
    const k = (i + 1) % nu
    tris.push([top, rings[0][i], rings[0][k]])
    for (let j = 0; j + 1 < rings.length; j++) {
      const [a, b, c, d] = [rings[j][i], rings[j][k], rings[j + 1][i], rings[j + 1][k]]
      tris.push([a, c, d], [a, d, b])
    }
    const last = rings[rings.length - 1]
    tris.push([bottom, last[k], last[i]])
  }
  const buf = Buffer.alloc(84 + 50 * tris.length)
  buf.writeUInt32LE(tris.length, 80)
  let o = 84
  for (const t of tris) {
    o += 12 // the normal: readers recompute it
    for (const p of t) {
      for (const x of p) {
        buf.writeFloatLE(x, o)
        o += 4
      }
    }
    o += 2
  }
  writeFileSync(path, buf)
  return tris.length
}

async function sketch(core, plane, ops) {
  const { id } = await core.call('sketch.new', { plane })
  for (const op of ops) await core.call('sketch.edit', { id, op })
  await core.call('sketch.close')
  return id
}
const front = { type: 'front_plane' }
const rect = (a, b) => ({ op: 'rectangle', a, b })
const circle = (at, radius) => ({ op: 'add_circle', center: { at }, radius })
const block = async (core, w, d, h, holes = []) => {
  await core.call('doc.new')
  const s = await sketch(core, front, [rect([0, 0], [w, d]), ...holes])
  check(
    await core.call('feature.add', {
      feature: { type: 'extrude', sketch: s, end1: { type: 'blind', depth: h } },
    }),
  )
}
const bodies = async (core, kind) =>
  (await core.call('doc.info')).bodies.filter((b) => b.kind === kind)

async function measure(dir) {
  const kept = process.env.OPENFORGE_PERF_OUTPUT ?? dir
  const out = {}
  const core = new Core()

  // Knit: a capped 48-sided prism of extruded surface, 50 faces.
  await core.call('doc.new')
  const prism = await sketch(core, front, [
    { op: 'polygon', center: [0, 0], vertex: [30, 0], sides: 48 },
  ])
  check(
    await core.call('feature.add', {
      feature: {
        type: 'extruded_surface',
        sketch: prism,
        end1: { type: 'blind', depth: 20 },
        cap_ends: true,
      },
    }),
  )
  const [shell] = await bodies(core, 'surface')
  if (shell.faces.length !== 50) throw new Error(`the prism has ${shell.faces.length} faces`)
  out.knit_50_faces_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: { type: 'knit', bodies: [shell.id], create_solid: true },
      }),
    ),
  )

  // Filled surface: the open top of a block, patched back into a solid.
  await block(core, 40, 20, 10)
  const topFace = (await core.call('doc.topology')).faces.find((f) => near(f.centroid[2], 10))
  check(
    await core.call('feature.add', {
      feature: { type: 'delete_face', faces: [topFace.ref], mode: 'delete' },
    }),
  )
  const rim = (await core.call('doc.topology')).edges.filter(
    (e) => near(e.start[2], 10) && near(e.end[2], 10),
  )
  if (rim.length !== 4) throw new Error(`the rim has ${rim.length} edges`)
  out.filled_surface_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: {
          type: 'filled_surface',
          boundary: rim.map((e) => ({ edge: e.ref })),
          create_solid: true,
        },
      }),
    ),
  )
  if (!near((await core.call('mass_properties')).volume, 8000, 1e-3))
    throw new Error('the filled block is not whole')

  // Move face: the top of a bracket plate with six holes.
  const holes = [10, 30, 50].flatMap((x) => [circle([x, 10], 3), circle([x, 30], 3)])
  await block(core, 60, 40, 8, holes)
  const plateTop = (await core.call('doc.topology')).faces.find((f) => near(f.centroid[2], 8))
  out.move_face_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: {
          type: 'move_face',
          faces: [plateTop.ref],
          motion: { type: 'offset', distance: 2 },
        },
      }),
    ),
  )

  // Tooling split: a drafted 40 x 30 x 20 part in a 70 x 60 x 40 mould box.
  await core.call('doc.new')
  const part = await sketch(core, front, [rect([-20, -15], [20, 15])])
  check(
    await core.call('feature.add', {
      feature: { type: 'extrude', sketch: part, end1: { type: 'blind', depth: 20 } },
    }),
  )
  const walls = (await core.call('doc.topology')).faces.filter(
    (f) => f.normal && Math.abs(f.normal[2]) < 1e-6,
  )
  check(
    await core.call('feature.add', {
      feature: {
        type: 'draft',
        faces: walls.map((f) => f.ref),
        neutral_plane: front,
        direction: null,
        angle: (3 * Math.PI) / 180,
      },
    }),
  )
  const line = check(
    await core.call('feature.add', {
      feature: { type: 'parting_line', pull: front, split_faces: true },
    }),
  ).id
  check(
    await core.call('feature.add', {
      feature: { type: 'parting_surface', parting_line: line, distance: 30 },
    }),
  )
  const box = await sketch(core, front, [rect([-35, -30], [35, 30])])
  out.tooling_split_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: { type: 'tooling_split', parting_line: line, sketch: box, depth1: 30, depth2: 10 },
      }),
    ),
  )

  // A 1M-triangle STL as a graphics body.
  const big = join(kept, 'sphere-1m.stl')
  const bigCount = sphereStl(big, 50, 1000, 500)
  await core.call('doc.new')
  out.import_1m_ms = await time(() => core.call('import', { path: big }))
  const imported = (await core.call('doc.info')).scene.meshes[0]
  if (imported?.triangle_count !== bigCount) throw new Error('the 1M-triangle mesh is missing')

  // A 100k-triangle sphere: B-rep conversion, decimation and a sphere fit.
  const mid = join(kept, 'sphere-100k.stl')
  sphereStl(mid, 50, 316, 158)
  const midMesh = async () => {
    await core.call('doc.new')
    await core.call('import', { path: mid })
    return (await core.call('doc.info')).scene.meshes[0].id
  }
  let mesh = await midMesh()
  out.mesh_brep_100k_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: { type: 'mesh_to_brep', body: mesh, solid: true, group_facets: false },
      }),
    ),
  )
  // Undercut analysis ray casts every display triangle of that solid.
  const sphere = (await core.call('doc.info')).bodies.find((b) => b.kind === 'solid')
  if (sphere?.faces.length !== 99224) throw new Error('the mesh solid is missing')
  out.undercut_100k_ms = await time(() =>
    core.call('doc.analysis', { analysis: { type: 'undercut', pull: front } }),
  )
  mesh = await midMesh()
  out.decimate_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: {
          type: 'decimate_mesh',
          body: mesh,
          target: { type: 'fraction', fraction: 0.25 },
        },
      }),
    ),
  )
  mesh = await midMesh()
  out.surface_fit_ms = await time(async () =>
    check(
      await core.call('feature.add', {
        feature: { type: 'surface_from_mesh', body: mesh, fit: 'sphere' },
      }),
    ),
  )
  const [fitted] = await bodies(core, 'surface')
  if (!fitted) throw new Error('no fitted surface')
  core.stop()
  return out
}
const dir = mkdtempSync(join(tmpdir(), 'openforge-phase6-perf-'))
try {
  const runs = settingBudgets ? 3 : 1
  const results = []
  for (let i = 0; i < runs; i++) results.push(await measure(dir))
  const machine = `${cpus()[0]?.model ?? 'unknown'}, ${platform()} ${arch()}`
  console.log(`Phase 6 performance (surfacing, direct editing, mould, mesh), ${machine}`)
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
            'Knit of a capped 48-sided extruded surface (50 faces) into a solid; filled surface closing the open top of a 40 x 20 x 10 block; move face on a 60 x 40 x 8 plate with six holes; tooling split of a drafted 40 x 30 x 20 part in a 70 x 60 x 40 box; import of a 1M-triangle UV sphere STL as a graphics body; mesh to solid B-rep (99,224 facets) and undercut analysis of that solid, decimation to a quarter and a sphere fit on the same mesh',
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
