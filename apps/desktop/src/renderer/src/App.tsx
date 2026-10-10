export function App() {
  const { platform, versions } = window.openforge

  return (
    <main className="welcome">
      <h1>OpenForge</h1>
      <p>Free, open-source parametric CAD.</p>
      <p className="meta">
        Electron {versions.electron} · Chromium {versions.chrome} · {platform}
      </p>
    </main>
  )
}
