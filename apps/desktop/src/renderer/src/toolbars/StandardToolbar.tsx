import { Toolbar, ToolbarControl, type Control } from './controls'
const groups: Control[][] = [
  [
    { command: 'new', menu: ['new', 'newAssembly', 'newDrawing', 'makeDrawing'] },
    { command: 'open', menu: ['import', 'importAssembly'] },
    { command: 'save', menu: ['saveAs', 'export'] },
    { command: 'print' },
  ],
  [{ command: 'undo' }, { command: 'redo' }],
  [{ command: 'select' }],
  [{ command: 'rebuild' }, { command: 'properties' }, { command: 'options' }],
]
export function StandardToolbar() {
  return (
    <Toolbar label="Standard" className="of-standard">
      {groups.map((group, index) => (
        <div className="of-standard-group" key={index}>
          {group.map((control, i) => (
            <ToolbarControl key={i} control={{ ...control, variant: 'icon' }} />
          ))}
        </div>
      ))}
    </Toolbar>
  )
}
