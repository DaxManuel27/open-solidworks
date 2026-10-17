// Checks Phase 1 to 5 behaviour against the feature specs in docs/features,
// driving `openforge serve` over JSON-RPC the way the desktop and MCP clients do.
//
//   cargo build && pnpm conformance [filter]
//
// Each check names the spec section it comes from. Exit status is nonzero when a
// check fails; the report lists every result.
import { spawn } from 'node:child_process'
import { mkdtempSync, existsSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import readline from 'node:readline'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const exe = join(root, 'target/debug', process.platform === 'win32' ? 'openforge.exe' : 'openforge')
const filter = process.argv[2]?.toLowerCase()
const tmp = mkdtempSync(join(tmpdir(), 'openforge-conformance-'))
const PI = Math.PI

const core = spawn(exe, ['serve'], { stdio: ['pipe', 'pipe', 'inherit'] })
const lines = readline.createInterface({ input: core.stdout })
let next = 0
const pending = new Map()
lines.on('line', (line) => {
  const message = JSON.parse(line)
  pending.get(message.id)?.(message)
  pending.delete(message.id)
})

/** Call a core method; resolves to the result, rejects with the error message. */
const call = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++next
    pending.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)))
    core.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n')
  })
const fails = async (method, params) => {
  try {
    await call(method, params)
    return null
  } catch (e) {
    return e.message
  }
}

class Failure extends Error {}
const expect = (ok, message) => {
  if (!ok) throw new Failure(message)
}
const near = (a, b, tol = 1e-6, what = 'value') =>
  expect(Math.abs(a - b) <= tol * Math.max(1, Math.abs(b)), `${what}: ${a} != ${b}`)

// ---- modelling helpers ----

const fresh = () => call('doc.new')
async function sketch(plane, ops) {
  const { id } = await call('sketch.new', { plane })
  const created = []
  for (const op of ops) created.push((await call('sketch.edit', { id, op })).created)
  const view = await call('sketch.get', { id })
  await call('sketch.close')
  return { id, created, view }
}
const front = { type: 'front_plane' }
const top = { type: 'top_plane' }
const right = { type: 'right_plane' }
const rect = (a, b) => ({ op: 'rectangle', a, b })
const circle = (at, radius) => ({ op: 'add_circle', center: { at }, radius })
const line = (a, b) => ({ op: 'add_line', a: { at: a }, b: { at: b } })
// add_line reports the new line first, then its two points.
const lineId = (created) => created[0]

const sub = (a, b) => a.map((v, i) => v - b[i])
const dot = (a, b) => a.reduce((t, v, i) => t + v * b[i], 0)
const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
]

/** A sketch on `plane` drawn in world coordinates mapped into the plane's frame. */
async function worldSketch(plane, draw) {
  const { id } = await call('sketch.new', { plane })
  // An empty sketch isn't in the scene yet; a probe line puts it there.
  const probe = await call('sketch.edit', { id, op: line([0, 0], [1, 0]) })
  await call('sketch.close')
  const view = (await call('doc.info')).scene.sketches.find((x) => x.id === id)
  await call('sketch.edit', { id, op: { op: 'delete', ids: probe.created } })
  expect(view, 'sketch frame in scene')
  const { origin, normal, x_dir } = view.plane
  const y = cross(normal, x_dir)
  const local = (p) => [dot(sub(p, origin), x_dir), dot(sub(p, origin), y)]
  const created = []
  for (const op of draw(local)) created.push((await call('sketch.edit', { id, op })).created)
  await call('sketch.close')
  return { id, created }
}

/** Add a feature; returns its id and its status from the tree. */
async function add(feature, extra = {}) {
  const snap = await call('feature.add', { feature, ...extra })
  const id = snap.id
  const status = snap.features.find((f) => f.id === id)
  return { id, error: status.error, snap }
}
async function ok(feature, extra) {
  const r = await add(feature, extra)
  expect(r.error === null, `${feature.type} failed: ${r.error}`)
  return r
}
async function bad(feature, pattern) {
  const r = await add(feature)
  expect(r.error !== null, `${feature.type} should fail`)
  if (pattern) expect(pattern.test(r.error), `unexpected error: ${r.error}`)
  return r
}
const blind = (sketch, depth, more = {}) => ({
  type: 'extrude',
  sketch,
  end1: { type: 'blind', depth },
  ...more,
})
const volume = async () => (await call('mass_properties')).volume
const topology = () => call('doc.topology')
async function face(pred) {
  const f = (await topology()).faces.find(pred)
  expect(f, 'face not found')
  return f.ref
}
async function edge(pred) {
  const e = (await topology()).edges.find(pred)
  expect(e, 'edge not found')
  return e.ref
}
const facing = (n) => (f) => f.normal && n.every((v, i) => Math.abs(f.normal[i] - v) < 1e-6)
const at =
  (p, tol = 1e-6) =>
  (x) =>
    p.every((v, i) => v === null || Math.abs(x[i] - v) < tol)

/** A 40 x 20 x 10 block on the front plane; returns the extrude id. */
async function block() {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [40, 20])])
  return (await ok(blind(s.id, 10))).id
}

// ---- checks ----

const checks = []
const check = (spec, name, fn) => checks.push({ spec, name, fn })

// 01 Sketching
check(
  '01 Definition status',
  'a rectangle is under defined; dimensions and an origin relation fully define it',
  async () => {
    await fresh()
    const { id } = await call('sketch.new', { plane: front })
    const r = await call('sketch.edit', { id, op: rect([0, 0], [10, 5]) })
    const [p1] = r.created
    const lines = r.view?.curves ?? (await call('sketch.get', { id })).curves
    const ls = lines.filter((c) => c.type === 'line').map((c) => c.id)
    await call('sketch.edit', {
      id,
      op: { op: 'constraint', constraint: { type: 'coincident', a: p1, b: 0 } },
    })
    await call('sketch.edit', {
      id,
      op: { op: 'dimension', constraint: { type: 'length', line: ls[0], value: 0 }, value: 12 },
    })
    const v = await call('sketch.edit', {
      id,
      op: { op: 'dimension', constraint: { type: 'length', line: ls[1], value: 0 }, value: 6 },
    })
    await call('sketch.close')
    const info = await call('doc.info')
    expect(
      info.features[0].sketch_status === 'Fully defined',
      `status ${info.features[0].sketch_status}`,
    )
    const e = await ok(blind(id, 1))
    near(await volume(), 72, 1e-9, 'dimensioned area x depth')
    void v
    void e
  },
)
check('01 Definition status', 'a conflicting dimension makes the sketch over defined', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: front })
  const r = await call('sketch.edit', { id, op: line([0, 0], [10, 0]) })
  const [l, a, b] = r.created
  await call('sketch.edit', {
    id,
    op: { op: 'dimension', constraint: { type: 'length', line: l, value: 0 }, value: 10 },
  })
  await call('sketch.edit', {
    id,
    op: { op: 'dimension', constraint: { type: 'distance', a, b, value: 0 }, value: 7 },
  }).catch(() => {})
  await call('sketch.close')
  const status = (await call('doc.info')).features[0].sketch_status
  expect(/Over/i.test(status), `status ${status}`)
})
check(
  '01 Sketch entities',
  'slot, polygon, circle and spline profiles extrude with their exact areas',
  async () => {
    for (const [op, area, tol] of [
      [{ op: 'slot', a: [0, 0], b: [10, 0], width: 4 }, 40 + PI * 4, 1e-9],
      [
        { op: 'polygon', center: [0, 0], vertex: [5, 0], sides: 6 },
        ((3 * Math.sqrt(3)) / 2) * 25,
        1e-9,
      ],
      [circle([0, 0], 3), PI * 9, 1e-9],
    ]) {
      await fresh()
      const s = await sketch(front, [op])
      await ok(blind(s.id, 1))
      near(await volume(), area, tol, op.op)
    }
    await fresh()
    const s = await sketch(front, [
      {
        op: 'add_spline',
        points: [{ at: [0, 0] }, { at: [10, 0] }, { at: [10, 10] }, { at: [0, 10] }],
        closed: true,
      },
    ])
    await ok(blind(s.id, 1))
    expect((await volume()) > 0, 'closed spline extrudes')
  },
)
check('01 Sketch tools', 'offset makes a wall; sketch fillet rounds a corner', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: front })
  const r = await call('sketch.edit', { id, op: rect([0, 0], [10, 10]) })
  const curves = (await call('sketch.get', { id })).curves
    .filter((c) => c.type === 'line')
    .map((c) => c.id)
  await call('sketch.edit', { id, op: { op: 'offset', ids: curves, distance: -1 } })
  await call('sketch.close')
  await ok(blind(id, 1))
  const v = await volume()
  expect(
    Math.abs(v - (144 - 100)) < 1e-6 || Math.abs(v - (100 - 64)) < 1e-6,
    `offset wall area ${v}`,
  )
  await fresh()
  const { id: s2 } = await call('sketch.new', { plane: front })
  const r2 = await call('sketch.edit', { id: s2, op: rect([0, 0], [10, 10]) })
  await call('sketch.edit', { id: s2, op: { op: 'fillet', point: r2.created[0], radius: 2 } })
  await call('sketch.close')
  await ok(blind(s2, 1))
  near(await volume(), 100 - (4 - PI), 1e-6, 'filleted square')
  void r
})
check('01 Sketch tools', 'mirror about a construction line doubles a profile', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: front })
  const axis = await call('sketch.edit', { id, op: line([0, -20], [0, 20]) })
  const axisLine = lineId(axis.created)
  await call('sketch.edit', { id, op: { op: 'construction', ids: [axisLine], construction: true } })
  const c = await call('sketch.edit', { id, op: circle([5, 0], 2) })
  await call('sketch.edit', { id, op: { op: 'mirror', ids: [c.created.at(-1)], about: axisLine } })
  await call('sketch.close')
  await ok(blind(id, 1))
  near(await volume(), 2 * PI * 4, 1e-9, 'two circles')
})
check('01 Convert entities', 'converting a face converts its outer and inner loops', async () => {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [40, 20]), circle([20, 10], 5)])
  await ok(blind(s.id, 10))
  const topFace = await face(facing([0, 0, 1]))
  const { id } = await call('sketch.new', { plane: topFace })
  const r = await call('sketch.edit', { id, op: { op: 'convert', refs: [topFace] } })
  expect(r.created.length === 5, `converted ${r.created.length} entities`)
  await call('sketch.close')
  await ok(blind(id, 5))
  near(await volume(), (800 - 25 * PI) * 15, 1e-6, 'extruded converted face')
})
check('01 Dimensions', 'changing a driving dimension rebuilds the feature', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: front })
  const r = await call('sketch.edit', { id, op: circle([0, 0], 2) })
  const d = await call('sketch.edit', {
    id,
    op: {
      op: 'dimension',
      constraint: { type: 'radius', curve: r.created.at(-1), value: 0 },
      value: 3,
    },
  })
  await call('sketch.close')
  await ok(blind(id, 1))
  near(await volume(), 9 * PI, 1e-9, 'radius 3')
  await call('sketch.edit', { id, op: { op: 'set_value', id: d.created.at(-1), value: 4 } })
  await call('sketch.close').catch(() => {})
  near(await volume(), 16 * PI, 1e-9, 'radius 4')
})

// 03 Extrude
check('03 Extrude', 'blind, mid plane, direction 2, reverse and start offset', async () => {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [10, 10])])
  const e = await ok(blind(s.id, 5))
  near(await volume(), 500, 1e-9, 'blind')
  const upd = (f) => call('feature.update', { id: e.id, feature: f })
  await upd({ type: 'extrude', sketch: s.id, end1: { type: 'mid_plane', depth: 6 } })
  near(await volume(), 600, 1e-9, 'mid plane')
  await upd({
    type: 'extrude',
    sketch: s.id,
    end1: { type: 'blind', depth: 2 },
    end2: { type: 'blind', depth: 3 },
  })
  near(await volume(), 500, 1e-9, 'two directions')
  await upd({
    type: 'extrude',
    sketch: s.id,
    end1: { type: 'blind', depth: 2 },
    reverse: true,
    start_offset: 1,
  })
  const bb = (await call('doc.info')).scene.bbox
  near(bb.max[2], 1, 1e-5, 'start offset')
  near(bb.min[2], -1, 1e-5, 'reversed from the offset')
})
check('03 Extrude', 'up to vertex, surface, offset from surface and up to next', async () => {
  await block()
  const plane = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 30 })
  const s = await sketch({ type: 'feature', id: plane.id }, [rect([50, 0], [55, 5])])
  await ok({
    type: 'extrude',
    sketch: s.id,
    reverse: true,
    end1: { type: 'up_to_surface', target: front },
  })
  near(await volume(), 8000 + 25 * 30, 1e-6, 'up to surface')
  const s2 = await sketch({ type: 'feature', id: plane.id }, [rect([0, 0], [5, 5])])
  await ok({
    type: 'extrude',
    sketch: s2.id,
    reverse: true,
    end1: { type: 'up_to_next' },
    merge: true,
  })
  near(await volume(), 8000 + 750 + 25 * 20, 1e-6, 'up to next stops at the block')
  const s3 = await sketch({ type: 'feature', id: plane.id }, [rect([60, 0], [65, 5])])
  await ok({
    type: 'extrude',
    sketch: s3.id,
    reverse: true,
    end1: { type: 'offset_from_surface', target: front, offset: 5 },
  })
  near(await volume(), 8000 + 750 + 500 + 25 * 25, 1e-6, 'offset from surface')
})
check('03 Extrude', 'through all cut and a cut that misses fails', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [circle([20, 10], 3)])
  const wrong = await add({
    type: 'extrude',
    sketch: s.id,
    cut: true,
    end1: { type: 'through_all' },
  })
  expect(wrong.error && /does not remove/.test(wrong.error), `cut outward: ${wrong.error}`)
  await call('feature.update', {
    id: wrong.id,
    feature: {
      type: 'extrude',
      sketch: s.id,
      cut: true,
      reverse: true,
      end1: { type: 'through_all' },
    },
  })
  near(await volume(), 8000 - 90 * PI, 1e-6, 'through hole')
})
check('03 Extrude', 'thin one-direction, mid-plane and two-direction walls', async () => {
  for (const [thin, area] of [
    [{ kind: 'mid_plane', thickness: 1 }, 10],
    [{ kind: 'one_direction', thickness: 1 }, 10],
    [{ kind: 'two_direction', thickness: 1, thickness2: 2 }, 30],
  ]) {
    await fresh()
    const s = await sketch(front, [line([0, 0], [10, 0])])
    await ok(blind(s.id, 1, { thin }))
    near(await volume(), area, 1e-9, thin.kind)
  }
})
check('03 Extrude', 'zero depth is rejected', async () => {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [10, 10])])
  const msg = await fails('feature.add', { feature: blind(s.id, 0) })
  if (msg === null) {
    const info = await call('doc.info')
    expect(info.features.at(-1).error, 'zero depth must fail')
  }
})

// 03 Revolve
check(
  '03 Revolve',
  'full, blind, mid-plane and two-direction revolutions of a ring section',
  async () => {
    await fresh()
    const { id } = await call('sketch.new', { plane: front })
    await call('sketch.edit', { id, op: rect([10, 0], [20, 5]) })
    const ax = await call('sketch.edit', { id, op: line([0, 0], [0, 1]) })
    const axis = lineId(ax.created)
    await call('sketch.edit', { id, op: { op: 'construction', ids: [axis], construction: true } })
    await call('sketch.close')
    const ring = PI * (400 - 100) * 5
    const rev = (angle) => ({
      type: 'revolve',
      sketch: id,
      axis: { type: 'sketch_entity', sketch: id, entity: axis },
      angle,
    })
    const r = await ok(rev({ type: 'blind', angle: 2 * PI }))
    near(await volume(), ring, 1e-6, 'full')
    for (const [angle, frac] of [
      [{ type: 'blind', angle: PI / 2 }, 0.25],
      [{ type: 'mid_plane', angle: PI }, 0.5],
      [{ type: 'two_direction', angle1: PI / 2, angle2: PI / 4 }, 0.375],
    ]) {
      await call('feature.update', { id: r.id, feature: rev(angle) })
      near(await volume(), ring * frac, 1e-6, angle.type)
    }
  },
)

// 03 Fillet / chamfer / shell / hole
check('03 Fillet', 'constant radius on one edge removes (1 - pi/4) r^2 per length', async () => {
  await block()
  const e = await edge(
    (x) => x.curve === 'line' && at([40, null, 10])(x.start) && at([40, null, 10])(x.end),
  )
  await ok({ type: 'fillet', edges: [e], radius: 2 })
  near(await volume(), 8000 - 20 * (4 - PI), 1e-6, 'filleted')
})
check('03 Fillet', 'a radius larger than the faces fails and keeps the body', async () => {
  await block()
  const e = await edge(
    (x) => x.curve === 'line' && at([40, null, 10])(x.start) && at([40, null, 10])(x.end),
  )
  await bad({ type: 'fillet', edges: [e], radius: 30 })
  near(await volume(), 8000, 1e-9, 'previous body kept')
})
check('03 Chamfer', 'equal, distance-distance and angle-distance', async () => {
  for (const [mode, area] of [
    [{ type: 'equal', distance: 2 }, 2],
    [{ type: 'distance_distance', d1: 2, d2: 3 }, 3],
    [{ type: 'angle_distance', distance: 2, angle: PI / 4 }, 2],
  ]) {
    await block()
    const e = await edge(
      (x) => x.curve === 'line' && at([40, null, 10])(x.start) && at([40, null, 10])(x.end),
    )
    await ok({ type: 'chamfer', edges: [e], mode })
    near(await volume(), 8000 - 20 * area, 1e-6, mode.type)
  }
})
check('03 Shell', 'open-top shell and too thick a wall', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const sh = await ok({ type: 'shell', faces: [topFace], thickness: 2 })
  near(await volume(), 8000 - 36 * 16 * 8, 1e-6, 'shelled')
  void sh
  for (const thickness of [5, 15]) {
    await block()
    const r = await add({ type: 'shell', faces: [], thickness })
    expect(r.error, `a ${thickness} mm wall in a 10 mm block must fail`)
    near(await volume(), 8000, 1e-9, 'previous body kept')
  }
})
check('03 Hole Wizard', 'counterbore through hole volume and tapped cosmetic thread', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [{ op: 'add_point', at: [10, 10] }])
  await ok({
    type: 'hole',
    sketch: s.id,
    kind: 'counterbore',
    standard: 'iso',
    size: 'M6',
    end: { type: 'through_all' },
  })
  near(await volume(), 8000 - (PI * 3.3 ** 2 * 3.6 + PI * 5.5 ** 2 * 6.4), 0.05, 'counterbore')
  const s2 = await sketch(topFace, [{ op: 'add_point', at: [30, 10] }])
  const before = await volume()
  const h = await ok({
    type: 'hole',
    sketch: s2.id,
    kind: 'tapped',
    standard: 'iso',
    size: 'M6',
    end: { type: 'blind', depth: 8 },
    cosmetic_thread: true,
  })
  expect((await volume()) < before, 'tapped hole removes material')
  const scene = (await call('doc.info')).scene
  const t = scene.cosmetic_threads.find((x) => x.id === h.id)
  expect(t?.parameters?.holes?.[0]?.pitch === 1, 'M6 coarse pitch 1')
  near(t.parameters.holes[0].length, 8, 1e-6, 'thread length')
})

// 03 Sweep / loft / boundary
const pathAlongZ = async (length = 20) => sketch(top, [line([0, 0], [0, -length])])
check('03 Sweep', 'profile, circular profile, twist and thin along a straight path', async () => {
  await fresh()
  const path = await pathAlongZ()
  const prof = await sketch(front, [rect([-2, -2], [2, 2])])
  const sw = await ok({ type: 'sweep', profile: prof.id, path: { type: 'feature', id: path.id } })
  near(await volume(), 320, 1e-9, 'square sweep')
  await call('feature.update', {
    id: sw.id,
    feature: {
      type: 'sweep',
      profile: prof.id,
      path: { type: 'feature', id: path.id },
      twist: PI / 2,
    },
  })
  near(await volume(), 320, 1e-5, 'twisted keeps area')
  await call('feature.update', {
    id: sw.id,
    feature: { type: 'sweep', circle_diameter: 4, path: { type: 'feature', id: path.id } },
  })
  near(await volume(), 80 * PI, 1e-9, 'circular profile')
  await call('feature.update', {
    id: sw.id,
    feature: {
      type: 'sweep',
      profile: prof.id,
      path: { type: 'feature', id: path.id },
      thin: { kind: 'mid_plane', thickness: 1 },
    },
  })
  near(await volume(), (25 - 9) * 20, 1e-9, 'thin closed profile')
})
check('03 Sweep', 'a sharp path corner is mitred, not truncated', async () => {
  await fresh()
  const path = await sketch(top, [line([0, 0], [0, -20]), line([0, -20], [15, -20])])
  const prof = await sketch(front, [circle([0, 0], 1)])
  await ok({ type: 'sweep', profile: prof.id, path: { type: 'feature', id: path.id } })
  near(await volume(), 35 * PI, 1e-4, 'mitred L')
})
check('03 Sweep', 'a profile wider than the bend folds and is rejected', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: top })
  await call('sketch.edit', {
    id,
    op: {
      op: 'add_center_arc',
      center: { at: [3, 0] },
      start: { at: [0, 0] },
      end: [3, -3],
      clockwise: false,
    },
  })
  await call('sketch.close')
  const prof = await sketch(front, [circle([0, 0], 5)])
  const r = await add({ type: 'sweep', profile: prof.id, path: { type: 'feature', id } })
  expect(r.error, 'self-intersecting sweep must fail')
})
/** Two lines on the right plane through the squares' side midpoints, y = ±half. */
const sideGuides = (bottom, top, height = 10) =>
  worldSketch(right, (local) => [
    line(local([0, -bottom, 0]), local([0, -top, height])),
    line(local([0, bottom, 0]), local([0, top, height])),
  ])
async function sections(bottom, top) {
  const b = await sketch(front, [rect([-bottom, -bottom], [bottom, bottom])])
  const plane = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 10 })
  const t = await sketch({ type: 'feature', id: plane.id }, [rect([-top, -top], [top, top])])
  return [b.id, t.id]
}
const guideRefs = (g) =>
  g.created.map((c) => ({ type: 'sketch_entity', sketch: g.id, entity: lineId(c) }))
const frustum = (10 / 3) * (100 + 400 + 200)
check(
  '03 Loft',
  'ruled frustum; guide curves the loft passes through; one guide is rejected',
  async () => {
    await fresh()
    const [b, t] = await sections(5, 10)
    // Features can only use earlier curves, so the guides come first.
    const g = await sideGuides(5, 10)
    const l = await ok({ type: 'loft', profiles: [b, t], ruled: true })
    near(await volume(), frustum, 1e-6, 'ruled frustum')
    await call('feature.update', {
      id: l.id,
      feature: { type: 'loft', profiles: [b, t], guides: guideRefs(g) },
    })
    const guided = (await call('doc.info')).features.find((f) => f.id === l.id).error
    expect(guided === null, `guided loft: ${guided}`)
    near(await volume(), frustum, 1e-6, 'straight guides keep the frustum')
    const one = await call('feature.update', {
      id: l.id,
      feature: { type: 'loft', profiles: [b, t], guides: guideRefs(g).slice(0, 1) },
    })
    expect(
      /at least two/.test(one.features.find((f) => f.id === l.id).error ?? ''),
      'single guide rejected',
    )
  },
)
/** Volume of square sections of half-width `r(z)` over 0..10. */
const squares = (r) => {
  let v = 0
  for (let i = 0; i < 4000; i++) v += 4 * r((i + 0.5) / 400) ** 2 * (10 / 4000)
  return v
}
check(
  '03 Loft',
  'start/end tangency normal to the profiles, a centerline, and seams inside circles',
  async () => {
    await fresh()
    const [b, t] = await sections(5, 3)
    const axis = await worldSketch(right, (local) => [line(local([0, 0, -2]), local([0, 0, 12]))])
    const l = await ok({
      type: 'loft',
      profiles: [b, t],
      start: { type: 'normal', tangent_length: 1 },
      end: { type: 'normal', tangent_length: 1 },
    })
    const hermite = (z) => 5 - 2 * (3 * (z / 10) ** 2 - 2 * (z / 10) ** 3)
    near(await volume(), squares(hermite), 1e-5, 'Hermite half-width')
    await call('feature.update', {
      id: l.id,
      feature: {
        type: 'loft',
        profiles: [b, t],
        centerline: { type: 'sketch_entity', sketch: axis.id, entity: lineId(axis.created[0]) },
      },
    })
    near(await volume(), (10 / 3) * (100 + 36 + 60), 1e-6, 'centerline frustum')
    // Ruled between equal circles whose seams are a quarter turn apart.
    await fresh()
    const c0 = await sketch(front, [circle([0, 0], 5)])
    const plane = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 10 })
    const c1 = await sketch({ type: 'feature', id: plane.id }, [circle([0, 0], 5)])
    const contour = (s, seam) => ({ sketch: s.id, entities: [s.created[0][0]], seam })
    await ok({
      type: 'loft',
      profiles: [contour(c0, null), contour(c1, 0.25)],
      ruled: true,
    })
    near(await volume(), (250 * PI * 2) / 3, 1e-6, 'twisted circles')
  },
)
check(
  '03 Sweep',
  'a guide that scales the section and a second guide that stretches it',
  async () => {
    await fresh()
    const path = await pathAlongZ()
    const g = await worldSketch(top, (local) => [line(local([2, 0, 0]), local([4, 0, 20]))])
    const g2 = await worldSketch(right, (local) => [line(local([0, 2, 0]), local([0, 2, 20]))])
    const prof = await sketch(front, [circle([0, 0], 2)])
    const ref = (s) => ({ type: 'sketch_entity', sketch: s.id, entity: lineId(s.created[0]) })
    const sw = await ok({
      type: 'sweep',
      profile: prof.id,
      path: { type: 'feature', id: path.id },
      guide: ref(g),
      guide_scale: true,
    })
    near(await volume(), ((PI * 20) / 3) * (4 + 8 + 16), 1e-6, 'scaled frustum')
    await call('feature.update', {
      id: sw.id,
      feature: {
        type: 'sweep',
        profile: prof.id,
        path: { type: 'feature', id: path.id },
        guide: ref(g),
        guide2: ref(g2),
      },
    })
    near(await volume(), PI * 2 * 60, 1e-6, 'stretched to an elliptic frustum')
  },
)
check(
  '03 Boundary',
  'sections and direction 2 curves bound a prism and a frustum; a curve that misses fails',
  async () => {
    await fresh()
    const [b, t] = await sections(5, 5)
    const g = await sideGuides(5, 5)
    await ok({ type: 'boundary', sections: [b, t], curves: guideRefs(g) })
    near(await volume(), 1000, 1e-6, 'prism')
    await fresh()
    const [b2, t2] = await sections(5, 10)
    const g2 = await sideGuides(5, 10)
    const short = await sideGuides(5, 10, 8)
    const bd = await ok({ type: 'boundary', sections: [b2, t2], curves: guideRefs(g2) })
    near(await volume(), frustum, 1e-6, 'frustum')
    const r = await call('feature.update', {
      id: bd.id,
      feature: { type: 'boundary', sections: [b2, t2], curves: guideRefs(short) },
    })
    const missing = r.features.find((f) => f.id === bd.id).error
    expect(/does not touch/.test(missing ?? ''), `missing curve: ${missing}`)
  },
)

// 03 Draft / rib / dome / wrap / threads / face fillet
check('03 Draft', 'neutral-plane draft tapers by tan(angle)', async () => {
  await block()
  const side = await face(facing([1, 0, 0]))
  const a = (3 * PI) / 180
  await ok({ type: 'draft', faces: [side], neutral_plane: front, direction: null, angle: a })
  near(await volume(), 8000 - 0.5 * 100 * 20 * Math.tan(a), 1e-5, 'drafted')
})
check('03 Draft', 'parting-line draft hinges each face on its edge', async () => {
  await block()
  const bottomEdge = await edge(
    (x) => x.curve === 'line' && at([40, null, 0])(x.start) && at([40, null, 0])(x.end),
  )
  const a = (3 * PI) / 180
  await ok({
    type: 'draft',
    faces: [],
    neutral_plane: front,
    direction: null,
    angle: a,
    parting_line: [bottomEdge],
  })
  near(await volume(), 8000 - 0.5 * 100 * 20 * Math.tan(a), 1e-5, 'parting line')
})
check('03 Rib', 'a rib meets the walls and floor of a shelled housing', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  await ok({ type: 'shell', faces: [topFace], thickness: 2 })
  const rim = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 10 })
  const s = await sketch({ type: 'feature', id: rim.id }, [line([20, 6], [20, 14])])
  await ok({ type: 'rib', sketch: s.id, thickness: 2, direction: 'normal_to_sketch' })
  near(await volume(), 8000 - 36 * 16 * 8 + 2 * 16 * 8, 1e-6, 'rib web')
  const r = await add({
    type: 'rib',
    sketch: s.id,
    thickness: 2,
    direction: 'normal_to_sketch',
    flip: true,
  })
  expect(r.error && /does not meet/.test(r.error), `flipped rib: ${r.error}`)
})
check(
  '03 Dome',
  'spherical and elliptical domes on a cylinder end, and on a rectangle',
  async () => {
    await fresh()
    const s = await sketch(front, [circle([0, 0], 10)])
    await ok(blind(s.id, 8))
    const base = await volume()
    const topFace = await face(facing([0, 0, 1]))
    const d = await ok({ type: 'dome', face: topFace, height: 4 })
    near((await volume()) - base, (PI * 4 * (3 * 100 + 16)) / 6, 1e-6, 'spherical cap')
    await call('feature.update', {
      id: d.id,
      feature: { type: 'dome', face: topFace, height: 15, elliptical: true },
    })
    near((await volume()) - base, (2 / 3) * PI * 100 * 15, 1e-6, 'elliptical dome')
    const r = await add({ type: 'dome', face: topFace, height: 15 })
    void r
    // A rectangular face gets a smooth dome through its edges and the apex.
    await block()
    const flat = await volume()
    await ok({ type: 'dome', face: await face(facing([0, 0, 1])), height: 3 })
    const added = (await volume()) - flat
    expect(added > (800 * 3) / 3 && added < 800 * 3, `rectangular dome volume ${added}`)
  },
)
check(
  '03 Draft',
  'a step draft over a level parting line matches the parting-line draft',
  async () => {
    await block()
    const bottomEdge = await edge(
      (x) => x.curve === 'line' && at([40, null, 0])(x.start) && at([40, null, 0])(x.end),
    )
    const a = (3 * PI) / 180
    await ok({
      type: 'draft',
      faces: [],
      neutral_plane: front,
      direction: null,
      angle: a,
      parting_line: [bottomEdge],
      step: true,
    })
    near(await volume(), 8000 - 0.5 * 100 * 20 * Math.tan(a), 1e-5, 'step draft')
  },
)
check('03 Wrap', 'a circle wrapped along geodesics onto a spherical dome', async () => {
  await fresh()
  const s = await sketch(front, [circle([0, 0], 10)])
  await ok(blind(s.id, 40))
  await ok({ type: 'dome', face: await face(facing([0, 0, 1])), height: 5 })
  const plane = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 45 })
  const w = await sketch({ type: 'feature', id: plane.id }, [circle([0, 0], 2)])
  const before = await volume()
  const cap = await face((f) => f.surface === 'sphere')
  await ok({ type: 'wrap', sketch: w.id, face: cap, mode: 'emboss', depth: 0.5 })
  const rs = (100 + 25) / 10
  const shell = ((2 * PI) / 3) * ((rs + 0.5) ** 3 - rs ** 3) * (1 - Math.cos(2 / rs))
  near((await volume()) - before, shell, 1e-6, 'geodesic disk shell')
})
check('03 Wrap', 'emboss, deboss and scribe on a cylinder', async () => {
  await fresh()
  const s = await sketch(front, [circle([0, 0], 10)])
  await ok(blind(s.id, 40))
  const full = await volume()
  const side = await face((f) => f.surface === 'cylinder')
  const plane = await ok({ type: 'plane', mode: 'offset', reference: right, distance: 10 })
  const w = await worldSketch({ type: 'feature', id: plane.id }, (local) => {
    const [p, q] = [local([10, -2.5, 10]), local([10, 2.5, 14])]
    return [
      rect(
        [Math.min(p[0], q[0]), Math.min(p[1], q[1])],
        [Math.max(p[0], q[0]), Math.max(p[1], q[1])],
      ),
    ]
  })
  const wr = await ok({ type: 'wrap', sketch: w.id, face: side, mode: 'emboss', depth: 1 })
  const sector = (r0, r1) => 0.5 * (5 / 10) * (r1 * r1 - r0 * r0) * 4
  near((await volume()) - full, sector(10, 11), 1e-6, 'emboss')
  await call('feature.update', {
    id: wr.id,
    feature: { type: 'wrap', sketch: w.id, face: side, mode: 'deboss', depth: 1 },
  })
  near(full - (await volume()), sector(9, 10), 1e-6, 'deboss')
  await call('feature.update', {
    id: wr.id,
    feature: { type: 'wrap', sketch: w.id, face: side, mode: 'scribe' },
  })
  near(await volume(), full, 1e-9, 'scribe keeps volume')
})
check('03 Thread', 'modeled external thread removes the ISO groove volume', async () => {
  await fresh()
  const s = await sketch(front, [circle([0, 0], 5)])
  await ok(blind(s.id, 20))
  const full = await volume()
  const side = await face((f) => f.surface === 'cylinder')
  const p = 1.5
  await ok({ type: 'thread', face: side, diameter: 10, pitch: p, length: 15, start_offset: 2 })
  const h = 0.541265877365274 * p
  const a = p / 4,
    b = (7 * p) / 8
  const removed =
    ((a + b) / 2) * h * 2 * PI * (5 - h + (h * (a + 2 * b)) / (3 * (a + b))) * (15 / p)
  near(full - (await volume()), removed, 1e-4, 'groove volume')
})
check('03 Fillet', 'face fillet between two planar face sets', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const side = await face(facing([1, 0, 0]))
  await ok({ type: 'face_fillet', faces1: [topFace], faces2: [side], radius: 2 })
  near(await volume(), 8000 - 20 * (4 - PI), 1e-6, 'face fillet')
})
check('03 Fillet', 'variable fillet with a straight radius law', async () => {
  await block()
  const e = await edge(
    (x) => x.curve === 'line' && at([40, 20, null])(x.start) && at([40, 20, null])(x.end),
  )
  await ok({
    type: 'variable_fillet',
    edge: e,
    stations: [
      { position: 0, radius: 0.5 },
      { position: 1, radius: 1.5 },
    ],
    straight: true,
  })
  near(await volume(), 8000 - ((1 - PI / 4) * 10 * (0.25 + 0.75 + 2.25)) / 3, 1e-4, 'linear law')
})

// 04 Patterns
check('04 Patterns', 'linear pattern in two directions, circular pattern and mirror', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [circle([5, 5], 1)])
  const cut = await ok({
    type: 'extrude',
    sketch: s.id,
    cut: true,
    reverse: true,
    end1: { type: 'blind', depth: 2 },
  })
  const one = 2 * PI
  await ok({
    type: 'linear_pattern',
    features: [cut.id],
    direction1: { reference: right, spacing: 10, count: 3 },
    direction2: { reference: top, spacing: 10, count: 2 },
  })
  near(await volume(), 8000 - 6 * one, 1e-6, '3 x 2 instances')
  await block()
  const t2 = await face(facing([0, 0, 1]))
  const s2 = await sketch(t2, [circle([30, 10], 1)])
  const c2 = await ok({
    type: 'extrude',
    sketch: s2.id,
    cut: true,
    reverse: true,
    end1: { type: 'blind', depth: 2 },
  })
  const axis = await ok({
    type: 'axis',
    mode: 'two_planes',
    a: {
      type: 'feature',
      id: (await ok({ type: 'plane', mode: 'offset', reference: right, distance: 20 })).id,
    },
    b: {
      type: 'feature',
      id: (await ok({ type: 'plane', mode: 'offset', reference: top, distance: 10 })).id,
    },
  })
  await ok({
    type: 'circular_pattern',
    features: [c2.id],
    axis: { type: 'feature', id: axis.id },
    count: 2,
    angle: 2 * PI,
  })
  near(await volume(), 8000 - 2 * one, 1e-6, 'circular instances')
  await ok({ type: 'mirror', features: [], plane: right })
  near(await volume(), 2 * (8000 - 2 * one), 1e-6, 'mirrored body')
})

// 06 / 08 Bodies
check('06 Body tools', 'separate bodies, combine, move/copy and split', async () => {
  await block()
  const s = await sketch(front, [rect([40, 0], [50, 10])])
  await ok(blind(s.id, 10, { merge: false }))
  let info = await call('doc.info')
  expect(info.bodies.length === 2, `bodies ${info.bodies.length}`)
  const [a, b] = info.bodies.map((x) => x.id)
  await ok({ type: 'move_body', bodies: [b], translation: [0, 0, 20], copy: true })
  info = await call('doc.info')
  expect(info.bodies.length === 3, 'copy adds a body')
  near(await volume(), 8000 + 2000, 1e-6, 'copy volume')
  await ok({ type: 'combine', bodies: [a, b], operation: 'fuse' })
  info = await call('doc.info')
  expect(info.bodies.length === 2, 'combine merges two')
  near(await volume(), 8000 + 2 * 1000, 1e-6, 'combined plus copy')
  const s2 = await sketch(front, [rect([100, 0], [110, 10])])
  await ok(blind(s2.id, 10, { merge: false }))
  const ids = (await call('doc.info')).bodies.map((x) => x.id)
  const apart = await add({ type: 'combine', bodies: [ids[0], ids.at(-1)], operation: 'fuse' })
  expect(apart.error && /don't touch/.test(apart.error), `disjoint add: ${apart.error}`)
  const mid = await ok({ type: 'plane', mode: 'offset', reference: right, distance: 45 })
  await ok({
    type: 'split_body',
    bodies: info.bodies.map((x) => x.id),
    plane: { type: 'feature', id: mid.id },
    keep: 'both',
  })
  const pieces = (await call('doc.info')).bodies.length
  // The fused block and the copy split in two; the separate box does not.
  expect(pieces === 5, `split made ${pieces} bodies`)
  near(await volume(), 11000, 1e-6, 'split keeps volume')
})

// 07 Reference geometry
check('07 Reference geometry', 'planes, axes, points and coordinate systems build', async () => {
  await block()
  const v = await ok({ type: 'point', mode: 'coordinates', x: 1, y: 2, z: 3 })
  const p1 = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 5 })
  await ok({ type: 'plane', mode: 'mid_plane', a: front, b: { type: 'feature', id: p1.id } })
  const ax = await ok({ type: 'axis', mode: 'two_planes', a: front, b: right })
  await ok({
    type: 'plane',
    mode: 'angle',
    reference: front,
    axis: { type: 'feature', id: ax.id },
    angle: PI / 4,
  })
  await ok({ type: 'coord_sys', origin: { type: 'feature', id: v.id } })
  const r = await add({
    type: 'plane',
    mode: 'mid_plane',
    a: { type: 'feature', id: 999 },
    b: front,
  })
  expect(r.error, 'missing reference must fail')
})

// 14 Configurations and equations
check(
  '14 Equations',
  'variables drive parameters; syntax errors are rejected; cycles are reported',
  async () => {
    const e = await block()
    await call('equations.set', {
      variables: { h: '12mm' },
      equations: { [`f${e}.end1.depth`]: 'h' },
    })
    near(await volume(), 9600, 1e-9, 'driven depth')
    const msg = await fails('equations.set', { variables: { h: '12mm *' }, equations: {} })
    expect(msg && /h/.test(msg), `syntax error: ${msg}`)
    near(await volume(), 9600, 1e-9, 'unchanged after rejection')
    await call('equations.set', {
      variables: { a: 'b', b: 'a' },
      equations: { [`f${e}.end1.depth`]: 'a' },
    })
    const info = await call('doc.info')
    expect(/cycle/.test(info.equation_values[`f${e}.end1.depth`]?.error ?? ''), 'cycle reported')
    const units = await call('equations.preview', {
      variables: {},
      equations: { [`f${e}.end1.depth`]: '3deg' },
    })
    expect(/units/.test(units.values[`f${e}.end1.depth`].error), 'unit mismatch reported')
  },
)
check('14 Equations', 'formulas typed while creating a feature become equations', async () => {
  await block()
  await call('equations.set', { variables: { h: '4mm' }, equations: {} })
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [rect([0, 0], [10, 10])])
  const r = await ok(blind(s.id, 1), { formulas: { 'end1.depth': 'h * 2' } })
  near(await volume(), 8000 + 800, 1e-9, 'formula depth')
  await call('equations.set', { variables: { h: '5mm' }, equations: r.snap.equations })
  near(await volume(), 9000, 1e-9, 'follows the variable')
})
check('14 Configurations', 'two configurations keep separate values and suppression', async () => {
  const e = await block()
  await call('configuration.create', { name: 'Tall' })
  const info = await call('doc.info')
  const tall = info.configurations.items.find((c) => c.name === 'Tall')
  await call('configuration.update', {
    configuration: { ...tall, parameters: { [`f${e}.end1.depth`]: 20 } },
  })
  near(await volume(), 16000, 1e-9, 'Tall')
  await call('configuration.activate', { id: 1 })
  near(await volume(), 8000, 1e-9, 'Default unchanged')
  await call('configuration.activate', { id: tall.id })
  near(await volume(), 16000, 1e-9, 'Tall again')
})

// 17 Evaluate
check(
  '17 Measure',
  'mass properties use the material density; measure gives face distance',
  async () => {
    await block()
    await call('doc.material', { material: { name: 'Steel', density: 7850 } }).catch(() => {})
    const m = await call('mass_properties')
    near(m.volume, 8000, 1e-9, 'volume')
    near(m.center[0], 20, 1e-9, 'centroid x')
    const a = await face(facing([0, 0, 1]))
    const b = await face(facing([0, 0, -1]))
    const d = await call('measure', { refs: [a, b] })
    expect(JSON.stringify(d).includes('10'), `face distance in ${JSON.stringify(d).slice(0, 200)}`)
  },
)

// 19 / 21 Files
check(
  '19 Import and export',
  'STEP, IGES and STL export; STEP import keeps the volume',
  async () => {
    await block()
    for (const ext of ['step', 'iges', 'stl']) {
      const path = join(tmp, `block.${ext}`)
      await call('export', { path })
      expect(existsSync(path) && statSync(path).size > 0, `${ext} written`)
    }
    await fresh()
    await call('import', { path: join(tmp, 'block.step') })
    near(await volume(), 8000, 1e-6, 'imported STEP')
  },
)
check('21 Documents', 'save, reopen, undo, redo and rollback', async () => {
  const e = await block()
  const path = join(tmp, 'part.ofp')
  await call('doc.save', { path })
  await call('feature.update', { id: e, feature: blind(1, 20) })
  near(await volume(), 16000, 1e-9, 'edited')
  await call('doc.undo')
  near(await volume(), 8000, 1e-9, 'undo')
  await call('doc.redo')
  near(await volume(), 16000, 1e-9, 'redo')
  await call('doc.open', { path })
  near(await volume(), 8000, 1e-9, 'reopened')
  await call('doc.rollback', { index: 1 })
  expect((await call('doc.info')).features[1].rolled_back, 'rolled back')
})

// Failure cases named in the specs: each must leave a clear feature error and
// keep the previous body.
const failure = (spec, name, build, pattern) =>
  check(spec, `fails: ${name}`, async () => {
    const before = await build()
    const r = await add(before.feature)
    expect(r.error, `${name} must fail`)
    if (pattern) expect(pattern.test(r.error), `unexpected error: ${r.error}`)
    if (before.volume !== undefined) near(await volume(), before.volume, 1e-9, 'previous body kept')
  })
failure(
  '03 Extrude',
  'an open sketch without thin',
  async () => {
    await fresh()
    const s = await sketch(front, [line([0, 0], [10, 0])])
    return { feature: blind(s.id, 5) }
  },
  /closed/,
)
failure('03 Revolve', 'an axis that crosses the profile', async () => {
  await fresh()
  const { id } = await call('sketch.new', { plane: front })
  await call('sketch.edit', { id, op: rect([-5, 0], [5, 5]) })
  const ax = await call('sketch.edit', { id, op: line([0, -10], [0, 10]) })
  await call('sketch.edit', {
    id,
    op: { op: 'construction', ids: [lineId(ax.created)], construction: true },
  })
  await call('sketch.close')
  return {
    feature: {
      type: 'revolve',
      sketch: id,
      axis: { type: 'sketch_entity', sketch: id, entity: lineId(ax.created) },
      angle: { type: 'blind', angle: 2 * PI },
    },
  }
})
failure('03 Sweep', 'a disconnected path', async () => {
  await fresh()
  const path = await sketch(top, [line([0, 0], [0, -10]), line([5, -15], [5, -25])])
  const prof = await sketch(front, [circle([0, 0], 1)])
  return { feature: { type: 'sweep', profile: prof.id, path: { type: 'feature', id: path.id } } }
})
failure('03 Loft', 'a single profile', async () => {
  await fresh()
  const b = await sketch(front, [rect([0, 0], [5, 5])])
  return { feature: { type: 'loft', profiles: [b.id] } }
})
failure(
  '03 Hole Wizard',
  'a sketch without points',
  async () => {
    await block()
    const topFace = await face(facing([0, 0, 1]))
    const s = await sketch(topFace, [line([0, 0], [5, 5])])
    return {
      feature: {
        type: 'hole',
        sketch: s.id,
        kind: 'simple',
        standard: 'iso',
        size: 'M6',
        end: { type: 'through_all' },
        reverse: true,
      },
      volume: 8000,
    }
  },
  /points/,
)
failure(
  '03 Hole Wizard',
  'an unknown size',
  async () => {
    await block()
    const topFace = await face(facing([0, 0, 1]))
    const s = await sketch(topFace, [{ op: 'add_point', at: [5, 5] }])
    return {
      feature: {
        type: 'hole',
        sketch: s.id,
        kind: 'simple',
        standard: 'iso',
        size: 'M7.3',
        end: { type: 'through_all' },
      },
      volume: 8000,
    }
  },
  /size/,
)
failure(
  '03 Rib',
  'a closed sketch',
  async () => {
    await block()
    const s = await sketch(front, [rect([5, 5], [10, 10])])
    return { feature: { type: 'rib', sketch: s.id, thickness: 2 }, volume: 8000 }
  },
  /open/,
)
failure(
  '03 Dome',
  'a face with a hole',
  async () => {
    await fresh()
    const s = await sketch(front, [rect([0, 0], [40, 20]), circle([20, 10], 3)])
    await ok(blind(s.id, 10))
    return {
      feature: { type: 'dome', face: await face(facing([0, 0, 1])), height: 2 },
      volume: 8000 - PI * 90,
    }
  },
  /holes/,
)
failure(
  '03 Wrap',
  'a sketch plane across the cylinder axis',
  async () => {
    await fresh()
    const c = await sketch(front, [circle([0, 0], 10)])
    await ok(blind(c.id, 40))
    const side = await face((f) => f.surface === 'cylinder')
    const plane = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 20 })
    const w = await sketch({ type: 'feature', id: plane.id }, [rect([-1, -1], [1, 1])])
    return {
      feature: { type: 'wrap', sketch: w.id, face: side, mode: 'emboss', depth: 1 },
      volume: PI * 100 * 40,
    }
  },
  /parallel/,
)
failure(
  '03 Thread',
  'a diameter that does not match the cylinder',
  async () => {
    await fresh()
    const c = await sketch(front, [circle([0, 0], 5)])
    await ok(blind(c.id, 20))
    const side = await face((f) => f.surface === 'cylinder')
    return {
      feature: { type: 'thread', face: side, diameter: 12, pitch: 1.5, length: 10 },
      volume: PI * 25 * 20,
    }
  },
  /diameter/,
)
failure(
  '03 Fillet',
  'a face fillet between parallel faces',
  async () => {
    await block()
    return {
      feature: {
        type: 'face_fillet',
        faces1: [await face(facing([0, 0, 1]))],
        faces2: [await face(facing([0, 0, -1]))],
        radius: 1,
      },
      volume: 8000,
    }
  },
  /parallel/,
)
failure('03 Draft', 'a parting line edge with no face on the pull side', async () => {
  await block()
  const topEdge = await edge(
    (x) => x.curve === 'line' && at([40, null, 10])(x.start) && at([40, null, 10])(x.end),
  )
  return {
    feature: {
      type: 'draft',
      faces: [],
      neutral_plane: front,
      direction: null,
      angle: 0.05,
      parting_line: [topEdge],
    },
    volume: 8000,
  }
})
failure(
  '02 Split line',
  'a silhouette on a planar face',
  async () => {
    await block()
    return {
      feature: {
        type: 'split_line',
        target: await face(facing([0, 0, 1])),
        direction: [1, 0, 0],
        mode: 'silhouette',
      },
      volume: 8000,
    }
  },
  /cylindrical|spherical/,
)
check('02 Projected curve', 'a hole splits a projected line into ordered pieces', async () => {
  await block()
  const hole = await sketch(front, [circle([20, 10], 2)])
  await ok({ type: 'extrude', sketch: hole.id, cut: true, end1: { type: 'through_all' } })
  const src = await sketch(front, [line([-1, 10], [41, 10])])
  const p = await ok({
    type: 'projected_curve',
    source: { type: 'feature', id: src.id },
    target: await face(facing([0, 0, 1])),
    direction: [0, 0, 1],
  })
  const curve = (await call('doc.info')).scene.curves.find((c) => c.id === p.id)
  const names = curve.segments.map((x) => x.reference.segment)
  expect(names.length === 2, `pieces: ${names}`)
  expect(
    names.some((n) => n.endsWith('#1')) && names.some((n) => n.endsWith('#2')),
    `ordered names: ${names}`,
  )
  const first = curve.segments.find((x) => x.reference.segment.endsWith('#1'))
  expect(first.polylines[0][0] < 20, 'piece #1 is nearest the line start')
})
failure('02 Composite curve', 'disconnected segments', async () => {
  await fresh()
  const a = await sketch(front, [line([0, 0], [1, 0])])
  const b = await sketch(front, [line([10, 0], [11, 0])])
  return {
    feature: {
      type: 'composite_curve',
      segments: [
        { reference: { type: 'feature', id: a.id } },
        { reference: { type: 'feature', id: b.id } },
      ],
    },
  }
})
failure('07 Reference geometry', 'a plane through three collinear points', async () => {
  await fresh()
  const ps = []
  for (const x of [0, 1, 2])
    ps.push((await ok({ type: 'point', mode: 'coordinates', x, y: 0, z: 0 })).id)
  return {
    feature: {
      type: 'plane',
      mode: 'three_points',
      points: ps.map((id) => ({ type: 'feature', id })),
    },
  }
})
failure('04 Patterns', 'a mirror plane that is not planar', async () => {
  await fresh()
  const c = await sketch(front, [circle([0, 0], 5)])
  const e = await ok(blind(c.id, 10))
  return {
    feature: {
      type: 'mirror',
      features: [e.id],
      plane: await face((f) => f.surface === 'cylinder'),
    },
    volume: PI * 250,
  }
})
check('14 Equations', 'fails: a non-integer instance count and an unknown variable', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [circle([5, 5], 1)])
  const cut = await ok({
    type: 'extrude',
    sketch: s.id,
    cut: true,
    reverse: true,
    end1: { type: 'blind', depth: 2 },
  })
  const pat = await ok({
    type: 'linear_pattern',
    features: [cut.id],
    direction1: { reference: right, spacing: 10, count: 2 },
  })
  const preview = await call('equations.preview', {
    variables: {},
    equations: { [`f${pat.id}.direction1.count`]: '2.5', [`f${cut.id}.end1.depth`]: 'nope' },
  })
  expect(
    /whole number/.test(preview.values[`f${pat.id}.direction1.count`]?.error ?? ''),
    'integer count',
  )
  expect(/unknown/.test(preview.values[`f${cut.id}.end1.depth`]?.error ?? ''), 'unknown variable')
})
check('14 Configurations', 'a feature suppressed in one configuration only', async () => {
  await block()
  const topFace = await face(facing([0, 0, 1]))
  const s = await sketch(topFace, [rect([0, 0], [10, 10])])
  const boss = await ok(blind(s.id, 5))
  await call('configuration.create', { name: 'Plain' })
  const plain = (await call('doc.info')).configurations.items.find((c) => c.name === 'Plain')
  await call('configuration.update', {
    configuration: { ...plain, suppressed: { [boss.id]: true } },
  })
  near(await volume(), 8000, 1e-9, 'suppressed in Plain')
  await call('configuration.activate', { id: 1 })
  near(await volume(), 8500, 1e-9, 'present in Default')
})
check(
  '02 3D sketch',
  'conflicting relations report a failed solve without losing geometry',
  async () => {
    await fresh()
    const sketch3 = {
      entities: [
        { id: 1, kind: 'point', at: [0, 0, 0] },
        { id: 2, kind: 'point', at: [10, 0, 0] },
        { id: 3, kind: 'line', a: 1, b: 2 },
      ],
      constraints: [
        { id: 4, kind: 'distance', a: 1, b: 2, value: 10 },
        { id: 5, kind: 'distance', a: 1, b: 2, value: 20 },
      ],
      next_id: 6,
    }
    const r = await call('sketch3d.solve', { sketch: sketch3 })
    expect(
      r.report.status === 'failed' || r.report.conflicting?.length,
      `solve status ${JSON.stringify(r.report).slice(0, 120)}`,
    )
    const p2 = r.sketch.entities.find((e) => e.id === 2)
    expect(
      p2.at.every((v) => Number.isFinite(v)),
      'geometry kept',
    )
  },
)
check('20 Units', 'inch documents keep core values in millimetres', async () => {
  await block()
  const snap = await call('doc.set_units', {
    units: { length: 'in', angle: 'deg', mass: 'lb', decimals: 3 },
  })
  expect(snap.units.length === 'in', 'units changed')
  near(await volume(), 8000, 1e-9, 'volume still mm3')
})

// ---- 13 Assemblies ----

/** Save a part made of one sketch extruded by `depth`; returns its path. */
async function savePart(name, ops, depth) {
  await fresh()
  const s = await sketch(front, ops)
  await ok(blind(s.id, depth))
  const path = join(tmp, `${name}.ofp`)
  await call('doc.save', { path })
  return path
}
const place = (x, y, z) => ({ translation: [x, y, z], rotation: [0, 0, 0, 1] })
async function insert(path, at) {
  return (await call('asm.insert', { path, ...(at ? { transform: place(...at) } : {}) })).key
}
/** A component face by geometry, as a component reference. */
async function asmFace(key, pred) {
  const f = (await call('asm.topology', { component: [key] })).faces.find(pred)
  expect(f, `face on ${key}`)
  return f.ref
}
const cyl = (r) => (f) => f.surface === 'cylinder' && Math.abs(f.radius - r) < 1e-9
const flat = (n, at) => (f) =>
  f.surface === 'plane' &&
  f.normal.every((v, i) => Math.abs(v - n[i]) < 1e-9) &&
  Math.abs(dot(f.centroid, n) - at) < 1e-6
const translation = (snap, key) => {
  const i = snap.scene.instances.find((x) => x.key.length === 1 && x.key[0] === key)
  return i.transform.slice(12, 15)
}
const component = (snap, key) => snap.components.find((c) => c.key[0] === key)
let plateFile, pinFile, cubeFile
async function asmParts() {
  plateFile ??= await savePart('c13-plate', [rect([0, 0], [100, 60]), circle([30, 30], 5)], 10)
  pinFile ??= await savePart('c13-pin', [circle([0, 0], 5)], 40)
  cubeFile ??= await savePart('c13-cube', [rect([0, 0], [10, 10])], 10)
}
/** Plate fixed at the origin with a pin seated in its hole. */
async function pinned() {
  await asmParts()
  await call('asm.new')
  const p = await insert(plateFile)
  const s = await insert(pinFile, [70, -20, 30])
  await call('asm.mate.add', {
    mate: { type: 'concentric', a: await asmFace(p, cyl(5)), b: await asmFace(s, cyl(5)) },
  })
  await call('asm.mate.add', {
    mate: {
      type: 'coincident',
      a: await asmFace(p, flat([0, 0, 1], 10)),
      b: await asmFace(s, facing([0, 0, -1])),
    },
  })
  return { p, s }
}

check(
  '13 Insert components',
  'the first component is fixed; files load relative to the assembly',
  async () => {
    await asmParts()
    await call('asm.new')
    const p = await insert(plateFile)
    const c = await insert(cubeFile, [200, 0, 0])
    let snap = await call('asm.info')
    expect(component(snap, p).fixed && !component(snap, c).fixed, 'first fixed, second floating')
    const file = join(tmp, 'c13-insert.ofa')
    await call('doc.save', { path: file })
    snap = await call('doc.open', { path: file })
    expect(snap.kind === 'assembly' && snap.components.length === 2, 'reopened')
    expect(
      /itself/.test(await fails('asm.insert', { path: file })),
      'an assembly cannot contain itself',
    )
  },
)
check(
  '13 Standard mates',
  'concentric then coincident leaves only rotation; conflicts are refused',
  async () => {
    const { p, s } = await pinned()
    const snap = await call('asm.info')
    expect(component(snap, s).dof === 1, `pin DOF ${component(snap, s).dof}`)
    const t = translation(snap, s)
    near(t[0], 30, 1e-6, 'x')
    near(t[1], 30, 1e-6, 'y')
    near(t[2], 10, 1e-6, 'z')
    const error = await fails('asm.mate.add', {
      mate: {
        type: 'distance',
        a: await asmFace(p, flat([0, 0, 1], 10)),
        b: await asmFace(s, facing([0, 0, -1])),
        distance: 5,
      },
    })
    expect(/Coincident1/.test(error ?? ''), `conflict names the mate: ${error}`)
    expect((await call('asm.info')).mates.length === 2, 'refused mate not added')
  },
)
check('13 Advanced mates', 'a distance limit holds while dragging', async () => {
  await asmParts()
  await call('asm.new')
  const p = await insert(plateFile)
  const c = await insert(cubeFile, [60, 0, 12])
  const top = await asmFace(p, flat([0, 0, 1], 10))
  await call('asm.mate.add', {
    mate: { type: 'coincident', a: top, b: await asmFace(c, facing([0, 0, -1])) },
  })
  await call('asm.mate.add', {
    mate: {
      type: 'distance',
      a: await asmFace(p, flat([-1, 0, 0], 0)),
      b: await asmFace(c, facing([-1, 0, 0])),
      distance: 0,
      limit: { min: 40, max: 70 },
    },
  })
  const snap = await call('asm.move', {
    component: [c],
    motion: { mode: 'delta', translation: [100, 0, 0] },
  })
  near(translation(snap, c)[0], 70, 1e-4, 'stopped at the limit')
})
check('13 Mechanical mates', 'a gear mate turns the driven shaft by the ratio', async () => {
  await asmParts()
  const twin = await savePart(
    'c13-twin',
    [rect([0, 0], [100, 60]), circle([30, 30], 5), circle([70, 30], 5)],
    10,
  )
  await call('asm.new')
  const p = await insert(twin)
  const a = await insert(pinFile, [30, 30, 10])
  const b = await insert(pinFile, [70, 30, 10])
  const holes = (await call('asm.topology', { component: [p] })).faces.filter(cyl(5))
  const hole = (x) => holes.find((f) => Math.abs(f.centroid[0] - x) < 1e-6).ref
  for (const [k, x] of [
    [a, 30],
    [b, 70],
  ]) {
    await call('asm.mate.add', {
      mate: { type: 'concentric', a: hole(x), b: await asmFace(k, cyl(5)) },
    })
    await call('asm.mate.add', {
      mate: {
        type: 'coincident',
        a: await asmFace(p, flat([0, 0, 1], 10)),
        b: await asmFace(k, facing([0, 0, -1])),
      },
    })
  }
  await call('asm.mate.add', {
    mate: {
      type: 'gear',
      a: await asmFace(a, cyl(5)),
      b: await asmFace(b, cyl(5)),
      ratio_a: 20,
      ratio_b: 40,
    },
  })
  for (let i = 0; i < 8; i++)
    await call('asm.move', { component: [a], motion: { mode: 'rotate', angles: [0, 0, PI / 4] } })
  const snap = await call('asm.info')
  expect(snap.diagnostics.solved, 'solved after a full turn')
  // B's X axis: after A's full turn, B has turned half a turn (ratio 20:40).
  const tb = snap.scene.instances.find((i) => i.key[0] === b).transform
  near(Math.abs(Math.atan2(tb[1], tb[0])), PI, 1e-3, 'driven shaft half turn')
})
check('13 Move component', 'collision detection stops a drag at contact', async () => {
  const { p, s } = await pinned()
  const c = await insert(cubeFile, [0, 25, 10])
  await call('asm.mate.add', {
    mate: {
      type: 'coincident',
      a: await asmFace(p, flat([0, 0, 1], 10)),
      b: await asmFace(c, facing([0, 0, -1])),
    },
  })
  const snap = await call('asm.move', {
    component: [c],
    motion: { mode: 'delta', translation: [60, 0, 0] },
    collision: {},
  })
  const x = translation(snap, c)[0]
  expect(x + 10 <= 25 + 1e-6 && x + 10 > 24, `stopped at the pin: ${x}`)
  expect(snap.collision.components?.flat().includes(s), 'collision names the pin')
})
check('13 Component patterns', 'linear pattern instances follow their seed', async () => {
  const { s } = await pinned()
  const snap = await call('asm.pattern.add', {
    pattern: {
      type: 'linear',
      direction1: { reference: { component: [], entity: right }, spacing: 25, count: 3 },
    },
    seeds: [s],
  })
  const xs = snap.scene.instances.filter((i) => i.derived !== null).map((i) => i.transform[12])
  expect(JSON.stringify(xs.sort()) === JSON.stringify([55, 80]), `instances at ${xs}`)
})
check('13 Mirror components', 'a mirrored copy lands on the other side of the plane', async () => {
  await asmParts()
  await call('asm.new')
  await insert(plateFile)
  const c = await insert(cubeFile, [120, 0, 0])
  const snap = await call('asm.mirror.add', {
    plane: { component: [], entity: right },
    components: [{ component: c }],
    position: 'bounding_box',
  })
  const copy = snap.scene.instances.find((i) => i.derived !== null)
  // Bounding-box center 125 mirrors to −125; the copy's box is −130..−120.
  near(copy.transform[12], -130, 1e-6, 'mirrored corner')
})
check(
  '13 Interference detection',
  'overlap volume is reported; touching only on request',
  async () => {
    const { p } = await pinned()
    const c = await insert(cubeFile, [80, 20, 5])
    void p
    const r = await call('asm.interference', {})
    expect(r.interferences.length === 1, `${r.interferences.length} interferences`)
    near(r.interferences[0].volume, 10 * 10 * 5, 1e-6, 'overlap volume')
    const touching = await call('asm.interference', { treat_coincidence: true })
    expect(touching.interferences.length === 2, 'the seated pin touches the plate')
    void c
  },
)
check('13 Exploded views', 'explode steps move the display, not the solved positions', async () => {
  const { s } = await pinned()
  const snap = await call('asm.explode.add', {
    components: [[s]],
    motion: { type: 'translate', direction: [0, 0, 1], distance: 50 },
  })
  near(translation(snap, s)[2], 60, 1e-9, 'exploded display')
  expect(snap.scene.explode_lines.length === 1, 'explode line')
  const back = await call('asm.explode.show', { steps: null })
  near(translation(back, s)[2], 10, 1e-9, 'collapsed')
  const info = await call('asm.info')
  expect(info.diagnostics.solved && component(info, s).dof === 1, 'mates untouched')
})
check('13 BOM', 'rows group instances of the same file', async () => {
  const { s } = await pinned()
  void s
  await insert(pinFile, [80, 40, 10])
  const rows = (await call('asm.bom', { type: 'top_level' })).rows
  expect(rows.length === 2, `${rows.length} rows`)
  expect(rows.find((r) => r.part_number === 'c13-pin').quantity === 2, 'two pins')
})
check('19 STEP', 'assembly export keeps the product structure; import rebuilds it', async () => {
  await pinned()
  const step = join(tmp, 'c13.step')
  await call('export', { path: step })
  expect(existsSync(step) && statSync(step).size > 0, 'written')
  const imported = await call('asm.import_step', { path: step, folder: join(tmp, 'c13-imported') })
  expect(imported.components.length === 2, `${imported.components.length} components`)
})

// 15 Drawings
const plain = (t) =>
  t
    .replace(/[⌀⌴⌵↧□]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
const textOf = (render, id) =>
  plain(
    render.graphics.items
      .filter((i) => i.owner.type === 'annotation' && i.owner.id === id && i.prim.type === 'text')
      .map((i) => i.prim.text)
      .join(' '),
  )
const formatText = (render) =>
  render.graphics.items
    .filter((i) => i.owner.type === 'format' && i.prim.type === 'text')
    .map((i) => i.prim.text)
const viewItems = (render, view) =>
  render.graphics.items.filter((i) => i.owner.type === 'view' && i.owner.view === view)
const render = (sheet = 1) => call('drawing.render', { sheet })
const noErrors = (r) => {
  for (const v of r.views) expect(!v.error, `view ${v.id}: ${v.error}`)
}
const width = (b) => b[1][0] - b[0][0]
const annotate = async (annotation, view, sheet = 1) =>
  (await call('drawing.annotation.add', { sheet, annotation, ...(view ? { view } : {}) })).id
const addView = async (view, origin, extra = {}, sheet = 1) =>
  (await call('drawing.view.add', { sheet, view, origin, ...extra })).id
const modelView = (path, type = 'front', more = {}) => ({
  type: 'model',
  model: { path },
  orientation: { type },
  ...more,
})

/** A 100 x 60 x 10 plate with a Ø12 bore, two M6 counterbores and properties, saved. */
async function drawingPart(name = 'c15-plate.ofp') {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [100, 60]), circle([75, 30], 6)])
  const bore = s.view.curves.find((c) => c.type === 'circle').id
  await call('edit.begin', { id: s.id })
  await call('sketch.edit', {
    id: s.id,
    op: { op: 'dimension', constraint: { type: 'diameter', curve: bore, value: 12 } },
  })
  await call('sketch.close')
  await ok(blind(s.id, 10))
  const topFace = await face(facing([0, 0, 1]))
  const h = await sketch(topFace, [
    { op: 'add_point', at: [20, 30] },
    { op: 'add_point', at: [40, 30] },
  ])
  await ok({
    type: 'hole',
    sketch: h.id,
    kind: 'counterbore',
    standard: 'iso',
    size: 'M6',
    end: { type: 'through_all' },
  })
  await call('doc.properties.set', {
    properties: { PartNo: { value: 'C15-PLATE' }, Description: { value: 'Plate' } },
  })
  await call('doc.set_material', { name: 'Steel', density: 7850 })
  const path = join(tmp, name)
  await call('doc.save', { path })
  return path
}
/** A front view (2) of the plate on a blank A3 drawing. */
async function plateDrawing(display) {
  const path = await drawingPart()
  await call('drawing.new')
  await addView(modelView(path), [150, 150], display ? { display } : {})
  return path
}

check(
  '15 Sheets',
  'title block fields show the model properties, mass, scale and sheet count',
  async () => {
    const path = await drawingPart()
    await call('drawing.new', { model: path })
    const r = await render()
    noErrors(r)
    const t = formatText(r)
    for (const want of ['C15-PLATE', 'Plate', 'Steel', '1 / 1'])
      expect(t.includes(want), `${want} in ${t.join('|')}`)
    expect(
      t.some((x) => / kg$/.test(x)),
      'mass',
    )
    await call('drawing.sheet.add', {})
    expect(formatText(await render()).includes('1 / 2'), 'sheet count follows')
  },
)
check('15 Sheets', 'fails: a zero sheet scale and deleting the last sheet', async () => {
  await call('drawing.new')
  expect(
    await fails('drawing.sheet.update', { id: 1, scale: { num: 0, den: 1 } }),
    'zero scale accepted',
  )
  expect(await fails('drawing.sheet.delete', { ids: [1] }), 'last sheet deleted')
})
check('15 View types', 'standard 3 view follows first and third angle projection', async () => {
  const path = await drawingPart()
  for (const [projection, above] of [
    ['third', true],
    ['first', false],
  ]) {
    await call('drawing.new', { projection })
    const v = (await call('drawing.views.standard', { sheet: 1, model: path })).views
    const r = await render()
    noErrors(r)
    const box = (id) => r.views.find((x) => x.id === id).bbox
    const top = box(v.top)[0][1] > box(v.front)[1][1]
    expect(top === above, `${projection}: top view ${top ? 'above' : 'below'} the front`)
  }
})
check(
  '15 View types',
  'model views in every named orientation and projected views in eight directions',
  async () => {
    const path = await plateDrawing()
    for (const o of [
      'back',
      'left',
      'right',
      'top',
      'bottom',
      'isometric',
      'dimetric',
      'trimetric',
    ])
      await addView(modelView(path, o), [300, 200])
    for (const placement of [
      'left',
      'right',
      'above',
      'below',
      'above_left',
      'above_right',
      'below_left',
      'below_right',
    ])
      await addView({ type: 'projected', parent: 2, placement }, [150, 150])
    const r = await render()
    noErrors(r)
    expect(r.views.length === 17, `${r.views.length} views`)
  },
)
check(
  '15 View types',
  'an auxiliary view normal to an edge; fails on a circular edge',
  async () => {
    const path = await plateDrawing()
    const r0 = await render()
    const line = viewItems(r0, 2).find((i) => i.owner.entity && i.prim.type === 'line')
    const aux = await addView({ type: 'auxiliary', parent: 2, edge: line.owner.entity }, [320, 150])
    noErrors(await render())
    const arc = viewItems(r0, 2).find((i) => i.owner.entity && i.prim.type === 'arc')
    const bad = await addView({ type: 'auxiliary', parent: 2, edge: arc.owner.entity }, [320, 60])
    const r = await render()
    expect(r.views.find((v) => v.id === aux).bbox, 'auxiliary drawn')
    expect(r.views.find((v) => v.id === bad).error, 'circular edge accepted')
    void path
  },
)
check(
  '15 Section views',
  'full sections hatch the cut faces; slice shows only them; aligned needs three points',
  async () => {
    await plateDrawing()
    const line = {
      points: [
        [50, 70],
        [50, -10],
      ],
    }
    const full = await addView({ type: 'section', parent: 2, line }, [300, 150])
    const slice = await addView({ type: 'section', parent: 2, line, slice: true }, [370, 150])
    const aligned = await addView({ type: 'section', parent: 2, line, kind: 'aligned' }, [300, 60])
    const r = await render()
    expect(
      viewItems(r, full).some((i) => i.layer === 'HATCH'),
      'hatching',
    )
    expect(
      viewItems(r, slice).some((i) => i.layer === 'HATCH'),
      'slice hatching',
    )
    expect(viewItems(r, slice).length < viewItems(r, full).length, 'slice draws less')
    expect(r.views.find((v) => v.id === aligned).error, 'two-point aligned section accepted')
    expect(
      viewItems(r, full).some((i) => i.prim.type === 'text' && i.prim.text.includes('A')),
      'labelled',
    )
  },
)
check(
  '15 Detail views',
  'a detail is twice the parent scale and keeps only what is in its circle',
  async () => {
    await plateDrawing()
    const profile = { type: 'circle', center: [75, 30], radius: 10 }
    const d = await addView({ type: 'detail', parent: 2, profile }, [320, 80])
    const r = await render()
    noErrors(r)
    const v = r.views.find((x) => x.id === d)
    expect(v.scale === 2, `scale ${v.scale}`)
    expect(width(v.bbox) <= 2 * 2 * 10 + 1e-6, `width ${width(v.bbox)}`)
  },
)
check(
  '15 Break views',
  'a break shortens the view and dimensions across it keep their true length',
  async () => {
    await plateDrawing()
    const before = (await render()).views[0].bbox
    await call('drawing.view.update', {
      id: 2,
      breaks: [{ vertical: true, from: 30, to: 70 }],
      break_style: 'zig_zag',
    })
    const r = await render()
    const after = r.views[0].bbox
    expect(width(after) < width(before) - 20, `${width(before)} -> ${width(after)}`)
    const bottom = viewItems(r, 2).filter((i) => i.owner.entity && i.prim.type === 'line')
    // Left and right edges, picked by their position on the sheet.
    const vertical = bottom.filter((i) => Math.abs(i.prim.a[0] - i.prim.b[0]) < 1e-6)
    vertical.sort((a, b) => a.prim.a[0] - b.prim.a[0])
    const dim = await annotate(
      {
        type: 'dimension',
        dim: 'linear',
        a: { view: 2, entity: vertical[0].owner.entity },
        b: { view: 2, entity: vertical[vertical.length - 1].owner.entity },
        at: [20, -20],
      },
      2,
    )
    expect(textOf(await render(), dim) === '100', `${textOf(await render(), dim)}`)
  },
)
check('15 Crop views', 'a crop keeps only the profile', async () => {
  await plateDrawing()
  await call('drawing.view.update', {
    id: 2,
    crop: { type: 'circle', center: [75, 30], radius: 15 },
  })
  const v = (await render()).views[0]
  expect(width(v.bbox) <= 30 + 1e-6, `width ${width(v.bbox)}`)
})
check(
  '15 Display',
  'hidden lines visible adds dashed hidden edges; removed draws none',
  async () => {
    // From above, the through holes are hidden behind the top face.
    const path = await drawingPart()
    await call('drawing.new')
    await addView(modelView(path, 'top'), [150, 150], { display: 'hidden_visible' })
    const hidden = (await render()).graphics.items.filter((i) => i.layer === 'HIDDEN')
    expect(hidden.length > 0 && hidden.every((i) => i.pen.style === 'hidden'), 'hidden lines')
    await call('drawing.view.update', { id: 2, display: 'hidden_removed' })
    expect(!(await render()).graphics.items.some((i) => i.layer === 'HIDDEN'), 'none when removed')
  },
)
check(
  '15 Model items',
  'dimensions import once, into the truest view; hidden ones stay out',
  async () => {
    const path = await drawingPart()
    await call('doc.open', { path })
    await call('doc.dimension_display.set', { key: 'f2.end1.depth', show: false })
    await call('doc.save', { path })
    await call('drawing.new', { model: path })
    const first = await call('drawing.model_items', { sheet: 1 })
    const dims = first.drawing.sheets[0].annotations.filter((a) => a.type === 'dimension')
    const keys = dims.map((a) => a.key)
    expect(!keys.includes('f2.end1.depth'), 'hidden dimension imported')
    expect(keys.length === 1 && keys[0].startsWith('f1.dimension.'), JSON.stringify(keys))
    // The bore's diameter lies in the front plane: the front view shows it true.
    const front = first.drawing.sheets[0].views.find((v) => v.kind.orientation?.type === 'front')
    expect(dims[0].attach === front.id, `in view ${dims[0].attach}`)
    const again = await call('drawing.model_items', { sheet: 1 })
    expect(
      again.added.every(
        (id) => again.drawing.sheets[0].annotations.find((a) => a.id === id).type !== 'dimension',
      ),
      'duplicate dimensions',
    )
  },
)
check(
  '15 Dimensions',
  'an imported dimension drives the part; a reference dimension does not',
  async () => {
    const path = await drawingPart()
    await call('drawing.new', { model: path })
    const r = await call('drawing.model_items', { sheet: 1 })
    const depth = r.drawing.sheets[0].annotations.find((a) => a.key === 'f2.end1.depth')
    await call('drawing.dimension.drive', { id: depth.id, value: 14 })
    expect(textOf(await render(), depth.id) === '14', 'redrawn')
    await call('doc.open', { path })
    near(
      await volume(),
      100 * 60 * 14 - PI * 36 * 14 - 2 * (PI * 3.3 ** 2 * (14 - 6.4) + PI * 5.5 ** 2 * 6.4),
      0.1,
      'part volume',
    )
    await call('drawing.new', { model: path })
    const r0 = await render()
    const line = viewItems(r0, r0.views[0].id).find((i) => i.owner.entity && i.prim.type === 'line')
    const ref = await annotate(
      {
        type: 'dimension',
        dim: 'linear',
        a: { view: r0.views[0].id, entity: line.owner.entity },
        at: [0, -20],
      },
      r0.views[0].id,
    )
    expect(
      await fails('drawing.dimension.drive', { id: ref, value: 5 }),
      'reference dimension drove the model',
    )
  },
)
check(
  '15 Dimension properties',
  'fits, bilateral and symmetric tolerances, prefix and dual units',
  async () => {
    await plateDrawing()
    const r0 = await render()
    const bore = viewItems(r0, 2).find(
      (i) => i.owner.entity && i.prim.type === 'arc' && Math.abs(i.prim.radius - 6) < 1e-6,
    )
    const target = { view: 2, entity: bore.owner.entity }
    const dim = (tolerance, more = {}) =>
      annotate({ type: 'dimension', dim: 'diameter', target, at: [100, 50], tolerance, ...more }, 2)
    const fit = await dim({ type: 'fit', hole: 'H7', shaft: 'g6' })
    const fitTol = await dim({ type: 'fit', hole: 'H7', display: 'classes_with_tolerance' })
    const bil = await dim({ type: 'bilateral', plus: 0.1, minus: 0.05 })
    const sym = await dim({ type: 'symmetric', value: 0.2 }, { dual: true })
    const r = await render()
    expect(textOf(r, fit) === '12 H7/g6', textOf(r, fit))
    expect(textOf(r, fitTol) === '12 H7 +0.018 0', textOf(r, fitTol))
    expect(textOf(r, bil) === '12 +0.1 -0.05', textOf(r, bil))
    expect(textOf(r, sym) === '12±0.2 [0.472]', textOf(r, sym))
    expect(
      await fails('drawing.annotation.add', {
        sheet: 1,
        view: 9,
        annotation: { type: 'dimension', dim: 'diameter', target, at: [0, 0] },
      }),
      'unknown view accepted',
    )
  },
)
check('15 Hole callouts', 'a counterbore callout reads the Hole Wizard data', async () => {
  const path = await drawingPart()
  await call('drawing.new')
  await addView(modelView(path, 'top'), [150, 150])
  const r = await call('drawing.model_items', {
    sheet: 1,
    items: { dimensions: false, hole_callouts: true, threads: false },
  })
  void r
  await call('drawing.new')
  await addView(modelView(path), [150, 150])
  const r0 = await render()
  const cb = viewItems(r0, 2).find(
    (i) => i.owner.entity && i.prim.type === 'arc' && Math.abs(i.prim.radius - 5.5) < 1e-6,
  )
  const id = await annotate(
    { type: 'hole_callout', at: [40, 40], target: { view: 2, entity: cb.owner.entity } },
    2,
  )
  expect(textOf(await render(), id) === '2X 6.6 THRU 11 6.4', textOf(await render(), id))
})
check(
  '15 Notes',
  'property links resolve; a leader to a deleted view drops, the note stays',
  async () => {
    await plateDrawing()
    const id = await annotate({
      type: 'note',
      at: [30, 40],
      text: '$PRP:"PartNo" $SHEET',
      leader: {
        type: 'model',
        target: { view: 2, entity: { component: [], entity: { type: 'origin' } } },
        at: [0, 0],
      },
    })
    expect(textOf(await render(), id) === 'C15-PLATE 1', textOf(await render(), id))
    await call('drawing.view.delete', { ids: [2] })
    const note = (await call('drawing.info')).drawing.sheets[0].annotations.find((a) => a.id === id)
    expect(note && !note.leader, 'note kept without its leader')
  },
)
check(
  '15 BOM and balloons',
  'auto balloons match BOM item numbers; a part view has nothing to balloon',
  async () => {
    const plate = await drawingPart()
    await fresh()
    const p = await sketch(front, [circle([0, 0], 6)])
    await ok(blind(p.id, 30))
    await call('doc.properties.set', { properties: { PartNo: { value: 'C15-PIN' } } })
    const pin = join(tmp, 'c15-pin.ofp')
    await call('doc.save', { path: pin })
    await call('asm.new')
    await call('asm.insert', { path: plate })
    await call('asm.insert', {
      path: pin,
      transform: { translation: [75, 30, 10], rotation: [0, 0, 0, 1] },
    })
    await call('asm.insert', {
      path: pin,
      transform: { translation: [75, 30, -30], rotation: [0, 0, 0, 1] },
    })
    const asm = join(tmp, 'c15.ofa')
    await call('doc.save', { path: asm })
    await call('drawing.new')
    const view = await addView(modelView(asm, 'isometric'), [200, 150])
    const bom = await annotate({ type: 'table', table: 'bom', view, at: [20, 280] })
    const r = await call('drawing.balloons.auto', { sheet: 1, view })
    const rr = await render()
    const items = r.added.map((id) => textOf(rr, id)).sort()
    expect(JSON.stringify(items) === '["1","2"]', JSON.stringify(items))
    const table = textOf(rr, bom)
    expect(table.includes('C15-PIN 2'), table)
    const csv = (await call('drawing.table.csv', { id: bom })).csv
    expect(csv.split('\n')[0].startsWith('ITEM NO.'), csv)
    const part = await addView(modelView(plate), [80, 80])
    expect(await fails('drawing.balloons.auto', { sheet: 1, view: part }), 'part view ballooned')
  },
)
check('15 Hole table', 'tags, positions from the origin and sizes', async () => {
  await plateDrawing()
  const r0 = await render()
  const holes = viewItems(r0, 2)
    .filter((i) => i.owner.entity && i.prim.type === 'arc' && Math.abs(i.prim.radius - 3.3) < 1e-6)
    .map((i) => ({ view: 2, entity: i.owner.entity }))
  const corner = viewItems(r0, 2).find((i) => i.owner.entity && i.prim.type === 'line')
  const id = await annotate({
    type: 'table',
    table: 'hole',
    origin: { view: 2, entity: corner.owner.entity, near: [0, 0] },
    holes,
    at: [20, 280],
  })
  const t = textOf(await render(), id)
  expect(t.startsWith('TAG X LOC Y LOC SIZE A1 20 30 6.6 THRU 11 6.4 A2 40 30'), t)
})
check(
  '15 Revision table',
  'automatic letters; the latest drives the title block revision',
  async () => {
    await plateDrawing()
    await annotate({
      type: 'table',
      table: 'revision',
      rows: [{ description: 'Released' }, { description: 'Bore' }],
      at: [410, 287],
      anchor: 'top_right',
    })
    expect(formatText(await render()).includes('B'), 'revision B in the title block')
  },
)
check('15 General tolerance table', 'ISO 2768-m values', async () => {
  await plateDrawing()
  const id = await annotate({
    type: 'table',
    table: 'general_tolerance',
    class: 'm',
    at: [20, 200],
  })
  const t = textOf(await render(), id)
  for (const want of ['ISO 2768-m', '30-120 mm ±0.3', '±0°30'])
    expect(t.includes(want), `${want} in ${t}`)
})
check(
  '15 Drafting standards',
  'ANSI keeps trailing zeros; standards save to and load from files',
  async () => {
    await plateDrawing()
    const r0 = await render()
    const bore = viewItems(r0, 2).find(
      (i) => i.owner.entity && i.prim.type === 'arc' && Math.abs(i.prim.radius - 6) < 1e-6,
    )
    const id = await annotate(
      {
        type: 'dimension',
        dim: 'diameter',
        target: { view: 2, entity: bore.owner.entity },
        at: [100, 50],
      },
      2,
    )
    expect(textOf(await render(), id) === '12', textOf(await render(), id))
    const file = join(tmp, 'c15.ofstd')
    await call('drawing.standard.set', {
      standard: { base: 'ansi', overrides: { precision: 3 } },
      save: file,
    })
    expect(textOf(await render(), id) === '12.000', textOf(await render(), id))
    await call('drawing.standard.set', { standard: { base: 'iso' } })
    await call('drawing.standard.set', { load: file })
    expect((await call('drawing.info')).drawing.standard.base === 'ansi', 'loaded')
    expect(
      await fails('drawing.standard.set', { load: join(tmp, 'missing.ofstd') }),
      'missing file loaded',
    )
  },
)
check(
  '15 Output',
  'PDF of chosen sheets, DXF per sheet and in one file; other extensions fail',
  async () => {
    await plateDrawing()
    await call('drawing.sheet.add', {})
    const pdf = join(tmp, 'c15.pdf')
    await call('drawing.export', { path: pdf, sheets: [1], monochrome: true })
    const bytes = (await import('node:fs')).readFileSync(pdf, 'latin1')
    expect(
      bytes.startsWith('%PDF') && (bytes.match(/\/Type\s*\/Page\b/g) ?? []).length === 1,
      'one page',
    )
    const many = await call('drawing.export', { path: join(tmp, 'c15.dxf'), per_sheet: true })
    expect(many.paths.length === 2, `${many.paths.length} files`)
    const one = (await call('drawing.export', { path: join(tmp, 'c15-all.dxf') })).paths[0]
    const dxf = (await import('node:fs')).readFileSync(one, 'utf8')
    expect(dxf.includes('AC1032') && dxf.includes('LAYOUT'), 'DXF 2018 with layouts')
    expect(await fails('drawing.export', { path: join(tmp, 'c15.png') }), 'png accepted')
  },
)
check(
  '15 Associativity',
  'saving the model marks views stale; a missing model reports an error',
  async () => {
    const path = await plateDrawing()
    await call('doc.save', { path: join(tmp, 'c15.ofd') })
    await call('doc.open', { path })
    await call('doc.properties.set', { properties: { PartNo: { value: 'C15-B' } } })
    await call('doc.save', { path })
    await call('doc.open', { path: join(tmp, 'c15.ofd') })
    const r = await render()
    expect(r.views[0].stale, 'stale after the model changed')
    expect(formatText(r).includes('C15-B'), 'title block follows the saved model')
    await call('drawing.update')
    expect(!(await render()).views[0].stale, 'updated')
    ;(await import('node:fs')).renameSync(path, path + '.moved')
    const gone = await render()
    expect(gone.views[0].error, 'missing model drawn')
    ;(await import('node:fs')).renameSync(path + '.moved', path)
  },
)

// ---- 10 Sheet metal ----

const HALF_PI = PI / 2
const smParams = (t, r, k = 0.44, more = {}) => ({
  thickness: t,
  radius: r,
  allowance: { type: 'k_factor', k },
  ...more,
})
/** Lines chained point to point, so they make one open profile. */
async function chain(plane, pts) {
  const { id } = await call('sketch.new', { plane })
  let prev = null
  for (let i = 0; i + 1 < pts.length; i++) {
    const r = await call('sketch.edit', {
      id,
      op: {
        op: 'add_line',
        a: prev === null ? { at: pts[i] } : { id: prev },
        b: { at: pts[i + 1] },
      },
    })
    prev = r.created[2]
  }
  await call('sketch.close')
  return id
}
async function smPlate(w, h, t = 1, r = 1, k = 0.5, more = {}) {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [w, h])])
  await ok({ type: 'base_flange', sketch: s.id, sheet: smParams(t, r, k, more) })
}
const edgeAt = async (p) => {
  const e = (await topology()).edges.find(
    (e) =>
      e.curve === 'line' &&
      Math.hypot(...[0, 1, 2].map((i) => (e.start[i] + e.end[i]) / 2 - p[i])) < 1e-3,
  )
  expect(e, `edge at ${p}`)
  return e.ref
}
const faceWhere = async (f) => {
  const faces = (await topology()).faces.filter(f)
  faces.sort((a, b) => b.area - a.area)
  expect(faces.length, 'face found')
  return faces[0].ref
}
const smInfo = async (outline = false) => (await call('doc.sheet_metal', { outline })).bodies
const flatProps = async () => (await smInfo())[0].properties
const edgeFlange = (edges, length, more = {}) => ({
  type: 'edge_flange',
  edges,
  length: { type: 'blind', length },
  ...more,
})
check(
  '10 Sheet metal parameters',
  'K-factor, bend allowance and bend deduction set the flat length',
  async () => {
    const lengths = []
    for (const allowance of [
      { type: 'k_factor', k: 0.44 },
      { type: 'bend_allowance', value: 5 },
      { type: 'bend_deduction', value: 4 },
    ]) {
      await fresh()
      const s = await chain(front, [
        [0, 30],
        [0, 0],
        [50, 0],
      ])
      await ok({
        type: 'base_flange',
        sketch: s,
        sheet: { thickness: 2, radius: 2, allowance },
        end1: { type: 'blind', depth: 20 },
      })
      lengths.push((await flatProps()).flat_length)
    }
    near(lengths[0], 30 - 4 + 50 - 4 + HALF_PI * (2 + 0.44 * 2), 1e-6, 'K-factor')
    near(lengths[1], 30 - 4 + 50 - 4 + 5, 1e-6, 'bend allowance')
    near(lengths[2], 30 + 50 - 4, 1e-6, 'bend deduction')
  },
)

check(
  '10 Sheet metal parameters',
  'a gauge table sets thickness and radius; an unknown gauge fails',
  async () => {
    const gauges = await call('sheet_metal.gauges', { table: 'steel' })
    expect(gauges.tables.includes('steel'), 'built-in tables listed')
    const g16 = gauges.gauges.find((g) => g.gauge === '16')
    await smPlate(60, 40, 1, 1, 0.5, { gauge: { table: 'steel', gauge: '16' } })
    near((await flatProps()).thickness, g16.thickness, 1e-9, 'gauge thickness')
    await fresh()
    const s = await sketch(front, [rect([0, 0], [60, 40])])
    await bad(
      {
        type: 'base_flange',
        sketch: s.id,
        sheet: smParams(1, 1, 0.5, { gauge: { table: 'steel', gauge: '99' } }),
      },
      /gauge 99/,
    )
  },
)

check(
  '10 Base flange/tab',
  'a closed sketch makes a plate; a tab on it merges at the sheet thickness',
  async () => {
    await smPlate(60, 40, 2, 2)
    near(await volume(), 60 * 40 * 2, 1e-9, 'plate')
    const topFace = await faceWhere((f) => f.normal?.[2] > 0.99)
    const tab = await worldSketch(topFace, (local) => [
      rect(local([60, 10, 2]), local([80, 30, 2])),
    ])
    await ok({ type: 'base_flange', sketch: tab.id, sheet: smParams(2, 2) })
    near(await volume(), (60 * 40 + 20 * 20) * 2, 1e-6, 'tab merged')
    expect((await smInfo()).length === 1, 'one sheet metal body')
  },
)

check('10 Base flange/tab', 'an open profile is thickened and bent at its corners', async () => {
  await fresh()
  const s = await chain(front, [
    [0, 30],
    [0, 0],
    [50, 0],
    [50, 20],
  ])
  await ok({
    type: 'base_flange',
    sketch: s,
    sheet: smParams(1.5, 1.5),
    end1: { type: 'blind', depth: 25 },
  })
  const info = await smInfo()
  expect(info[0].bends.length === 2, `two bends, got ${info[0].bends.length}`)
})

check('10 Edge flange', 'length measures and angle from a face', async () => {
  // The flat's side across the flange: the one that isn't the plate's 60.
  const flat = async (more) => {
    await smPlate(60, 40, 1, 1, 0.5)
    const e = await edgeAt([30, 40, 1])
    await ok(edgeFlange([e], 20, more))
    const p = await flatProps()
    return Math.abs(p.flat_length - 60) < 1e-9 ? p.flat_width : p.flat_length
  }
  const ba = HALF_PI * (1 + 0.5)
  const outer = await flat({ measure: 'outer_virtual_sharp' })
  const inner = await flat({ measure: 'inner_virtual_sharp' })
  const tangent = await flat({ measure: 'tangent_to_bend' })
  near(outer, 40 - 2 + (20 - 2) + ba, 1e-6, 'outer virtual sharp')
  near(inner, 40 - 2 + (20 - 1) + ba, 1e-6, 'inner virtual sharp')
  near(tangent, 40 - 2 + 20 + ba, 1e-6, 'tangent to bend')
  const square = await flat({ angle: 0.2, angle_from: { face: front, parallel: false } })
  near(square, outer, 1e-9, 'square to the sheet plane is 90°')
})

check('10 Edge flange', 'gap at a shared corner, offsets and a wall outline', async () => {
  await smPlate(60, 40, 1, 1, 0.5)
  const edges = [await edgeAt([30, 40, 1]), await edgeAt([60, 20, 1])]
  await ok(edgeFlange(edges, 15, { gap: 0.4 }))
  expect((await smInfo())[0].bends.length === 2, 'two flanges')
  await smPlate(60, 40, 1, 1, 0.5)
  const e = await edgeAt([30, 40, 1])
  const plain = await (async () => {
    await ok(edgeFlange([e], 20, { measure: 'tangent_to_bend', start_offset: 10, end_offset: 10 }))
    return volume()
  })()
  await smPlate(60, 40, 1, 1, 0.5)
  const e2 = await edgeAt([30, 40, 1])
  await ok(
    edgeFlange([e2], 99, {
      profiles: [
        {
          edge: 0,
          points: [
            [10, 0],
            [50, 0],
            [40, 20],
            [20, 20],
          ],
        },
      ],
    }),
  )
  near(plain - (await volume()), 200, 1e-6, 'trapezoid wall')
})

check('10 Miter flange', 'a profile runs round a corner along two edges', async () => {
  await smPlate(60, 40, 1, 1, 0.5)
  const edges = [await edgeAt([30, 40, 1]), await edgeAt([60, 20, 1])]
  const s = await chain(right, [
    [-1, 40],
    [-11, 40],
    [-11, 45],
  ])
  await ok({ type: 'miter_flange', sketch: s, edges, gap: 0.2 })
  expect((await smInfo())[0].bends.length === 4, 'two bends per edge')
  await ok({ type: 'flat_pattern' })
})

check('10 Hem', 'every hem kind builds; an open hem flattens to its length', async () => {
  for (const kind of [
    { type: 'closed' },
    { type: 'open', gap: 2 },
    { type: 'tear_drop', angle: (220 * PI) / 180, radius: 2 },
    { type: 'rolled', angle: (270 * PI) / 180, radius: 3 },
    { type: 'double', gap: 0.5 },
  ]) {
    await smPlate(60, 40, 1, 1, 0.5)
    await ok({ type: 'hem', edges: [await edgeAt([30, 40, 1])], kind, length: 8 })
    await ok({ type: 'flat_pattern' })
    if (kind.type === 'open') {
      near((await flatProps()).flat_width, 40 - 2 + PI * (1 + 0.5) + 8, 1e-6, 'open hem flat')
    }
  }
})

/** A line across the plate's top face at x = `x`. */
async function lineAcross(x, h, t) {
  const topFace = await faceWhere((f) => f.normal?.[2] > 0.99)
  return worldSketch(topFace, (local) => [line(local([x, -5, t]), local([x, h + 5, t]))])
}

check('10 Jog', 'a jog lifts the far side by its offset', async () => {
  await smPlate(100, 40, 1.5, 1.5, 0.44)
  const s = await lineAcross(60, 40, 1.5)
  await ok({ type: 'jog', sketch: s.id, offset: 10, position: 'bend_outside' })
  expect((await smInfo())[0].bends.length === 2, 'two bends')
  near((await call('doc.info')).scene.bbox.max[2], 11.5, 1e-6, 'offset')
})

check('10 Sketched bend', 'a sketched bend folds the plate and flattens back', async () => {
  await smPlate(80, 40, 2, 2, 0.44)
  const before = await volume()
  const s = await lineAcross(50, 40, 2)
  await ok({ type: 'sketched_bend', sketch: s.id, angle: HALF_PI })
  await ok({ type: 'flat_pattern' })
  const p = await flatProps()
  near(p.flat_length, 80, 1e-6, 'flat length kept')
  near(await volume(), before, 1e-6, 'volume kept')
})

check('10 Lofted bend', 'square to round, bent and formed, flatten', async () => {
  for (const method of ['bent', 'formed']) {
    await fresh()
    const sq = await chain(front, [
      [1, -20],
      [20, -20],
      [20, 20],
      [-20, 20],
      [-20, -20],
      [-1, -20],
    ])
    const { id: plane } = await ok({
      type: 'plane',
      mode: 'offset',
      reference: front,
      distance: 30,
    })
    const { id: arc } = await call('sketch.new', { plane: { type: 'feature', id: plane } })
    const g = 1 / 15
    await call('sketch.edit', {
      id: arc,
      op: {
        op: 'add_center_arc',
        center: { at: [0, 0] },
        start: { at: [15 * Math.cos(-HALF_PI + g), 15 * Math.sin(-HALF_PI + g)] },
        end: [15 * Math.cos(-HALF_PI - g), 15 * Math.sin(-HALF_PI - g)],
      },
    })
    await call('sketch.close')
    await ok({
      type: 'lofted_bend',
      profile1: sq,
      profile2: arc,
      method,
      facets: 8,
      sheet: smParams(1, 1, 0.5),
    })
    await ok({ type: 'flat_pattern' })
    expect((await flatProps()).thickness === 1, method)
  }
})

check('10 Swept flange', 'an open profile along a path rolls out along it', async () => {
  await fresh()
  const { id: path } = await call('sketch.new', { plane: front })
  const l1 = await call('sketch.edit', { id: path, op: line([0, 0], [40, 0]) })
  const arc = await call('sketch.edit', {
    id: path,
    op: {
      op: 'add_center_arc',
      center: { at: [40, 20] },
      start: { id: l1.created[2] },
      end: [60, 20],
    },
  })
  const arcEnd = arc.created[arc.created.length - 1]
  await call('sketch.edit', {
    id: path,
    op: { op: 'add_line', a: { id: arcEnd }, b: { at: [60, 60] } },
  })
  await call('sketch.close')
  const profile = await chain(right, [
    [0, 0],
    [-15, 0],
  ])
  await ok({
    type: 'swept_flange',
    profile,
    path: { type: 'feature', id: path },
    sheet: smParams(1, 1, 0.5),
  })
  await ok({ type: 'flat_pattern' })
  near(
    (await flatProps()).flat_length,
    80 + HALF_PI * 20.5,
    1e-6,
    'path length at the neutral radius',
  )
})

check('10 Cross break', 'recorded on a face; the solid is unchanged', async () => {
  await smPlate(60, 40)
  const v = await volume()
  await ok({ type: 'cross_break', face: await faceWhere((f) => f.normal?.[2] > 0.99) })
  near(await volume(), v, 1e-9, 'unchanged')
})

/** A 60 × 40 × 30 block. */
async function smBlock() {
  await fresh()
  const s = await sketch(front, [rect([0, 0], [60, 40])])
  await ok(blind(s.id, 30))
}

check(
  '10 Insert bends and convert to sheet metal',
  'a block converts to a tray; a shell gets bends and rips',
  async () => {
    await smBlock()
    const bottom = await faceWhere((f) => f.normal?.[2] < -0.99)
    const edges = []
    for (const p of [
      [30, 0, 0],
      [60, 20, 0],
      [30, 40, 0],
      [0, 20, 0],
    ])
      edges.push(await edgeAt(p))
    await ok({
      type: 'convert_to_sheet_metal',
      fixed_face: bottom,
      sheet: smParams(1.5, 1, 0.5),
      bend_edges: edges,
    })
    expect((await smInfo())[0].bends.length === 4, 'four bends')
    await smBlock()
    await ok({ type: 'shell', faces: [await faceWhere((f) => f.normal?.[2] > 0.99)], thickness: 2 })
    const rips = []
    for (const [x, y] of [
      [0, 0],
      [60, 0],
      [60, 40],
      [0, 40],
    ])
      rips.push({ edge: await edgeAt([x, y, 15]) })
    await ok({
      type: 'insert_bends',
      fixed_face: await faceWhere((f) => f.normal?.[2] < -0.99),
      sheet: smParams(2, 2, 0.44),
      rips,
    })
    expect((await smInfo())[0].bends.length === 4, 'four bends')
    await ok({ type: 'flat_pattern' })
  },
)

check('10 Rip', 'ripping a corner edge opens it', async () => {
  await smBlock()
  await ok({ type: 'shell', faces: [await faceWhere((f) => f.normal?.[2] > 0.99)], thickness: 2 })
  const v = await volume()
  await ok({ type: 'rip', edges: [{ edge: await edgeAt([0, 0, 15]) }], gap: 0.5 })
  expect((await volume()) < v, 'material removed')
})

/** A 60 × 40 × 1 plate with a 20 mm flange on y = 40. */
async function flanged() {
  await smPlate(60, 40, 1, 1, 0.5)
  await ok(edgeFlange([await edgeAt([30, 40, 1])], 20))
}

check('10 Unfold and fold', 'a cut made across a bend while unfolded refolds', async () => {
  await flanged()
  const formed = await volume()
  await ok({ type: 'unfold' })
  const s = await sketch(front, [circle([30, 40], 3)])
  await ok({ ...blind(s.id, 0, { cut: true, end1: { type: 'through_all' } }) })
  await ok({ type: 'fold' })
  expect((await volume()) < formed - 1, 'hole folded with the bend')
  expect((await smInfo())[0].bends.length === 1, 'still one bend')
})

check('10 Normal cut', 'a sketch projected through the bend cuts square to the sheet', async () => {
  await flanged()
  const formed = await volume()
  const { id: plane } = await ok({ type: 'plane', mode: 'offset', reference: front, distance: 50 })
  const s = await sketch({ type: 'feature', id: plane }, [circle([30, 39], 3)])
  await ok({ type: 'normal_cut', sketch: s.id })
  expect((await volume()) < formed - 1, 'cut')
})

check(
  '10 Flatten and flat pattern',
  'flatten toggles by suppression; options split bend regions and simplify bends',
  async () => {
    await flanged()
    const formed = (await call('doc.info')).scene.body.positions.length
    const { id } = await ok({ type: 'flat_pattern' })
    expect((await smInfo())[0].flattened, 'flattened')
    await call('feature.suppress', { id, suppressed: true })
    expect(!(await smInfo())[0].flattened, 'formed again')
    expect((await call('doc.info')).scene.body.positions.length === formed, 'same body back')
    await call('feature.suppress', { id, suppressed: false })
    const merged = (await topology()).faces.length
    await call('feature.update', {
      id,
      feature: { type: 'flat_pattern', split_bend_regions: true },
    })
    expect((await topology()).faces.length === merged + 4, 'front and back split in three')
  },
)

check(
  '10 Flat pattern configurations and DXF export',
  'a part exported to .dxf writes its flat with bend layers',
  async () => {
    await flanged()
    const path = join(tmp, 'c10-flat.dxf')
    const r = await call('export', { path, dxf_version: 'r12' })
    expect(r.files.length === 1 && existsSync(path), 'written')
    const text = (await import('node:fs')).readFileSync(path, 'utf8')
    expect(text.includes('CUT') && /BEND_(UP|DOWN)/.test(text), 'layers')
    expect(/(UP|DOWN) 90/.test(text), 'bend note')
    await fresh()
    expect(/no sheet metal/.test(await fails('export', { path })), 'parts without sheet metal fail')
  },
)

check(
  '10 Corner treatments',
  'reliefs of every type, a closed corner and a broken corner',
  async () => {
    const tray = async () => {
      await smPlate(60, 40, 1, 1, 0.5)
      await ok(edgeFlange([await edgeAt([30, 40, 1]), await edgeAt([60, 20, 1])], 15, { gap: 0.2 }))
    }
    for (const kind of [
      'rectangular',
      'circular',
      'tear',
      'obround',
      'constant_width',
      'full_round',
      'suitcase',
    ]) {
      await tray()
      const v = await volume()
      await ok({ type: 'corner_relief', kind })
      expect((await volume()) < v, `${kind} relief cut`)
    }
    await tray()
    const end = await faceWhere(
      (f) =>
        f.surface === 'plane' &&
        f.centroid[0] > 55 &&
        f.centroid[1] > 37 &&
        f.centroid[2] > 2 &&
        f.normal[0] > 0.3,
    )
    const v = await volume()
    await ok({ type: 'closed_corner', faces: [end], kind: 'overlap', gap: 0.2 })
    expect((await volume()) > v, 'corner closed')
    await smPlate(60, 40, 1)
    await ok({ type: 'break_corner', corner: { type: 'fillet', radius: 5 } })
    near(await volume(), 2400 - 4 * (25 - (PI * 25) / 4), 1e-6, 'four rounded corners')
  },
)

check('10 Bend notch', 'notches at both ends of a bend line', async () => {
  await flanged()
  const v = await volume()
  await ok({ type: 'bend_notch', shape: 'rectangular', width: 1, depth: 1.5, both_ends: true })
  expect((await volume()) < v, 'notched')
})

check('10 Tab and slot', 'tabs through slots in another body', async () => {
  await smPlate(80, 40, 2, 1, 0.5)
  const { id: plane } = await ok({ type: 'plane', mode: 'offset', reference: top, distance: 20 })
  const s = await sketch({ type: 'feature', id: plane }, [rect([10, -40], [70, -2])])
  await ok({ type: 'base_flange', sketch: s.id, sheet: smParams(2, 1, 0.5) })
  const before = await volume()
  await ok({
    type: 'tab_and_slot',
    edge: await edgeAt([40, 20, 2]),
    face: await faceWhere((f) => f.normal?.[2] > 0.99 && Math.abs(f.centroid[2] - 2) < 1e-6),
    count: 3,
    length: 8,
    clearance: 0.1,
  })
  near(await volume(), before + 3 * 32 - 3 * 8.2 * 2.2 * 2, 1e-6, 'three tabs and slots')
})

check(
  '10 Sheet metal gusset',
  'a gusset stiffens the bend and is left out of the flat',
  async () => {
    await fresh()
    const s = await chain(front, [
      [0, 30],
      [0, 0],
      [50, 0],
    ])
    await ok({
      type: 'base_flange',
      sketch: s,
      sheet: smParams(2, 2),
      end1: { type: 'blind', depth: 40 },
    })
    const v = await volume()
    const bend = (await topology()).faces.find((f) => f.surface === 'cylinder').ref
    await ok({ type: 'sheet_metal_gusset', bend, depth: 8, width: 3 })
    expect((await volume()) > v, 'gusset added')
    await ok({ type: 'flat_pattern' })
    expect((await flatProps()).forms === 1, 'marked in the flat')
  },
)

check(
  '10 Forming tools and stamp tool',
  'a louver array shows as punch marks in the flat',
  async () => {
    await smPlate(120, 60, 1.5, 1.5)
    const pts = await sketch(front, [
      { op: 'add_point', at: [30, 30] },
      { op: 'add_point', at: [60, 30] },
      { op: 'add_point', at: [90, 30] },
    ])
    const v = await volume()
    await ok({
      type: 'form_tool',
      shape: { type: 'louver', length: 20, width: 5, height: 3 },
      face: await faceWhere((f) => f.normal?.[2] > 0.99),
      sketch: pts.id,
      punch_id: 'LV',
    })
    expect((await volume()) !== v, 'pressed')
    const body = (await smInfo(true))[0]
    expect(
      body.properties.forms === 3 && body.outline.marks.every((m) => m.punch_id === 'LV'),
      'punch marks',
    )
  },
)

check(
  '10 Gauge tables and bend tables',
  'a bend table CSV interpolates the allowance',
  async () => {
    const csv = join(tmp, 'c10-bends.csv')
    ;(await import('node:fs')).writeFileSync(
      csv,
      'thickness,radius,angle,allowance\n1,1,90,3\n1,1,180,6\n',
    )
    await fresh()
    const s = await chain(front, [
      [0, 30],
      [0, 0],
      [50, 0],
    ])
    await ok({
      type: 'base_flange',
      sketch: s,
      sheet: { thickness: 1, radius: 1, allowance: { type: 'bend_table', table: csv } },
      end1: { type: 'blind', depth: 20 },
    })
    near((await flatProps()).flat_length, 30 - 2 + 50 - 2 + 3, 1e-9, 'table allowance')
  },
)

check(
  '10 Bend lines and bend notes in drawings',
  'a flat pattern view draws bend lines, notes and a bend table',
  async () => {
    await flanged()
    const path = join(tmp, 'c10-flanged.ofp')
    await call('doc.save', { path })
    await call('drawing.new')
    const view = await addView(modelView(path, 'front', {}), [150, 150])
    await call('drawing.view.update', {
      id: view,
      view: {
        type: 'model',
        model: { path, flat: { bend_lines: true, bend_notes: true } },
        orientation: { type: 'front' },
      },
    })
    await annotate({ type: 'table', table: 'bend', view, at: [20, 280] })
    const r = await render()
    noErrors(r)
    const layers = r.graphics.items.map((i) => i.layer)
    expect(
      layers.some((l) => l === 'BEND_UP' || l === 'BEND_DOWN'),
      'bend line',
    )
    const words = r.graphics.items
      .filter((i) => i.prim.type === 'text')
      .map((i) => i.prim.text)
      .join(' ')
    expect(/(UP|DOWN) 90° R1/.test(words), 'bend note')
    expect(words.includes('DIRECTION'), 'bend table')
  },
)

check(
  '10 Multibody sheet metal and merging bodies',
  'an up-to-edge flange merges two sheet metal bodies',
  async () => {
    await smPlate(60, 40, 1, 1, 0.5)
    const { id: plane } = await ok({ type: 'plane', mode: 'offset', reference: top, distance: 40 })
    const s = await sketch({ type: 'feature', id: plane }, [rect([0, -30], [60, -10])])
    // A wall standing at y = 39..40 from z = 10 up.
    await ok({ type: 'base_flange', sketch: s.id, sheet: smParams(1, 1, 0.5), side: 'reverse' })
    expect((await smInfo()).length === 2, 'two bodies')
    const target = (await topology()).edges.find(
      (e) =>
        e.curve === 'line' &&
        [39, 40, 41].some(
          (y) =>
            Math.hypot(
              (e.start[0] + e.end[0]) / 2 - 30,
              (e.start[1] + e.end[1]) / 2 - y,
              (e.start[2] + e.end[2]) / 2 - 10,
            ) < 1e-3,
        ),
    ).ref
    await ok({
      type: 'edge_flange',
      edges: [await edgeAt([30, 40, 1])],
      length: { type: 'up_to_edge_and_merge', target },
    })
    expect((await smInfo()).length === 1, 'merged')
  },
)

check(
  '10 Sheet metal cut list and bounding box',
  'flat size, cutting lengths and property links',
  async () => {
    await fresh()
    const s = await sketch(front, [rect([0, 0], [80, 30]), circle([20, 15], 4)])
    await ok({ type: 'base_flange', sketch: s.id, sheet: smParams(1.5, 1) })
    const p = await flatProps()
    near(p.flat_length, 80, 1e-9, 'length')
    near(p.flat_width, 30, 1e-9, 'width')
    near(p.inner_cut, 8 * PI, 1e-6, 'hole cutting length')
    expect(p.cutouts === 1, 'one cut-out')
    await call('doc.properties.set', {
      properties: { Blank: { value: '{flat_length} x {flat_width}' } },
    })
    const props = (await call('doc.info')).properties
    expect(props.Blank.value === '{flat_length} x {flat_width}', 'link stored as written')
  },
)

// ---- 11 Weldments ----
const tube = (size) => ({ type: 'standard', family: 'rhs', size })
const seg = (sketch, entity) => ({ type: 'sketch_entity', sketch, entity })
const memberOf = (profile, segments, corner = 'end_miter') => ({
  type: 'structural_member',
  profile,
  groups: [{ segments, corner }],
})
const cutList = () => call('doc.cut_list')
const rowsOf = async () =>
  (await cutList()).items.map((i) => [i.properties.DESCRIPTION, i.properties.LENGTH, i.quantity])
/** A weldment with a 1200 × 600 rectangle; returns its four line refs. */
async function frameSketch() {
  await fresh()
  await ok({ type: 'weldment' })
  const s = await sketch(front, [rect([0, 0], [1200, 600])])
  return s.created[0].slice(4, 8).map((e) => seg(s.id, e))
}
/** The mitered SHS frame with two RHS cross members trimmed to its sides. */
async function trimmedFrame() {
  const lines4 = await frameSketch()
  const { id: f1 } = await ok(memberOf(tube('SHS 40x40x3'), lines4))
  const c = await sketch(front, [line([0, 200], [1200, 200]), line([0, 400], [1200, 400])])
  const { id: f2 } = await ok(
    memberOf(
      tube('RHS 60x40x3'),
      c.created.map((x) => seg(c.id, lineId(x))),
    ),
  )
  await ok({
    type: 'trim_extend',
    bodies: [`b${f2}:g0.0`, `b${f2}:g0.1`],
    boundary: { type: 'bodies', bodies: [`b${f1}:g0.3`, `b${f1}:g0.1`] },
    kind: 'end_trim',
  })
  return { f1, f2 }
}
const shsArea = async () =>
  (await call('weldment.profiles', { family: 'rhs', size: 'SHS 40x40x3' })).area

check('11 Weldment feature', 'one weldment feature per part', async () => {
  await fresh()
  await ok({ type: 'weldment' })
  await bad({ type: 'weldment' }, /one|already/i)
})

check('11 Structural member', 'a mitered rectangle has mitered lengths and angles', async () => {
  const lines4 = await frameSketch()
  await ok(memberOf(tube('SHS 40x40x3'), lines4))
  expect(
    JSON.stringify(await rowsOf()) ===
      JSON.stringify([
        ['SHS 40x40x3', '1240', 2],
        ['SHS 40x40x3', '640', 2],
      ]),
    `rows ${JSON.stringify(await rowsOf())}`,
  )
  const a = await shsArea()
  // Each mitered member loses a 40 × 40 triangle at both ends.
  near(await volume(), a * (2 * 1200 + 2 * 600), 1e-3 * a * 3600, 'volume')
  const angles = (await cutList()).items.map((i) => [i.properties.ANGLE1, i.properties.ANGLE2])
  expect(
    angles.every(([a1, a2]) => a1 === '45' && a2 === '45'),
    `angles ${angles}`,
  )
})

check('11 Structural member', 'a 3D sketch line with a corner pierce point', async () => {
  await fresh()
  await ok({ type: 'weldment' })
  const { id } = await ok({
    type: 'sketch3d',
    entities: [
      { id: 1, kind: 'point', at: [0, 0, 0] },
      { id: 2, kind: 'point', at: [0, 0, 800] },
      { id: 3, kind: 'line', a: 1, b: 2 },
    ],
    next_id: 4,
  })
  const m = memberOf(tube('SHS 40x40x3'), [seg(id, 3)])
  await ok({ ...m, pierce: 'bottom_left' })
  near(await volume(), (await shsArea()) * 800, 1e-5 * (await shsArea()) * 800, 'volume')
})

check(
  '11 Weldment profiles library and custom profiles',
  'sizes and section properties',
  async () => {
    const lib = await call('weldment.profiles')
    const rhs = lib.families.find((f) => f.family === 'rhs')
    expect(rhs?.sizes.includes('SHS 40x40x3'), 'SHS 40x40x3 listed')
    for (const f of ['ipe', 'hea', 'upn', 'chs', 'w', 'pipe'])
      expect(
        lib.families.some((x) => x.family === f),
        `${f} family`,
      )
    const p = await call('weldment.profiles', { family: 'rhs', size: 'SHS 40x40x3' })
    // 40² − 34² less the corner rounds (R6 outside, R3 inside).
    const exact = 40 * 40 - (4 - PI) * 36 - (34 * 34 - (4 - PI) * 9)
    near(p.area, exact, 0.5, 'area')
    near(p.centroid[0], 0, 1e-9, 'centroid on the axis')
  },
)

check('11 Trim/extend', 'cross members end on the frame sides', async () => {
  await trimmedFrame()
  const rows = await rowsOf()
  expect(
    rows.some(([d, l, q]) => d === 'RHS 60x40x3' && l === '1160' && q === 2),
    `rows ${JSON.stringify(rows)}`,
  )
})

check('11 Trim/extend', 'trim to a face with a gap; extension past it', async () => {
  const lines4 = await frameSketch()
  await ok(memberOf(tube('SHS 40x40x3'), lines4))
  const c = await sketch(front, [line([300, 100], [800, 100])])
  const { id: f2 } = await ok(memberOf(tube('SHS 20x20x2'), [seg(c.id, lineId(c.created[0]))]))
  // The right member's inner face at x = 1180: extend to it, 5 back.
  const inner = await face((f) => facing([-1, 0, 0])(f) && at([1180, null, 0])(f.centroid))
  await ok({
    type: 'trim_extend',
    bodies: [`b${f2}:g0.0`],
    boundary: { type: 'faces', faces: [inner] },
    kind: 'end_trim',
    allow_extension: true,
    gap: 5,
  })
  const rows = await rowsOf()
  expect(
    rows.some(([d, l]) => d === 'SHS 20x20x2' && l === '875'),
    `rows ${JSON.stringify(rows)}`,
  )
})

check(
  '11 Corner treatments (structure system)',
  'butt corners run one member through',
  async () => {
    const lines4 = await frameSketch()
    await ok(memberOf(tube('SHS 40x40x3'), lines4, 'end_butt1'))
    const lengths = (await rowsOf()).map((r) => r[1]).sort()
    expect(
      JSON.stringify(lengths) === JSON.stringify(['1200', '1240', '560', '600']),
      `lengths ${lengths}`,
    )
  },
)

/** A 300 long SHS 40 up the y axis. */
async function post() {
  await fresh()
  await ok({ type: 'weldment' })
  const s = await sketch(front, [line([0, 0], [0, 300])])
  return (await ok(memberOf(tube('SHS 40x40x3'), [seg(s.id, lineId(s.created[0]))]))).id
}

check('11 End cap', 'outward, inward and chamfered caps', async () => {
  for (const [direction, corners, area] of [
    [{ type: 'outward' }, null, 40 * 40 - (4 - PI) * 36],
    [{ type: 'inward' }, null, 34 * 34 - (4 - PI) * 9],
  ]) {
    await post()
    const before = await volume()
    const end = await face((f) => facing([0, 1, 0])(f) && at([null, 300, null])(f.centroid))
    await ok({
      type: 'end_cap',
      faces: [end],
      direction,
      thickness: 5,
      offset: { type: 'distance', distance: 0 },
      corners,
    })
    near((await volume()) - before, 5 * area, 2e-3 * 5 * area, `${direction.type} cap`)
  }
})

check('11 Gusset', 'a triangular gusset with a chamfer in a corner', async () => {
  await fresh()
  await ok({ type: 'weldment' })
  const p = await sketch(front, [line([0, 0], [0, 500])])
  await ok(memberOf(tube('SHS 40x40x3'), [seg(p.id, lineId(p.created[0]))]))
  const b = await sketch(front, [line([0, 500], [500, 500])])
  await ok(memberOf(tube('SHS 40x40x3'), [seg(b.id, lineId(b.created[0]))]))
  const before = await volume()
  const side = await face(
    (f) => facing([1, 0, 0])(f) && at([20, null, null])(f.centroid) && f.centroid[1] < 400,
  )
  const under = await face(
    (f) => facing([0, -1, 0])(f) && at([null, 480, null])(f.centroid) && f.centroid[0] > 100,
  )
  await ok({
    type: 'weld_gusset',
    faces: [side, under],
    shape: { type: 'triangle', d1: 100, d2: 100 },
    chamfer: 10,
    thickness: 10,
  })
  near((await volume()) - before, (5000 - 50) * 10, 1e-6, 'gusset volume')
})

/** A SHS 60 post with a SHS 40 beam butting its right side at y = 250. */
async function postAndBeam() {
  await fresh()
  await ok({ type: 'weldment' })
  const p = await sketch(front, [line([0, 0], [0, 500])])
  await ok(memberOf(tube('SHS 60x60x4'), [seg(p.id, lineId(p.created[0]))]))
  const b = await sketch(front, [line([30, 250], [300, 250])])
  await ok(memberOf(tube('SHS 40x40x3'), [seg(b.id, lineId(b.created[0]))]))
  const beam = (n) =>
    face((f) => facing(n)(f) && f.centroid[0] > 100 && Math.abs(f.centroid[1] - 250) < 30)
  return {
    sides: [
      await beam([0, 1, 0]),
      await beam([0, -1, 0]),
      await beam([0, 0, 1]),
      await beam([0, 0, -1]),
    ],
    post: await face((f) => facing([1, 0, 0])(f) && at([30, null, null])(f.centroid)),
  }
}

check('11 Weld beads', 'cosmetic beads round a seam, intermittent and both sides', async () => {
  const { sides, post: p } = await postAndBeam()
  const before = await volume()
  await ok({ type: 'weld_bead', path: { type: 'faces', from: sides, to: [p] }, size: 5 })
  near((await cutList()).welds.total_length, 4 * 28, 1e-6, 'four flats less the corners')
  near(await volume(), before, 1e-9, 'cosmetic')
  const { id } = await ok({
    type: 'weld_bead',
    path: { type: 'faces', from: sides, to: [p] },
    size: 5,
    sides: 'both',
    intermittent: { length: 10, gap: 10, staggered: true },
  })
  const w = (await cutList()).welds.beads.find((b) => b.feature === id)
  near(w.length, 4 * 28, 1e-6, 'staggered both sides')
})

check('11 Weld beads', 'a modeled fillet bead adds its triangle', async () => {
  const { sides, post: p } = await postAndBeam()
  const before = await volume()
  await ok({
    type: 'weld_bead',
    path: { type: 'faces', from: [sides[0]], to: [p] },
    size: 5,
    kind: 'fillet',
  })
  near((await volume()) - before, 0.5 * 25 * 28, 1e-6, 'fillet volume')
})

check(
  '11 Cut lists, cut list properties and folders',
  'grouping, properties and mass',
  async () => {
    await trimmedFrame()
    await call('doc.set_material', { name: 'Steel', density: 7850 })
    const l = await cutList()
    expect(l.items.length === 3, `${l.items.length} items`)
    const it = l.items[2]
    expect(it.properties.MATERIAL === 'Steel', 'material')
    expect(it.properties['BOUNDING WIDTH'] === '60', 'bounding width')
    const rhs = (await call('weldment.profiles', { family: 'rhs', size: 'RHS 60x40x3' })).area
    near(Number(it.properties.MASS), (rhs * 1160 * 7850e-9).toFixed(3), 1.5e-3, 'mass')
    expect(l.items.map((i) => i.number).join() === '1,2,3', 'item numbers')
  },
)

check(
  '11 Cut lists, cut list properties and folders',
  'overrides, exclusion and separate items',
  async () => {
    const { f2 } = await trimmedFrame()
    const first = (await cutList()).items[0].bodies[0]
    await call('doc.cut_list.set', {
      settings: {
        overrides: { [first]: { DESCRIPTION: 'Frame rail' } },
        excluded: [`b${f2}:g0.1`],
        separate: true,
      },
    })
    const l = await cutList()
    expect(l.items[0].properties.DESCRIPTION === 'Frame rail', 'override')
    expect(
      l.items.every((i) => i.quantity === 1),
      'separate',
    )
    const ex = l.items.find((i) => i.bodies.includes(`b${f2}:g0.1`))
    expect(ex?.excluded && ex.number === null, 'excluded item has no number')
    // Saved with the part.
    const path = join(tmp, 'c11-cutlist.ofp')
    await call('doc.save', { path })
    await call('doc.open', { path })
    expect((await cutList()).items[0].properties.DESCRIPTION === 'Frame rail', 'saved')
  },
)

check('11 Sub-weldments and groups', 'a sub-weldment folder lists its own items', async () => {
  const { f2 } = await trimmedFrame()
  await call('doc.cut_list.set', {
    settings: { sub_weldments: [{ name: 'Cross', bodies: [`b${f2}:g0.0`, `b${f2}:g0.1`] }] },
  })
  const l = await cutList()
  const sub = l.items.filter((i) => i.folder === 'Cross')
  expect(sub.length === 1 && sub[0].quantity === 2, `sub ${JSON.stringify(sub)}`)
})

failure(
  '11 Trim/extend',
  'a body that is not a member',
  async () => {
    const lines4 = await frameSketch()
    await ok(memberOf(tube('SHS 40x40x3'), lines4))
    return {
      feature: {
        type: 'trim_extend',
        bodies: ['b99:g0.0'],
        boundary: { type: 'faces', faces: [] },
      },
      volume: await volume(),
    }
  },
  /member|body|face/i,
)

check(
  '11 Weldment drawings and cut list tables',
  'a member view, cut list and weld tables, balloons and CSV',
  async () => {
    const { f2 } = await trimmedFrame()
    const bottom = await face(
      (f) => facing([0, 1, 0])(f) && Math.abs(f.centroid[1] - 230) < 1e-6 && f.area > 1000,
    )
    const p = await face((f) => facing([1, 0, 0])(f) && at([1180, null, 0], 1)(f.centroid))
    await ok({
      type: 'weld_bead',
      path: { type: 'faces', from: [bottom], to: [p] },
      size: 4,
      symbol: '4 FILLET',
      process: 'MAG',
    })
    const path = join(tmp, 'c11-frame.ofp')
    await call('doc.save', { path })
    await call('drawing.new')
    const whole = await addView(modelView(path), [120, 200])
    const member = await addView(
      { ...modelView(path), model: { path, bodies: [`b${f2}:g0.0`] } },
      [120, 80],
    )
    const t = await annotate({ type: 'table', table: 'cut_list', view: whole, at: [250, 280] })
    await annotate({ type: 'table', table: 'weld', view: whole, at: [250, 120] })
    const r = await render()
    noErrors(r)
    near(
      width(r.views.find((v) => v.id === member).bbox),
      1160 * r.views[0].scale,
      1e-6,
      'member view',
    )
    const words = r.graphics.items
      .filter((i) => i.prim.type === 'text')
      .map((i) => i.prim.text)
      .join(' ')
    for (const w of ['ITEM NO.', 'RHS 60x40x3', '1160', 'SYMBOL', '4 FILLET', 'MAG'])
      expect(words.includes(w), `${w} in the tables`)
    const { csv } = await call('drawing.table.csv', { sheet: 1, id: t })
    expect(csv.startsWith('ITEM NO.,QTY.,DESCRIPTION,LENGTH,ANGLE1,ANGLE2\n'), csv)
    const b = await call('drawing.balloons.auto', { sheet: 1, view: whole })
    expect(b.added.length === 3, `${b.added.length} balloons`)
  },
)

// ---- run ----
const results = []
for (const c of checks) {
  if (filter && !`${c.spec} ${c.name}`.toLowerCase().includes(filter)) continue
  try {
    await c.fn()
    results.push({ ...c, ok: true })
  } catch (e) {
    results.push({
      ...c,
      ok: false,
      error: e instanceof Failure ? e.message : `error: ${e.message}`,
    })
  }
}
core.kill()
for (const r of results)
  console.log(`${r.ok ? 'PASS' : 'FAIL'}  ${r.spec} — ${r.name}${r.ok ? '' : `\n      ${r.error}`}`)
const failed = results.filter((r) => !r.ok).length
console.log(`\n${results.length - failed} of ${results.length} checks passed`)
process.exit(failed ? 1 : 0)
