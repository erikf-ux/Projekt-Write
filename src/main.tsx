import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { applyThemeSetting } from './modules/settings/uiStore';
import './index.css';

// Apply the colour tokens before the first render to avoid a flash; the
// saved theme follows as soon as the settings are loaded.
applyThemeSetting('system', []);

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root element #root is missing');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
