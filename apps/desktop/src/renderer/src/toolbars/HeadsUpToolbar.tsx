import { Toolbar, ToolbarControl, type Control } from './controls'
const controls: Control[] = [
  { command: 'fit' },
  { command: 'zoomArea' },
  { command: 'previousView' },
  {
    label: 'Orientation',
    icon: 'orientation',
    menu: ['front', 'back', 'left', 'right', 'top', 'bottom', 'iso'],
  },
  { command: 'normal' },
  { command: 'section' },
  { command: 'display' },
  { command: 'visibility' },
  { command: 'appearance' },
  { command: 'scene' },
  { command: 'viewSettings' },
]
export function HeadsUpToolbar() {
  return (
    <Toolbar label="Heads-Up View" className="of-heads-up">
      {controls.map((control, i) => (
        <ToolbarControl key={i} control={{ ...control, variant: 'icon' }} />
      ))}
    </Toolbar>
  )
}
