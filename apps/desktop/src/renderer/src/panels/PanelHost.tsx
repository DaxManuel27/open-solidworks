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
