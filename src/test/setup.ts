import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  cleanup();
});

// jsdom has no matchMedia; the theme module needs it.
if (typeof window.matchMedia !== 'function') {
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: (query: string): MediaQueryList =>
      ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        dispatchEvent: () => false,
      }) as unknown as MediaQueryList,
  });
}

// jsdom has no layout: scrolling into view is a no-op there.
Element.prototype.scrollIntoView = function scrollIntoView() {
  // Nothing to scroll in jsdom.
};

// jsdom has no ClipboardEvent; ProseMirror's pasteText and pasteHTML create one.
if (typeof globalThis.ClipboardEvent !== 'function') {
  class ClipboardEventStub extends Event {
    readonly clipboardData: DataTransfer | null = null;
  }
  Object.defineProperty(globalThis, 'ClipboardEvent', { configurable: true, value: ClipboardEventStub });
}
