import type { Tab } from '../store'
import type { Control } from './controls'

export interface CommandGroup {
  label: string
  columns: Control[][]
}
export const featureGroups: CommandGroup[] = [
  {
    label: 'Boss/base',
    columns: [
      [{ command: 'extrude', variant: 'large' }],
      [{ command: 'revolve', variant: 'large' }],
      [{ command: 'sweep' }, { command: 'loft' }, { command: 'boundary' }],
    ],
  },
  {
    label: 'Cuts',
    columns: [
      [{ command: 'cut', variant: 'large' }],
      [{ command: 'hole', variant: 'large' }],
      [{ command: 'cutRevolve', variant: 'large' }],
      [{ command: 'sweepCut' }, { command: 'loftCut' }, { command: 'boundaryCut' }],
    ],
  },
  {
    label: 'Modification',
    columns: [
      [
        {
          command: 'fillet',
          variant: 'large',
          menu: ['fillet', 'variableFillet', 'faceFillet', 'chamfer'],
        },
      ],
      [
        {
          label: 'Pattern',
          icon: 'pattern',
          variant: 'large',
          menu: ['linearPattern', 'circularPattern', 'mirror'],
        },
      ],
      [{ command: 'shell' }, { command: 'draft' }, { command: 'rib' }],
      [{ command: 'wrap' }, { command: 'intersect' }, { command: 'cosmeticThread' }],
      [{ command: 'thread' }],
      [{ command: 'moveBody' }, { command: 'splitBody' }],
      [{ command: 'dome' }],
    ],
  },
  {
    label: 'Reference',
    columns: [
      [
        {
          label: 'Reference Geometry',
          icon: 'plane',
          variant: 'large',
          menu: ['plane', 'axis', 'point', 'coordinates'],
        },
      ],
      [
        {
          command: 'curves',
          variant: 'large',
          menu: ['curves', 'compositeCurve', 'projectedCurve', 'splitLine'],
        },
      ],
      [{ command: 'direct', variant: 'large' }],
    ],
  },
]
export const sketchGroups = (editing: boolean): CommandGroup[] => [
  {
    label: 'Sketch',
    columns: [
      [{ command: editing ? 'exit' : 'sketch', variant: 'large' }],
      [{ command: 'sketch3d' }, { command: 'dimension' }],
    ],
  },
  {
    label: 'Geometry',
    columns: [
      [{ command: 'line' }, { command: 'centerline' }, { command: 'sketchSelect' }],
      [
        { command: 'rectangle', menu: ['rectangle', 'centerRectangle'] },
        { command: 'circle' },
        { command: 'centerArc', menu: ['centerArc', 'arc3', 'tangentArc'] },
      ],
      [{ command: 'slot' }, { command: 'polygon' }, { command: 'spline' }],
      [{ command: 'sketchPoint' }],
    ],
  },
  {
    label: 'Edit',
    columns: [
      [{ command: 'trim' }, { command: 'extend' }, { command: 'sketchFillet' }],
      [{ command: 'convert', variant: 'large' }],
    ],
  },
]
export const evaluateGroups: CommandGroup[] = [
  {
    label: 'Analysis',
    columns: [[{ command: 'measure', variant: 'large' }], [{ command: 'mass', variant: 'large' }]],
  },
  {
    label: 'Document',
    columns: [
      [{ command: 'material', variant: 'large' }],
      [{ command: 'units', variant: 'large' }],
      [{ command: 'equations', variant: 'large' }],
      [{ command: 'configurations', variant: 'large' }],
    ],
  },
  { label: 'Exchange', columns: [[{ command: 'export', variant: 'large' }]] },
]
// Placeholder tabs never enter the store's selectable Tab union.
export const availableTabs: { id: Tab; label: string }[] = [
  { id: 'features', label: 'Features' },
  { id: 'sketch', label: 'Sketch' },
  { id: 'evaluate', label: 'Evaluate' },
]
export const unavailableTabs = [
  'Surfaces',
  'Sheet Metal',
  'Direct Editing',
  'Annotations',
  'Add-ins',
] as const
export const tabOrder = [
  'Features',
  'Sketch',
  'Surfaces',
  'Sheet Metal',
  'Direct Editing',
  'Evaluate',
  'Annotations',
  'Add-ins',
]
