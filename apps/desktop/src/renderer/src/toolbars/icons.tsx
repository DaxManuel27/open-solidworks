/** Original, grid-drawn vectors shared by the three toolbars. */
const paths = {
  new: 'M6 3h8l4 4v14H6z M14 3v5h4',
  open: 'M3 7h7l2 3h9l-3 10H3z M3 10V5h7l2 2h7v3',
  save: 'M4 3h14l3 3v15H3V3z M7 3v6h10V3 M7 21v-8h10v8 M14 5v2',
  undo: 'M9 5 3 10l6 5 M3 10h11a6 6 0 0 1 0 12',
  redo: 'm15 5 6 5-6 5 M21 10H10a6 6 0 0 0 0 12',
  select: 'm5 3 14 10-7 1-3 7z',
  print: 'M7 8V3h10v5 M6 17H3V9h18v8h-3 M7 13h10v8H7z M17 11h1',
  rebuild: 'M19 8a8 8 0 1 0 1 8 M19 3v6h-6 M10 8l-2 5h5l-2 5',
  properties: 'M5 3h14v18H5z M8 7h8 M8 11h8 M8 15h5',
  options:
    'm9 3-1 3-3 1-2 3 2 2-1 4 3 1 2 4h4l2-3 4-1 2-4-3-2V7l-4-1-1-3z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
  extrude: 'm3 9 8-5 10 4-8 5z M3 9v10l10 3 8-6V8 M13 13v9 M11 4v7',
  revolve: 'M9 5c-8 0-8 14 0 14V5z M13 3v18 M17 6c6 3 6 9 0 12 M17 14v4h4',
  sweep: 'M4 18C4 3 20 21 20 6 M2 15l5 1v5l-5-1z M17 3h5v5h-5z',
  loft: 'm3 19 7 3 11-5-6-3z M7 4l5-2 5 2-5 3z M7 4 3 19 M17 4l4 13 M12 7l-2 15',
  cut: 'm3 9 9-5 9 5v10l-9 3-9-3z M3 9l9 4 9-4 M12 13v9 M8 8h8 M12 6v4',
  hole: 'm3 9 9-5 9 5v10l-9 3-9-3z M3 9l9 4 9-4 M12 13v9 M16 8c0 3-8 3-8 0s8-3 8 0',
  fillet: 'M3 21V3h10a8 8 0 0 1 8 8v10z M3 8h8a5 5 0 0 1 5 5v8 M16 13l5-2',
  chamfer: 'M3 21V3h11l7 7v11z M3 8h9l4 5v8 M12 8l2-5 M16 13l5-3',
  pattern: 'M3 3h6v6H3z M15 3h6v6h-6z M3 15h6v6H3z M15 15h6v6h-6z',
  circular: 'M9 2h6v5H9z M17 13h5v6h-5z M2 13h5v6H2z M4 10l3-3 M17 7l3 3 M9 19h6',
  mirror: 'M12 2v20 M3 6l6 3v10l-6-3z M21 6l-6 3v10l6-3z',
  shell: 'm3 8 9-5 9 5v11l-9 3-9-3z M3 8l9 5 9-5 M12 13v9 M7 8l5-3 5 3-5 2z',
  draft: 'M4 20 8 4h8l4 16z M8 4v16 M16 4v16',
  dome: 'M3 17a9 9 0 0 1 18 0 M3 17h18 M3 17v3h18v-3',
  rib: 'M3 3v18h18v-4H7V3z M7 7l10 10H7z',
  wrap: 'M5 5c0-4 14-4 14 0v14c0 4-14 4-14 0z M5 5c0 4 14 4 14 0 M3 12c4-4 14 4 18 0',
  intersect: 'M3 3h12v12H3z M9 9h12v12H9z M9 9h6v6H9z',
  plane: 'm3 17 5-12 14 2-5 12z M12 2v20',
  axis: 'M12 2v4 M12 9v6 M12 18v4 M5 18 19 6 M4 15l1 3 3 1 M16 5l3 1 1 3',
  point: 'M12 3v6 M12 15v6 M3 12h6 M15 12h6 M13 12a1 1 0 1 1-2 0 1 1 0 0 1 2 0',
  coordinates: 'M6 4v14h14 M3 7l3-3 3 3 M17 15l3 3-3 3 M6 18l9-10 M12 8h3v3',
  curve: 'M3 20C18 20 2 3 12 3s0 17 9 17',
  move: 'M12 2v20 M2 12h20 M9 5l3-3 3 3 M9 19l3 3 3-3 M5 9l-3 3 3 3 M19 9l3 3-3 3',
  sketch: 'm4 16 12-12 4 4L8 20H4z M14 6l4 4 M3 22h18',
  exit: 'm3 12 6 6L21 4 M17 14v7H3V3h10',
  dimension: 'M3 3v18 M21 3v18 M3 12h18 M7 8l-4 4 4 4 M17 8l4 4-4 4',
  line: 'M4 20 20 4 M2 18h4v4H2z M18 2h4v4h-4z',
  centerline: 'm3 21 4-4 m3-3 4-4 m3-3 4-4 M2 2h3 M2 2v3',
  rectangle: 'M3 5h18v14H3z M1 3h4v4H1z',
  centerRectangle: 'M3 5h18v14H3z M9 12h6 M12 9v6',
  circle: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0 M10 12h4 M12 10v4',
  arc: 'M3 18a9 9 0 1 1 18 0 M12 12v6h6',
  arc3: 'M3 18a9 9 0 1 1 18 0 M2 16h3v3H2z M19 16h3v3h-3z M10 3h3v3h-3z',
  tangent: 'M3 21V11a8 8 0 0 1 16-1 M1 11h5 M19 8v4',
  slot: 'M8 5h8a7 7 0 0 1 0 14H8A7 7 0 0 1 8 5z M6 12h12',
  polygon: 'm12 2 10 6v9l-10 5-10-5V8z',
  trim: 'm4 3 15 18 M4 21 19 3 M3 7h18 M3 17h18',
  extend: 'M3 3v18 M21 3v18 M8 12h13 M17 8l4 4-4 4',
  convert: 'M3 4h12v12H3z M9 9h12v12H9 M5 12l4 4-4 4',
  measure: 'm2 15 16-13 5 6L7 22z M7 11l3 3 M11 8l3 3 M15 5l3 3',
  mass: 'M12 3v18 M5 21h14 M3 7h18 M5 7l-4 8h8z M19 7l-4 8h8z',
  material: 'm12 2 10 10-10 10L2 12z M7 7l10 10 M5 11l8 8 M11 5l8 8',
  units: 'M3 6v12 M3 12h18 M21 6v12 M7 9v3 M11 6v6 M15 9v3 M19 9v3',
  export: 'M4 12v9h16v-9 M12 16V2 M7 7l5-5 5 5',
  fit: 'M3 9V3h6 M15 3h6v6 M21 15v6h-6 M9 21H3v-6 M8 8h8v8H8z',
  zoom: 'M16 9a7 7 0 1 1-14 0 7 7 0 0 1 14 0 M14 14l8 8',
  orientation: 'm3 7 9-5 9 5v10l-9 5-9-5z M3 7l9 5 9-5 M12 12v10',
  normal: 'M3 19h18 M12 19V3 M7 8l5-5 5 5 M12 15h4v4',
  section: 'M3 3h18v18H3z M3 21 21 3 M7 21 21 7 M12 21l9-9',
  visibility: 'M2 12c5-10 15-10 20 0-5 10-15 10-20 0 M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
  appearance: 'M21 12a9 9 0 1 0-9 9c6 0-2-6 3-6h3z M7 7h1 M12 5h1 M17 8h1 M5 12h1',
  scene: 'M3 3h18v18H3z M3 17l6-7 4 5 3-3 5 6 M15 7h2',
  settings: 'M3 4h18v13H3z M8 21h8 M12 17v4',
} as const

export type IconId = keyof typeof paths

export function CommandIcon({ id }: { id: IconId }) {
  return (
    <svg
      className="toolbar-icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[id]} />
    </svg>
  )
}
