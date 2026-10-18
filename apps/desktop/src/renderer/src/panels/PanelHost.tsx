import { CosmeticThreadPanel } from './cosmetic-thread'
import { SpatialPanel } from './spatial'
import { DomePanel } from './dome'
import { FaceFilletPanel } from './face-fillet'
import { RibPanel } from './rib'
import { WrapPanel } from './wrap'
import { AdvancedDressupPanel, BoundaryPanel, CurvePanel, SweepLoftPanel } from './advanced'
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
import {
  AsmConfigurationsPanel,
  AsmMassPanel,
  AsmMeasurePanel,
  AsmReferencePanel,
  BomPanel,
  ClearancePanel,
  ComponentPatternPanel,
  ComponentPropertiesPanel,
  ExplodePanel,
  HardwarePanel,
  InsertComponentsPanel,
  InterferencePanel,
  MirrorComponentsPanel,
  MovePanel,
} from './assembly'
import { MatePanel } from './mates'
import {
  AnnotationPropertiesPanel,
  AreaHatchPanel,
  AutoBalloonPanel,
  BalloonPanel,
  CenterMarkPanel,
  CenterlinePanel,
  HoleCalloutPanel,
  NotePanel,
  RevisionCloudPanel,
  SymbolPanel,
  TablePanel,
} from '../drawing/panels/annotate'
import { ModelItemsPanel, SmartDimensionPanel } from '../drawing/panels/dimensions'
import {
  DraftingStandardPanel,
  DrawingExportPanel,
  DrawingPropertiesPanel,
  PartPropertiesPanel,
  SheetPropertiesPanel,
} from '../drawing/panels/sheet'
import {
  AuxiliaryViewPanel,
  BreakViewPanel,
  BrokenOutPanel,
  CropViewPanel,
  DetailViewPanel,
  ModelViewPanel,
  ProjectedViewPanel,
  SectionViewPanel,
  StandardViewsPanel,
  ViewPropertiesPanel,
} from '../drawing/panels/views'

/** The PropertyManager for the open panel. Keyed so each opening starts fresh. */
export function PanelHost({ panel }: { panel: Panel }) {
  const key = `${panel.type}:${panel.editing ?? 'new'}:${panel.sketch ?? ''}:${panel.variant ?? ''}`
  switch (panel.type) {
    case 'sheet_properties':
      return <SheetPropertiesPanel key={key} editing={panel.editing} />
    case 'drawing_properties':
      return <DrawingPropertiesPanel key={key} />
    case 'part_properties':
      return <PartPropertiesPanel key={key} />
    case 'drafting_standard':
      return <DraftingStandardPanel key={key} />
    case 'drawing_export':
      return <DrawingExportPanel key={key} />
    case 'standard_views':
      return <StandardViewsPanel key={key} />
    case 'model_view':
      return <ModelViewPanel key={key} />
    case 'projected_view':
      return <ProjectedViewPanel key={key} />
    case 'auxiliary_view':
      return <AuxiliaryViewPanel key={key} />
    case 'section_view':
      return <SectionViewPanel key={key} variant={panel.variant} />
    case 'detail_view':
      return <DetailViewPanel key={key} />
    case 'crop_view':
      return <CropViewPanel key={key} />
    case 'broken_out':
      return <BrokenOutPanel key={key} />
    case 'break_view':
      return <BreakViewPanel key={key} />
    case 'view_properties':
      return <ViewPropertiesPanel key={key} editing={panel.editing} />
    case 'smart_dimension':
      return <SmartDimensionPanel key={key} variant={panel.variant} />
    case 'model_items':
      return <ModelItemsPanel key={key} />
    case 'note':
      return <NotePanel key={key} />
    case 'balloon':
      return <BalloonPanel key={key} />
    case 'auto_balloon':
      return <AutoBalloonPanel key={key} />
    case 'symbol':
      return <SymbolPanel key={key} variant={panel.variant} />
    case 'hole_callout':
      return <HoleCalloutPanel key={key} />
    case 'center_mark':
      return <CenterMarkPanel key={key} />
    case 'centerline':
      return <CenterlinePanel key={key} />
    case 'area_hatch':
      return <AreaHatchPanel key={key} />
    case 'revision_cloud':
      return <RevisionCloudPanel key={key} variant={panel.variant} />
    case 'table':
      return <TablePanel key={key} variant={panel.variant} />
    case 'annotation_properties':
      return <AnnotationPropertiesPanel key={key} editing={panel.editing} />
    case 'asm_insert':
      return <InsertComponentsPanel key={key} />
    case 'hardware':
      return <HardwarePanel key={key} />
    case 'mate':
      return <MatePanel key={key} panel={panel} />
    case 'move_component':
    case 'rotate_component':
      return <MovePanel key={key} panel={panel} />
    case 'component_pattern':
      return <ComponentPatternPanel key={key} panel={panel} />
    case 'mirror_components':
      return <MirrorComponentsPanel key={key} panel={panel} />
    case 'explode':
      return <ExplodePanel key={key} />
    case 'interference':
      return <InterferencePanel key={key} />
    case 'clearance':
      return <ClearancePanel key={key} />
    case 'bom':
      return <BomPanel key={key} />
    case 'component_properties':
      return <ComponentPropertiesPanel key={key} panel={panel} />
    case 'asm_configurations':
      return <AsmConfigurationsPanel key={key} />
    case 'asm_reference':
      return <AsmReferencePanel key={key} panel={panel} />
    case 'asm_measure':
      return <AsmMeasurePanel key={key} />
    case 'asm_mass':
      return <AsmMassPanel key={key} />
    case 'dome':
      return <DomePanel key={key} panel={panel} />
    case 'thread':
      return <CosmeticThreadPanel key={key} panel={panel} />
    case 'face_fillet':
      return <FaceFilletPanel key={key} panel={panel} />
    case 'rib':
      return <RibPanel key={key} panel={panel} />
    case 'wrap':
      return <WrapPanel key={key} panel={panel} />
    case 'cosmetic_thread':
      return <CosmeticThreadPanel key={key} panel={panel} />
    case 'sketch3d':
      return <SpatialPanel key={key} panel={panel} />
    case 'draft':
    case 'variable_fillet':
      return <AdvancedDressupPanel key={key} panel={panel} />
    case 'sweep':
    case 'sweep_cut':
    case 'loft':
    case 'loft_cut':
      return <SweepLoftPanel key={key} panel={panel} />
    case 'boundary':
    case 'boundary_cut':
      return <BoundaryPanel key={key} panel={panel} />
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
