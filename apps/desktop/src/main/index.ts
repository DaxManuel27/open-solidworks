import { join } from 'node:path'
import {
  BrowserWindow,
  Menu,
  app,
  dialog,
  ipcMain,
  type MenuItemConstructorOptions,
} from 'electron'
import { CoreError, CoreProcess, findCore } from './core'

let win: BrowserWindow | null = null
let core: CoreProcess | null = null
let dirty = false
let closing = false

const PART_FILTER = { name: 'OpenForge Part', extensions: ['ofp'] }

function startCore(): void {
  const path = findCore(app.getAppPath(), process.resourcesPath)
  core = new CoreProcess(path)
  core.onExit = (code) => {
    if (closing) return
    win?.webContents.send('core:crashed', code)
    // Restart so the user can keep working (unsaved changes are lost).
    setTimeout(startCore, 500)
  }
  core.start()
}

function send(command: string): void {
  win?.webContents.send('menu', command)
}

function buildMenu(): void {
  const mac = process.platform === 'darwin'
  const item = (
    label: string,
    command: string,
    accelerator?: string,
  ): MenuItemConstructorOptions => ({
    label,
    accelerator,
    click: () => send(command),
  })
  const template: MenuItemConstructorOptions[] = [
    ...(mac ? [{ role: 'appMenu' as const }] : []),
    {
      label: 'File',
      submenu: [
        item('New', 'new', 'CmdOrCtrl+N'),
        item('Open…', 'open', 'CmdOrCtrl+O'),
        item('Save', 'save', 'CmdOrCtrl+S'),
        item('Save As…', 'save-as', 'CmdOrCtrl+Shift+S'),
        { type: 'separator' },
        item('Import STEP/IGES…', 'import'),
        item('Export…', 'export'),
        { type: 'separator' },
        mac ? { role: 'close' } : { role: 'quit' },
      ],
    },
    {
      label: 'Edit',
      submenu: [
        item('Undo', 'undo', 'CmdOrCtrl+Z'),
        item('Redo', 'redo', mac ? 'Cmd+Shift+Z' : 'Ctrl+Y'),
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'selectAll' },
      ],
    },
    {
      label: 'View',
      submenu: [
        // Bare-key accelerators would swallow typing, so F is handled in the renderer.
        item('Zoom to Fit (F)', 'zoom-fit'),
        item('Front', 'view-front', 'CmdOrCtrl+1'),
        item('Back', 'view-back', 'CmdOrCtrl+2'),
        item('Left', 'view-left', 'CmdOrCtrl+3'),
        item('Right', 'view-right', 'CmdOrCtrl+4'),
        item('Top', 'view-top', 'CmdOrCtrl+5'),
        item('Bottom', 'view-bottom', 'CmdOrCtrl+6'),
        item('Isometric', 'view-iso', 'CmdOrCtrl+7'),
        item('Normal To', 'view-normal', 'CmdOrCtrl+8'),
        { type: 'separator' },
        { role: 'toggleDevTools' },
        { role: 'togglefullscreen' },
      ],
    },
    {
      label: 'Tools',
      submenu: [
        item('Units…', 'units'),
        item('Material…', 'material'),
        item('Mass Properties', 'mass'),
      ],
    },
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

function createWindow(): void {
  win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 600,
    title: 'OpenForge',
    backgroundColor: '#e7e9ec',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  win.on('close', (event) => {
    if (!dirty || closing) return
    event.preventDefault()
    const choice = dialog.showMessageBoxSync(win!, {
      type: 'question',
      buttons: ['Save', "Don't Save", 'Cancel'],
      defaultId: 0,
      cancelId: 2,
      message: 'Save changes to this part before closing?',
    })
    if (choice === 0) send('save-and-close')
    if (choice === 1) {
      closing = true
      win?.close()
    }
  })

  const devUrl = process.env['ELECTRON_RENDERER_URL']
  if (!app.isPackaged && devUrl) {
    void win.loadURL(devUrl)
  } else {
    void win.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

ipcMain.handle('core:call', async (_e, method: string, params: unknown) => {
  if (!core) throw new Error('The core is not running.')
  try {
    return { ok: true, value: await core.call(method, params) }
  } catch (e) {
    // Errors cross the IPC boundary as values so the message survives intact.
    const err = e as CoreError
    return { ok: false, error: err.message, code: err.code ?? -1 }
  }
})

ipcMain.handle('dialog:open', async (_e, kind: 'part' | 'import') => {
  const filters =
    kind === 'part'
      ? [PART_FILTER]
      : [{ name: 'STEP or IGES', extensions: ['step', 'stp', 'iges', 'igs'] }]
  const r = await dialog.showOpenDialog(win!, { properties: ['openFile'], filters })
  return r.canceled ? null : (r.filePaths[0] ?? null)
})

ipcMain.handle('dialog:save', async (_e, kind: 'part' | 'export', defaultName: string) => {
  const filters =
    kind === 'part'
      ? [PART_FILTER]
      : [
          { name: 'STEP', extensions: ['step'] },
          { name: 'STL', extensions: ['stl'] },
          { name: 'IGES', extensions: ['iges'] },
        ]
  const r = await dialog.showSaveDialog(win!, { defaultPath: defaultName, filters })
  return r.canceled ? null : (r.filePath ?? null)
})

ipcMain.on('doc:state', (_e, title: string, isDirty: boolean) => {
  dirty = isDirty
  win?.setTitle(title)
  win?.setDocumentEdited(isDirty)
})

ipcMain.on('window:close', () => {
  closing = true
  win?.close()
})

void app.whenReady().then(() => {
  try {
    startCore()
  } catch (e) {
    dialog.showErrorBox('OpenForge', (e as Error).message)
    app.quit()
    return
  }
  buildMenu()
  createWindow()
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('before-quit', () => {
  closing = true
  core?.stop()
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
