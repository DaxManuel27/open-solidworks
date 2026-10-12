import { useEffect, useState } from 'react'
import { call } from '../core'
import type { FeatureParams, Ref, Vec3 } from '../protocol'
import { type Panel, useStore } from '../store'
import { formatLength } from '../units'
import { Choice, LengthField, PanelShell, RefBox } from './fields'
import { useFeature } from './features'

type PlanarFace = { ref: Ref; centroid: Vec3 }

export function DomePanel({ panel }: { panel: Panel }) {
  const [params, setParams] = useState(() => {
    const initial = useStore
      .getState()
      .snapshot?.features.find((f) => f.id === panel.editing)?.params
    const face =
      useStore
        .getState()
        .selection.find(
          (s) => s.ref.type === 'face' || (s.ref.type === 'body_topology' && s.ref.kind === 'face'),
        )?.ref ?? null
    return {
      type: 'dome' as const,
      face: face as Ref | null,
      height: 3,
      inward: false,
      elliptical: false,
      ...initial,
    }
  })
  const feature = params.face && params.height > 0 ? (params as FeatureParams) : null
  const wiring = useFeature(panel, feature)
  const revision = useStore((s) => s.snapshot?.revision)
  const units = useStore((s) => s.snapshot?.units)
  const [faces, setFaces] = useState<PlanarFace[]>([])
  useEffect(() => {
    let current = true
    void call<{ faces: (PlanarFace & { surface: string })[] }>('doc.topology')
      .then(async (topology) => {
        const planar = await Promise.all(
          topology.faces.flatMap((face, index) =>
            face.surface === 'plane'
              ? [
                  call<{ ref: Ref }>('select.ref', { kind: 'face', index }).then(({ ref }) => ({
                    ...face,
                    ref,
                  })),
                ]
              : [],
          ),
        )
        if (current) setFaces(planar)
      })
      .catch(() => {
        if (current) setFaces([])
      })
    return () => {
      current = false
    }
  }, [revision])
  const selected = faces.findIndex(
    (face) => JSON.stringify(face.ref) === JSON.stringify(params.face),
  )
  return (
    <PanelShell title="Dome" {...wiring} okDisabled={wiring.okDisabled || !!wiring.error}>
      <RefBox
        label="Planar face"
        refs={params.face ? [params.face] : []}
        kinds={['face']}
        single
        active
        onActivate={() => {}}
        onChange={(refs) => setParams({ ...params, face: refs[0] ?? null })}
      />
      <Choice
        label="Available planar face"
        value={selected}
        options={[
          { value: -1, label: 'Pick a planar face…' },
          ...faces.map((face, index) => ({
            value: index,
            label: `Face ${index + 1} · ${face.centroid.map((value) => (units ? formatLength(value, units) : `${value} mm`)).join(', ')}`,
          })),
        ]}
        onChange={(index) => setParams({ ...params, face: faces[index]?.ref ?? null })}
      />
      <LengthField
        label="Height"
        path="height"
        value={params.height}
        onChange={(height) => setParams({ ...params, height })}
      />
      <Choice
        label="Shape"
        value={params.elliptical ? 'elliptical' : 'spherical'}
        options={[
          { value: 'spherical', label: 'Spherical cap / smooth dome' },
          { value: 'elliptical', label: 'Elliptical — tangent at the rim' },
        ]}
        onChange={(shape) => setParams({ ...params, elliptical: shape === 'elliptical' })}
      />
      <Choice
        label="Direction"
        value={params.inward ? 'inward' : 'outward'}
        options={[
          { value: 'outward', label: 'Outward — add material' },
          { value: 'inward', label: 'Inward — remove material' },
        ]}
        onChange={(direction) => setParams({ ...params, inward: direction === 'inward' })}
      />
      <p className="hint">
        On a circular disk the dome is an exact spherical cap (height up to the radius) or half an
        ellipsoid that leaves the rim along the face normal. On other planar faces without holes it
        is a smooth surface through the face&apos;s edges and a point the given height above its
        centre; the tangent option keeps it tangent to the side faces, which needs a boundary
        without corners. Inward domes need material beneath the entire dome.
      </p>
    </PanelShell>
  )
}
