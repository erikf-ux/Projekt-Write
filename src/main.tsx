import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { followSystemTheme } from './modules/themes/applyTheme';
import './index.css';

// Apply the colour tokens before the first render to avoid a flash.
followSystemTheme();

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root element #root is missing');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
