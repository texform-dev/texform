import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { applyTheme, getInitialTheme } from './lib/theme'
import App from './App'
import './styles/index.css'

applyTheme(getInitialTheme())

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
