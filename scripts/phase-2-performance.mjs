// Reproducible native-core measurements for the delivered Phase 2 geometry slices.
// Build first, then:
//   node scripts/phase-2-performance.mjs [--runs N]     measure and write the report
//   node scripts/phase-2-performance.mjs --set-budgets   measure 3 runs, write budgets
//   node scripts/phase-2-performance.mjs --check         measure, compare with budgets
// OPENFORGE_CORE may select a release binary. Timings include RPC and tessellation.
// OPENFORGE_BUDGET_SCALE multiplies the budgets for slower machines (default 1).
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { arch, cpus, platform, release, tmpdir, totalmem } from 'node:os'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { performance } from 'node:perf_hooks'

const root = resolve(import.meta.dirname, '..')
const binary =
  process.env.OPENFORGE_CORE ??
  join(root, 'target/debug', process.platform === 'win32' ? 'openforge.exe' : 'openforge')
const repetitions = 5
const argv = process.argv.slice(2)
const checking = argv.includes('--check')
const settingBudgets = argv.includes('--set-budgets')
const runsArg = argv.indexOf('--runs')
const runs = settingBudgets ? 3 : runsArg >= 0 ? Math.max(1, Number(argv[runsArg + 1]) || 1) : 1
const budgetsFile = join(root, 'docs/phase-2-budgets.json')
const budgetScale = Number(process.env.OPENFORGE_BUDGET_SCALE ?? 1)
const temporary = mkdtempSync(join(tmpdir(), 'openforge-phase2-performance-'))
const output = join(root, 'docs/phase-2-performance')
const systemTime =
  process.platform === 'darwin' ? ['-l'] : process.platform === 'linux' ? ['-v'] : null
const memoryEnabled = systemTime !== null && existsSync('/usr/bin/time')

class Core {
  constructor() {
    this.next = 0
    this.pending = new Map()
    this.stderr = ''
    this.child = spawn(
      memoryEnabled ? '/usr/bin/time' : binary,
      memoryEnabled ? [...systemTime, binary, 'serve'] : ['serve'],
      { stdio: ['pipe', 'pipe', 'pipe'] },
    )
    this.child.stderr.on('data', (data) => {
      this.stderr += data.toString()
    })
    this.lines = createInterface({ input: this.child.stdout })
    this.lines.on('line', (line) => {
      try {
        const response = JSON.parse(line)
        const pending = this.pending.get(response.id)
        if (!pending) throw new Error(`Unexpected RPC response ID ${response.id}`)
        this.pending.delete(response.id)
        clearTimeout(pending.timer)
        if (response.error) pending.reject(new Error(response.error.message))
        else pending.resolve(response.result)
      } catch (error) {
        this.fail(error)
      }
    })
    this.finished = new Promise((resolve, reject) => {
      this.child.once('error', (error) => {
        this.fail(error)
        reject(error)
      })
      this.child.once('close', (code) => {
        if (this.pending.size) this.fail(new Error('Core closed with outstanding requests'))
        if (code) reject(new Error(`Core exited ${code}: ${this.stderr}`))
        else resolve()
      })
    })
  }
  fail(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer)
      pending.reject(error)
    }
    this.pending.clear()
  }
  call(method, params = {}) {
    const id = ++this.next
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        reject(new Error(`RPC timed out: ${method}`))
        this.child.kill()
      }, 60_000)
      this.pending.set(id, { resolve, reject, timer })
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
    })
  }
  async close() {
    this.child.stdin.end()
    await this.finished
    const match =
      process.platform === 'darwin'
        ? /(\d+)\s+maximum resident set size/.exec(this.stderr)
        : /Maximum resident set size \(kbytes\):\s*(\d+)/.exec(this.stderr)
    return match ? Number(match[1]) * (process.platform === 'darwin' ? 1 : 1024) : null
  }
}

function valid(snapshot) {
  assert.ok(
    snapshot.features.every((feature) => feature.error === null),
    JSON.stringify(snapshot.features),
  )
}
async function add(core, feature) {
  const snapshot = await core.call('feature.add', { feature })
  valid(snapshot)
  return snapshot.features.at(-1).id
}
async function profile(core, radii) {
  const sketch = (await core.call('sketch.new', { plane: { type: 'front_plane' } })).id
  const entities = []
  for (const radius of radii) {
    const response = await core.call('sketch.edit', {
      id: sketch,
      op: { op: 'add_circle', center: { at: [0, 0] }, radius },
    })
    entities.push(response.created[0])
  }
  await core.call('sketch.close')
  const contours = (await core.call('sketch.get', { id: sketch })).contours
  return {
    sketch,
    contours: entities.map((entity) => {
      const contour = contours.find((c) => c.reference.entities.includes(entity))
      assert.ok(contour)
      return {
        sketch,
        entities: contour.reference.entities,
        start: contour.ordered_entities[0],
        reverse: false,
      }
    }),
  }
}
async function seedSweep(hollow) {
  const core = new Core()
  try {
    const profiles = await profile(core, hollow ? [4, 2] : [4])
    const source = await add(core, {
      type: 'sketch3d',
      next_id: 7,
      entities: [
        { id: 1, kind: 'point', at: [0, 0, 0] },
        { id: 2, kind: 'point', at: [0, 0, 80] },
        { id: 3, kind: 'line', a: 1, b: 2 },
      ],
      constraints: [
        { id: 4, kind: 'fixed', point: 1, at: [0, 0, 0] },
        { id: 5, kind: 'axis', line: 3, axis: 'z' },
        { id: 6, kind: 'distance', a: 1, b: 2, value: 80 },
      ],
    })
    const feature = {
      type: 'sweep',
      profile: hollow ? profiles.contours[0] : profiles.sketch,
      inner_profiles: hollow ? [profiles.contours[1]] : [],
      path: { type: 'feature', id: source },
    }
    const id = await add(core, feature)
    const name = hollow ? 'hollow_spatial_sweep' : 'straight_spatial_sweep'
    const path = join(temporary, `${name}.ofp`)
    await core.call('doc.save', { path })
    return {
      name,
      path,
      feature,
      id,
      parameter: { key: `f${source}.dimension.6`, value: 80 },
      volume: Math.PI * (hollow ? 12 : 16) * 80,
    }
  } finally {
    await core.close()
  }
}
async function seedDome() {
  const core = new Core()
  try {
    const { sketch } = await profile(core, [10])
    await add(core, { type: 'extrude', sketch, end1: { type: 'blind', depth: 8 } })
    const topology = await core.call('doc.topology')
    const face = topology.faces.find(
      (face) => face.surface === 'plane' && face.normal[2] > 0.99,
    )?.ref
    assert.ok(face, 'Dome fixture requires a planar circular top face')
    const feature = { type: 'dome', face, height: 3 }
    const id = await add(core, feature)
    const path = join(temporary, 'spherical_cap_dome.ofp')
    await core.call('doc.save', { path })
    return {
      name: 'spherical_cap_dome',
      path,
      feature,
      id,
      parameter: { key: `f${id}.height`, value: 3 },
      volume: Math.PI * 100 * 8 + (Math.PI * 3 * (3 * 100 + 9)) / 6,
    }
  } finally {
    await core.close()
  }
}
async function sketchOn(core, plane, ops) {
  const { id } = await core.call('sketch.new', { plane })
  for (const op of ops) await core.call('sketch.edit', { id, op })
  await core.call('sketch.close')
  return id
}
const front = { type: 'front_plane' }
const rectangle = (a, b) => ({ op: 'rectangle', a, b })
const segment = (a, b) => ({ op: 'add_line', a: { at: a }, b: { at: b } })
/** Build a fixture in a fresh core, save it and describe its measured feature. */
async function seed(name, build) {
  const core = new Core()
  try {
    const fixture = await build(core)
    const path = join(temporary, `${name}.ofp`)
    await core.call('doc.save', { path })
    return { name, path, ...fixture }
  } finally {
    await core.close()
  }
}
const cylinder = async (core, radius, depth) => {
  const { sketch } = await profile(core, [radius])
  await add(core, { type: 'extrude', sketch, end1: { type: 'blind', depth } })
  return (await core.call('doc.topology')).faces.find((f) => f.surface === 'cylinder').ref
}
const seedThread = () =>
  seed('modeled_thread', async (core) => {
    const face = await cylinder(core, 5, 20)
    const feature = { type: 'thread', face, diameter: 10, pitch: 1.5, length: 15, start_offset: 2 }
    const id = await add(core, feature)
    const p = 1.5,
      h = 0.541265877365274 * p,
      a = p / 4,
      b = (7 * p) / 8
    const groove =
      ((a + b) / 2) * h * 2 * Math.PI * (5 - h + (h * (a + 2 * b)) / (3 * (a + b))) * (15 / p)
    return { feature, id, volume: Math.PI * 25 * 20 - groove, tolerance: 1e-4 * groove }
  })
const seedRib = () =>
  seed('housing_rib', async (core) => {
    const base = await sketchOn(core, front, [rectangle([0, 0], [40, 20])])
    await add(core, { type: 'extrude', sketch: base, end1: { type: 'blind', depth: 10 } })
    const top = (await core.call('doc.topology')).faces.find((f) => f.normal?.[2] > 0.99).ref
    await add(core, { type: 'shell', faces: [top], thickness: 2 })
    const rim = await add(core, { type: 'plane', mode: 'offset', reference: front, distance: 10 })
    const sketch = await sketchOn(core, { type: 'feature', id: rim }, [segment([20, 6], [20, 14])])
    const feature = { type: 'rib', sketch, thickness: 2, direction: 'normal_to_sketch' }
    const id = await add(core, feature)
    return { feature, id, volume: 8000 - 36 * 16 * 8 + 2 * 16 * 8 }
  })
const seedWrap = () =>
  seed('cylinder_wrap', async (core) => {
    const face = await cylinder(core, 10, 40)
    const plane = await add(core, {
      type: 'plane',
      mode: 'offset',
      reference: { type: 'right_plane' },
      distance: 10,
    })
    // The right plane's local x runs along world -z and local y along world y.
    const sketch = await sketchOn(core, { type: 'feature', id: plane }, [
      rectangle([-14, -2.5], [-10, 2.5]),
    ])
    const feature = { type: 'wrap', sketch, face, mode: 'emboss', depth: 1 }
    const id = await add(core, feature)
    return { feature, id, volume: Math.PI * 100 * 40 + 0.5 * 0.5 * (121 - 100) * 4 }
  })
const seedBoundary = () =>
  seed('boundary_prism', async (core) => {
    const bottom = await sketchOn(core, front, [rectangle([-5, -5], [5, 5])])
    const lift = await add(core, { type: 'plane', mode: 'offset', reference: front, distance: 10 })
    const upper = await sketchOn(core, { type: 'feature', id: lift }, [rectangle([-5, -5], [5, 5])])
    const curves = []
    for (const y of [-5, 5])
      curves.push({
        type: 'feature',
        id: await sketchOn(core, { type: 'right_plane' }, [segment([0, y], [-10, y])]),
      })
    const feature = { type: 'boundary', sections: [bottom, upper], curves }
    const id = await add(core, feature)
    return { feature, id, volume: 1000 }
  })
const top = { type: 'top_plane' }
const right = { type: 'right_plane' }
const circleAt = (at, radius) => ({ op: 'add_circle', center: { at }, radius })
/** Volume of square sections of half-width `r(z)` over 0..10. */
const squares = (r) => {
  let v = 0
  for (let i = 0; i < 4000; i++) v += 4 * r((i + 0.5) / 400) ** 2 * (10 / 4000)
  return v
}
/** Squares of half-width 5 at z = 0 and 3 at z = 10. */
async function squareSections(core) {
  const bottom = await sketchOn(core, front, [rectangle([-5, -5], [5, 5])])
  const lift = await add(core, { type: 'plane', mode: 'offset', reference: front, distance: 10 })
  const upper = await sketchOn(core, { type: 'feature', id: lift }, [rectangle([-3, -3], [3, 3])])
  return [bottom, upper]
}
/** The id of the first line in a new sketch. */
async function lineSketch(core, plane, a, b) {
  const { id } = await core.call('sketch.new', { plane })
  const line = (await core.call('sketch.edit', { id, op: segment(a, b) })).created[0]
  await core.call('sketch.close')
  return { type: 'sketch_entity', sketch: id, entity: line }
}
const seedGuidedSweep = () =>
  seed('scaled_guide_sweep', async (core) => {
    // On the top plane local y runs along world -z.
    const path = await lineSketch(core, top, [0, 0], [0, -20])
    const guide = await lineSketch(core, top, [2, 0], [4, -20])
    const profile = await sketchOn(core, front, [circleAt([0, 0], 2)])
    const feature = { type: 'sweep', profile, path, guide, guide_scale: true }
    const id = await add(core, feature)
    return { feature, id, volume: ((Math.PI * 20) / 3) * (4 + 8 + 16) }
  })
const seedTangentLoft = () =>
  seed('tangent_loft', async (core) => {
    const profiles = await squareSections(core)
    const end = { type: 'normal', tangent_length: 1 }
    const feature = { type: 'loft', profiles, start: end, end }
    const id = await add(core, feature)
    const hermite = (z) => 5 - 2 * (3 * (z / 10) ** 2 - 2 * (z / 10) ** 3)
    return { feature, id, volume: squares(hermite), tolerance: 1e-3 }
  })
const seedCenterlineLoft = () =>
  seed('centerline_loft', async (core) => {
    const profiles = await squareSections(core)
    // On the right plane local x runs along world -z.
    const centerline = await lineSketch(core, right, [2, 0], [-12, 0])
    const feature = { type: 'loft', profiles, centerline }
    const id = await add(core, feature)
    return { feature, id, volume: (10 / 3) * (100 + 36 + 60) }
  })
const seedDomeWrap = () =>
  seed('dome_geodesic_wrap', async (core) => {
    await cylinder(core, 10, 40)
    const topFace = (await core.call('doc.topology')).faces.find(
      (f) => f.surface === 'plane' && f.normal[2] > 0.99,
    ).ref
    await add(core, { type: 'dome', face: topFace, height: 5 })
    const plane = await add(core, { type: 'plane', mode: 'offset', reference: front, distance: 45 })
    const sketch = await sketchOn(core, { type: 'feature', id: plane }, [circleAt([0, 0], 2)])
    const face = (await core.call('doc.topology')).faces.find((f) => f.surface === 'sphere').ref
    const feature = { type: 'wrap', sketch, face, mode: 'emboss', depth: 0.5 }
    const id = await add(core, feature)
    const rs = 12.5
    const shell = ((2 * Math.PI) / 3) * ((rs + 0.5) ** 3 - rs ** 3) * (1 - Math.cos(2 / rs))
    const dome = (Math.PI * 5 * (3 * 100 + 25)) / 6
    return { feature, id, volume: Math.PI * 100 * 40 + dome + shell }
  })
function series(samples) {
  const sorted = [...samples].sort((a, b) => a - b)
  return {
    samples_ms: samples,
    median_ms: sorted[Math.floor(sorted.length / 2)],
    min_ms: sorted[0],
    max_ms: sorted.at(-1),
  }
}
async function timed(action) {
  const start = performance.now()
  const result = await action()
  return { result, ms: performance.now() - start }
}
async function measure(fixture) {
  const startup = [],
    rebuild = [],
    preview = [],
    memory = []
  for (let i = 0; i < repetitions; i++) {
    const start = performance.now()
    const core = new Core()
    try {
      valid(await core.call('doc.open', { path: fixture.path }))
      startup.push(performance.now() - start)
    } finally {
      memory.push(await core.close())
    }
  }
  const core = new Core()
  let info, actualVolume
  try {
    info = await core.call('core.info')
    valid(await core.call('doc.open', { path: fixture.path }))
    for (let i = 0; i < repetitions; i++) {
      // Resubmitting the definition invalidates the feature and rebuilds it and
      // everything after it; earlier cached states are reused.
      const sample = await timed(() =>
        core.call('feature.update', { id: fixture.id, feature: fixture.feature }),
      )
      valid(sample.result)
      rebuild.push(sample.ms)
    }
    for (let i = 0; i < repetitions; i++) {
      const sample = await timed(() =>
        core.call('feature.preview', { editing: fixture.id, feature: fixture.feature }),
      )
      assert.equal(sample.result.error, null)
      assert.ok(sample.result.scene?.body)
      preview.push(sample.ms)
    }
    actualVolume = (await core.call('mass_properties')).volume
    assert.ok(
      Math.abs(actualVolume - fixture.volume) < (fixture.tolerance ?? 1e-4),
      `${fixture.name}: ${actualVolume} != ${fixture.volume}`,
    )
  } finally {
    memory.push(await core.close())
  }
  const observed = memory.filter((value) => value !== null)
  return {
    fixture: fixture.name,
    repetitions,
    volume_mm3: actualVolume,
    expected_volume_mm3: fixture.volume,
    volume_tolerance_mm3: fixture.tolerance ?? 1e-4,
    startup_open: series(startup),
    feature_rebuild_rpc: series(rebuild),
    preview_rpc: series(preview),
    peak_rss_bytes: observed.length ? Math.max(...observed) : null,
    peak_rss_samples_bytes: memory,
    core_info: info,
  }
}
function command(name, args) {
  const response = spawnSync(name, args, { cwd: root, encoding: 'utf8' })
  return response.status === 0 ? response.stdout.trim() : null
}
function kernelVersion() {
  const candidates = [
    process.env.OCCT_INCLUDE_DIR,
    process.env.CONDA_PREFIX && join(process.env.CONDA_PREFIX, 'include/opencascade'),
    '/opt/homebrew/include/opencascade',
    '/usr/local/include/opencascade',
    '/usr/include/opencascade',
  ].filter(Boolean)
  for (const directory of candidates) {
    const header = join(directory, 'Standard_Version.hxx')
    if (existsSync(header))
      return {
        version:
          /#define\s+OCC_VERSION_COMPLETE\s+"([^"]+)"/.exec(readFileSync(header, 'utf8'))?.[1] ??
          'unknown',
        header,
      }
  }
  return null
}
try {
  const metadata = {
    recorded_at_utc: new Date().toISOString(),
    commit: command('git', ['rev-parse', 'HEAD']),
    worktree_status: command('git', ['status', '--short']),
    os: { platform: platform(), release: release(), arch: arch() },
    cpu: { model: cpus()[0]?.model ?? 'unknown', logical_processors: cpus().length },
    system_memory_bytes: totalmem(),
    node: process.version,
    rust: command('rustc', ['--version']),
    core_binary: binary,
    core_sha256: createHash('sha256').update(readFileSync(binary)).digest('hex'),
    build_profile: binary.includes('/target/debug/') ? 'debug' : 'unspecified',
    opencascade: kernelVersion(),
    timing_method:
      'Monotonic parent-process wall clock; RPC response includes rebuild/preview geometry, tessellation and JSON serialization. Startup includes process creation and doc.open; operating-system file caches are uncontrolled.',
    memory_method: memoryEnabled
      ? `/usr/bin/time ${systemTime.join(' ')} maximum core resident set size over each complete core process, including RPC scenes, preview states and undo checkpoints; no polling estimate.`
      : 'Peak core memory unavailable on this platform.',
  }
  const fixtures = [
    await seedSweep(false),
    await seedSweep(true),
    await seedDome(),
    await seedThread(),
    await seedRib(),
    await seedWrap(),
    await seedBoundary(),
    await seedGuidedSweep(),
    await seedTangentLoft(),
    await seedCenterlineLoft(),
    await seedDomeWrap(),
  ]
  const metrics = ['startup_open', 'feature_rebuild_rpc', 'preview_rpc']
  const allRuns = []
  for (let run = 0; run < runs; run++) {
    const results = []
    for (const fixture of fixtures) {
      results.push(await measure(fixture))
      process.stdout.write(`Measured ${fixture.name}${runs > 1 ? ` (run ${run + 1})` : ''}\n`)
    }
    allRuns.push(results)
  }
  const results = allRuns.at(-1)
  if (checking) {
    // Compare medians and peak memory with the recorded budgets.
    const budgets = JSON.parse(readFileSync(budgetsFile, 'utf8')).budgets
    const failures = []
    for (const result of results) {
      const budget = budgets[result.fixture]
      if (!budget) {
        failures.push(`${result.fixture}: no budget recorded`)
        continue
      }
      for (const metric of metrics) {
        const limit = budget[`${metric}_ms`] * budgetScale
        const value = result[metric].median_ms
        const line = `${result.fixture} ${metric}: ${value.toFixed(1)} ms (budget ${limit.toFixed(0)} ms)`
        process.stdout.write(`${value > limit ? 'OVER' : 'ok  '} ${line}\n`)
        if (value > limit) failures.push(line)
      }
      if (result.peak_rss_bytes !== null && budget.peak_rss_mib) {
        const mib = result.peak_rss_bytes / 1024 / 1024
        const limit = budget.peak_rss_mib * budgetScale
        process.stdout.write(
          `${mib > limit ? 'OVER' : 'ok  '} ${result.fixture} peak RSS: ${mib.toFixed(1)} MiB (budget ${limit.toFixed(0)} MiB)\n`,
        )
        if (mib > limit) failures.push(`${result.fixture} peak RSS ${mib.toFixed(1)} MiB`)
      }
    }
    if (failures.length) {
      process.stderr.write(`Performance budgets exceeded:\n${failures.join('\n')}\n`)
      process.exitCode = 1
    } else process.stdout.write('All performance budgets met\n')
  } else {
    if (settingBudgets) {
      // Three times the slowest median over the runs (at least 50 ms), and
      // one and a half times the largest peak memory, rounded up.
      const budgets = {}
      for (const fixture of fixtures) {
        const rows = allRuns.map((r) => r.find((x) => x.fixture === fixture.name))
        const entry = {}
        for (const metric of metrics) {
          const worst = Math.max(...rows.map((r) => r[metric].median_ms))
          entry[`${metric}_ms`] = Math.max(50, Math.ceil((3 * worst) / 10) * 10)
        }
        const rss = rows.map((r) => r.peak_rss_bytes).filter((v) => v !== null)
        if (rss.length)
          entry.peak_rss_mib = Math.ceil((1.5 * Math.max(...rss)) / 1024 / 1024 / 10) * 10
        entry.observed_median_ms = Object.fromEntries(
          metrics.map((metric) => [
            metric,
            rows.map((r) => Number(r[metric].median_ms.toFixed(2))),
          ]),
        )
        budgets[fixture.name] = entry
      }
      writeFileSync(
        budgetsFile,
        JSON.stringify(
          {
            recorded_at_utc: metadata.recorded_at_utc,
            machine: `${metadata.cpu.model}, ${metadata.os.platform} ${metadata.os.arch}`,
            build_profile: metadata.build_profile,
            rule: 'Each budget is three times the slowest of three five-sample medians (at least 50 ms); memory is one and a half times the largest peak RSS. Scale with OPENFORGE_BUDGET_SCALE on slower machines.',
            budgets,
          },
          null,
          2,
        ) + '\n',
      )
      process.stdout.write(`Wrote ${budgetsFile}\n`)
    }
    const report = { metadata, runs: allRuns, results }
    writeFileSync(`${output}.json`, JSON.stringify(report, null, 2) + '\n')
    const row = (result) =>
      `| ${result.fixture} | ${result.startup_open.median_ms.toFixed(2)} | ${result.feature_rebuild_rpc.median_ms.toFixed(2)} | ${result.preview_rpc.median_ms.toFixed(2)} | ${result.peak_rss_bytes === null ? 'Unavailable' : (result.peak_rss_bytes / 1024 / 1024).toFixed(2)} |`
    const budgetNote = existsSync(budgetsFile)
      ? `Budgets for these fixtures are in [phase-2-budgets.json](phase-2-budgets.json): three times the slowest of three runs' medians (at least 50 ms) and one and a half times the largest peak memory, on this machine and build. \`node scripts/phase-2-performance.mjs --check\` (\`pnpm perf:check\`) fails when a median or the peak memory exceeds its budget; \`OPENFORGE_BUDGET_SCALE\` scales them for slower machines.`
      : 'No budgets are recorded yet; run with `--set-budgets`.'
    const markdown = `# Phase 2 measured performance\n\nRecorded ${metadata.recorded_at_utc}. This is a local development-build measurement of the delivered fixtures.\n\nRun \`cargo build -p openforge && node scripts/phase-2-performance.mjs\` to repeat. \`OPENFORGE_CORE\` can select another binary. Raw samples and exact metadata are in [phase-2-performance.json](phase-2-performance.json).\n\n- Commit: \`${metadata.commit}\`; working tree modified as recorded in the JSON.\n- Binary SHA-256: \`${metadata.core_sha256}\`; build profile: ${metadata.build_profile}.\n- Machine: ${metadata.cpu.model}, ${metadata.cpu.logical_processors} logical processors; ${metadata.os.platform} ${metadata.os.release} ${metadata.os.arch}; ${(metadata.system_memory_bytes / 1024 ** 3).toFixed(1)} GiB system memory.\n- Kernel header: OpenCASCADE ${metadata.opencascade?.version ?? 'unavailable'}; ${metadata.rust}; Node ${metadata.node}.\n\nFive repetitions per operation${runs > 1 ? `, ${runs} runs (the table shows the last run)` : ''}; the table reports medians in milliseconds. Startup opens a fresh core process each time (a cold rebuild of the whole part); it does not flush operating-system caches. Feature rebuild resubmits the measured feature's definition, which rebuilds it and every later feature while reusing earlier cached states (a warm rebuild). Preview edits the existing measured feature. All timed RPCs include tessellation and response serialization.\n\n| Fixture | Startup + open (ms) | Feature rebuild RPC (ms) | Preview RPC (ms) | Peak core RSS (MiB) |\n| --- | ---: | ---: | ---: | ---: |\n${results.map(row).join('\n')}\n\nThe straight sweep has a 4 mm circular section along a fully constrained 80 mm spatial line. The hollow sweep uses outer radius 4 mm and inner radius 2 mm on the same path. The dome adds a 3 mm spherical cap to the planar circular top of a radius 10 mm, depth 8 mm cylinder. The modeled thread is an M10 x 1.5 external thread 15 mm long on a radius 5 mm, 20 mm cylinder. The rib is a 2 mm web normal to its sketch inside a 40 x 20 x 10 housing shelled 2 mm. The wrap embosses a 5 x 4 mm rectangle 1 mm onto a radius 10 mm cylinder. The boundary is a 10 x 10 x 10 prism bounded by two sections and two straight curves. The scaled guide sweep grows a radius 2 circle to radius 4 over 20 mm along a straight guide. The tangent loft joins 10 and 6 mm squares 10 mm apart, leaving both normal to them. The centerline loft joins the same squares along a straight centerline. The dome wrap embosses a radius 2 circle 0.5 mm along geodesics onto a 5 mm spherical dome. Each fixture's final volume was checked against its analytical value (within 0.0001 mm³, or the tolerance recorded in the JSON).\n\n${metadata.memory_method}\n\n${budgetNote} Native UI, hosted-platform qualification and release-profile costs require their own evidence.\n`
    writeFileSync(`${output}.md`, markdown)
    process.stdout.write(`Wrote ${output}.json and ${output}.md\n`)
  }
} finally {
  rmSync(temporary, { recursive: true, force: true })
}
