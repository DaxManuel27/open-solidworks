import { AdvancedDressupPanel, CurvePanel, SweepLoftPanel } from './advanced'
import { ConfigurationsPanel } from './configurations'
import { EquationsPanel } from './equations'
import { BodiesPanel } from './bodies'
import type { Panel } from '../store'
import {
  ChamferPanel,
  CircularPatternPanel,
  ExtrudePanel,
  FilletPanel,
  HolePanel,
  LinearPatternPanel,
  MirrorPanel,
  RevolvePanel,
  ShellPanel,
} from './features'
import { AxisPanel, CoordSysPanel, PlanePanel, PointPanel } from './reference'
import { ExportPanel, MassPanel, MaterialPanel, MeasurePanel, UnitsPanel } from './tools'

/** The PropertyManager for the open panel. Keyed so each opening starts fresh. */
export function PanelHost({ panel }: { panel: Panel }) {
  const key = `${panel.type}:${panel.editing ?? 'new'}:${panel.sketch ?? ''}`
  switch (panel.type) {
    case 'draft':
    case 'variable_fillet':
      return <AdvancedDressupPanel key={key} panel={panel} />
    case 'sweep':
    case 'sweep_cut':
    case 'loft':
    case 'loft_cut':
      return <SweepLoftPanel key={key} panel={panel} />
    case 'helix':
    case 'composite_curve':
    case 'split_line':
    case 'projected_curve':
      return <CurvePanel key={key} panel={panel} />
    case 'configurations':
      return <ConfigurationsPanel key={key} />
    case 'equations':
      return <EquationsPanel key={key} />
    case 'combine':
    case 'move_body':
    case 'split_body':
      return <BodiesPanel key={key} panel={panel} />
    case 'extrude':
    case 'cut':
      return <ExtrudePanel key={key} panel={panel} />
    case 'revolve':
    case 'cut_revolve':
      return <RevolvePanel key={key} panel={panel} />
    case 'fillet':
      return <FilletPanel key={key} panel={panel} />
    case 'chamfer':
      return <ChamferPanel key={key} panel={panel} />
    case 'shell':
      return <ShellPanel key={key} panel={panel} />
    case 'hole':
      return <HolePanel key={key} panel={panel} />
    case 'linear_pattern':
      return <LinearPatternPanel key={key} panel={panel} />
    case 'circular_pattern':
      return <CircularPatternPanel key={key} panel={panel} />
    case 'mirror':
      return <MirrorPanel key={key} panel={panel} />
    case 'plane':
      return <PlanePanel key={key} panel={panel} />
    case 'axis':
      return <AxisPanel key={key} panel={panel} />
    case 'point':
      return <PointPanel key={key} panel={panel} />
    case 'coord_sys':
      return <CoordSysPanel key={key} panel={panel} />
    case 'measure':
      return <MeasurePanel key={key} />
    case 'mass':
      return <MassPanel key={key} />
    case 'material':
      return <MaterialPanel key={key} />
    case 'units':
      return <UnitsPanel key={key} />
    case 'export':
      return <ExportPanel key={key} />
  }
}
