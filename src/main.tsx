import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { createAppCommands } from './app/commands';
import { createFormatCommands } from './app/formatCommands';
import { registerBuiltins } from './extensions/builtin';
import { sealRegistries } from './extensions/registries';
import { registerCommands } from './modules/commands/registry';
import { applyThemeSetting } from './modules/settings/uiStore';
import './index.css';

// The built-in extensions, then nothing more: after `sealRegistries()` no
// code can register anything (ADR 0027, decisions 2 and 3).
registerBuiltins();
sealRegistries();
// The commands are made from the registries (the exporters), so after them.
registerCommands([...createAppCommands(), ...createFormatCommands()]);

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
