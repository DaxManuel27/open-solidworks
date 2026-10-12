import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { useStore } from './store'
import './styles.css'

// For the native smoke test and debugging from DevTools.
;(window as unknown as { __openforgeStore: typeof useStore }).__openforgeStore = useStore

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
