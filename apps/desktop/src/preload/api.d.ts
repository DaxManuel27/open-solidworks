export interface OpenForgeApi {
  platform: string
  versions: { electron: string; chrome: string }
}

declare global {
  interface Window {
    openforge: OpenForgeApi
  }
}
