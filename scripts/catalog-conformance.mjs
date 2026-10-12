// Checks Phase 1, 2 and 3 behaviour against the feature specs in docs/features,
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
