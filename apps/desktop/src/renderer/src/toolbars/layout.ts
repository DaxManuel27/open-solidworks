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
export const sheetMetalGroups: CommandGroup[] = [
  {
    label: 'Flanges',
    columns: [
      [{ command: 'baseFlange', variant: 'large' }],
      [{ command: 'edgeFlange', variant: 'large' }],
      [{ command: 'miterFlange' }, { command: 'hem' }, { command: 'sweptFlange' }],
      [{ command: 'loftedBend' }],
    ],
  },
  {
    label: 'Bends',
    columns: [
      [{ command: 'sketchedBend' }, { command: 'jog' }, { command: 'crossBreak' }],
      [{ command: 'insertBends' }, { command: 'convertToSheetMetal' }, { command: 'rip' }],
      [{ command: 'unfold' }, { command: 'fold' }, { command: 'normalCut' }],
    ],
  },
  {
    label: 'Corners',
    columns: [
      [
        {
          command: 'cornerRelief',
          variant: 'large',
          menu: ['cornerRelief', 'closedCorner', 'weldedCorner', 'breakCorner'],
        },
      ],
      [{ command: 'bendNotch' }, { command: 'closedCorner' }, { command: 'breakCorner' }],
    ],
  },
  {
    label: 'Details',
    columns: [
      [{ command: 'tabAndSlot' }, { command: 'sheetMetalGusset' }],
      [{ command: 'formTool' }, { command: 'formingToolDef' }],
    ],
  },
  {
    label: 'Flat',
    columns: [[{ command: 'flatten', variant: 'large' }], [{ command: 'flatPattern' }]],
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
  {
    label: 'Exchange',
    columns: [
      [{ command: 'export', variant: 'large' }],
      [{ command: 'makeDrawing', variant: 'large' }],
    ],
  },
]
export const assemblyGroups: CommandGroup[] = [
  {
    label: 'Components',
    columns: [
      [{ command: 'insertComponents', variant: 'large' }],
      [{ command: 'hardware', variant: 'large' }],
    ],
  },
  {
    label: 'Mates',
    columns: [
      [{ command: 'mate', variant: 'large', menu: ['mate', 'advancedMate', 'mechanicalMate'] }],
    ],
  },
  {
    label: 'Move',
    columns: [
      [{ command: 'moveComponent', variant: 'large' }],
      [{ command: 'rotateComponent', variant: 'large' }],
    ],
  },
  {
    label: 'Patterns',
    columns: [
      [
        {
          command: 'linearComponentPattern',
          variant: 'large',
          menu: [
            'linearComponentPattern',
            'circularComponentPattern',
            'patternDrivenPattern',
            'sketchDrivenPattern',
            'curveDrivenPattern',
          ],
        },
      ],
      [{ command: 'mirrorComponents', variant: 'large' }],
    ],
  },
  {
    label: 'Tools',
    columns: [
      [{ command: 'explodedView', variant: 'large' }],
      [{ command: 'interference' }, { command: 'clearance' }, { command: 'bom' }],
      [
        {
          label: 'Reference Geometry',
          icon: 'plane',
          variant: 'large',
          menu: ['asmPlane', 'asmAxis'],
        },
      ],
    ],
  },
]
export const assemblyEvaluateGroups: CommandGroup[] = [
  {
    label: 'Analysis',
    columns: [
      [{ command: 'interference', variant: 'large' }],
      [{ command: 'clearance', variant: 'large' }],
      [{ command: 'asmMeasure', variant: 'large' }],
      [{ command: 'asmMass', variant: 'large' }],
    ],
  },
  {
    label: 'Document',
    columns: [
      [{ command: 'bom', variant: 'large' }],
      [{ command: 'asmConfigurations', variant: 'large' }],
      [{ command: 'units', variant: 'large' }],
    ],
  },
  {
    label: 'Exchange',
    columns: [
      [{ command: 'export', variant: 'large' }],
      [{ command: 'makeDrawing', variant: 'large' }],
    ],
  },
]
export const drawingGroups: CommandGroup[] = [
  {
    label: 'Views',
    columns: [
      [{ command: 'standardViews', variant: 'large' }],
      [{ command: 'modelView', variant: 'large' }],
      [{ command: 'projectedView' }, { command: 'auxiliaryView' }],
      [
        { command: 'sectionView', menu: ['sectionView', 'alignedSection'] },
        { command: 'detailView' },
      ],
    ],
  },
  {
    label: 'Modify',
    columns: [
      [{ command: 'brokenOut' }, { command: 'breakView' }, { command: 'cropView' }],
      [{ command: 'updateViews', variant: 'large' }],
    ],
  },
  {
    label: 'Sheet',
    columns: [
      [{ command: 'addSheet' }, { command: 'sheetProperties' }],
      [{ command: 'drawingProperties' }, { command: 'draftingStandard' }],
    ],
  },
]
export const annotationGroups: CommandGroup[] = [
  {
    label: 'Dimensions',
    columns: [
      [
        {
          command: 'smartDimension',
          variant: 'large',
          menu: [
            'smartDimension',
            'ordinateDimension',
            'baselineDimension',
            'chainDimension',
            'chamferDimension',
            'angularRunning',
            'arcLength',
          ],
        },
      ],
      [{ command: 'modelItems', variant: 'large' }],
    ],
  },
  {
    label: 'Annotations',
    columns: [
      [{ command: 'note', variant: 'large' }],
      [{ command: 'balloon', menu: ['balloon', 'autoBalloon'] }, { command: 'holeCallout' }],
      [{ command: 'surfaceFinish' }, { command: 'weldSymbol' }],
      [
        { command: 'geometricTolerance' },
        { command: 'datumFeature', menu: ['datumFeature', 'datumTarget'] },
      ],
      [{ command: 'centerMark' }, { command: 'drawCenterline' }],
      [
        { command: 'areaHatch' },
        { command: 'revisionCloud', menu: ['revisionCloud', 'revisionSymbol'] },
      ],
    ],
  },
  {
    label: 'Tables',
    columns: [
      [
        {
          command: 'bomTable',
          variant: 'large',
          menu: [
            'bomTable',
            'holeTable',
            'revisionTable',
            'generalTable',
            'toleranceTable',
            'bendTable',
            'punchTable',
          ],
        },
      ],
    ],
  },
]
export const drawingEvaluateGroups: CommandGroup[] = [
  {
    label: 'Document',
    columns: [
      [{ command: 'drawingProperties', variant: 'large' }],
      [{ command: 'draftingStandard', variant: 'large' }],
    ],
  },
  {
    label: 'Output',
    columns: [
      [{ command: 'drawingExport', variant: 'large' }],
      [{ command: 'print', variant: 'large' }],
    ],
  },
]
export const drawingTabs: { id: Tab; label: string }[] = [
  { id: 'drawing', label: 'View Layout' },
  { id: 'annotation', label: 'Annotation' },
  { id: 'evaluate', label: 'Evaluate' },
]
export const drawingTabOrder = ['View Layout', 'Annotation', 'Sketch', 'Evaluate', 'Add-ins']
export const assemblyTabs: { id: Tab; label: string }[] = [
  { id: 'assembly', label: 'Assembly' },
  { id: 'evaluate', label: 'Evaluate' },
]
export const assemblyTabOrder = [
  'Assembly',
  'Layout',
  'Sketch',
  'Evaluate',
  'Annotations',
  'Add-ins',
]
// Placeholder tabs never enter the store's selectable Tab union.
export const availableTabs: { id: Tab; label: string }[] = [
  { id: 'features', label: 'Features' },
  { id: 'sheet_metal', label: 'Sheet Metal' },
  { id: 'sketch', label: 'Sketch' },
  { id: 'evaluate', label: 'Evaluate' },
]
export const unavailableTabs = [
  'Layout',
  'Surfaces',
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
