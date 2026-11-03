/**
 * State of the library panel: the library of this instance, a pending
 * confirmation and a running move (Phase 2, section 9).
 *
 * Every folder is chosen in Rust; the store only keeps the tokens Rust
 * returns. Locations in sync folders or on network drives need a
 * confirmation before a library is created or moved there.
 */
import { create } from 'zustand';
import { libraryService } from '@/services/library';
import type { CommandError } from '@/types/generated/CommandError';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { LocationChoice } from '@/types/generated/LocationChoice';
import type { MoveProgress } from '@/types/generated/MoveProgress';

export type Prompt =
  /** Sync folder or network drive: create/move there anyway? */
  | { kind: 'confirmLocation'; action: 'create' | 'move'; choice: LocationChoice }
  /** The chosen folder already holds a library: open it instead? */
  | { kind: 'existingLibrary'; choice: LocationChoice }
  /** The library was moved elsewhere: open this old copy anyway? */
  | { kind: 'movedLibrary'; choice: LocationChoice; movedTo: string };

/** Shown after a completed move. */
export interface MoveNotice {
  oldPath: string;
}

/** Translated dialog titles, provided by the component. */
export interface DialogTitles {
  create: string;
  open: string;
  move: string;
}

interface LibraryState {
  /** `null` while the first status is loading. */
  status: LibraryStatus | null;
  defaultLocation: LocationChoice | null;
  busy: boolean;
  error: CommandError | null;
  prompt: Prompt | null;
  progress: MoveProgress | null;
  moveNotice: MoveNotice | null;
  load: () => Promise<void>;
  createAtDefault: () => Promise<void>;
  chooseAndCreate: (title: string) => Promise<void>;
  chooseAndOpen: (title: string) => Promise<void>;
  chooseAndMove: (title: string) => Promise<void>;
  retry: () => Promise<void>;
  acceptLocation: () => Promise<void>;
  cancelMove: () => Promise<void>;
  confirmPrompt: () => Promise<void>;
  dismissPrompt: () => void;
}

const FOLDER_NOT_EMPTY: CommandError = { code: 'FOLDER_NOT_EMPTY', messageKey: 'error.folderNotEmpty' };
const FOLDER_IS_LIBRARY: CommandError = {
  code: 'FOLDER_IS_LIBRARY',
  messageKey: 'error.folderIsLibrary',
};

function needsWarning(choice: LocationChoice): boolean {
  return choice.location.kind !== 'local';
}

export const useLibrary = create<LibraryState>((set, get) => {
  /** Runs an action with the busy flag; stores the resulting status or error. */
  async function perform(
    action: () => Promise<{ ok: true; value: LibraryStatus } | { ok: false; error: CommandError }>,
  ): Promise<boolean> {
    set({ busy: true, error: null });
    const result = await action();
    if (result.ok) {
      set({ status: result.value, busy: false });
      return true;
    }
    set({ error: result.error, busy: false });
    return false;
  }

  async function create(choice: LocationChoice, acceptWarning: boolean): Promise<void> {
    if (choice.contents === 'library') {
      set({ prompt: { kind: 'existingLibrary', choice }, error: null });
      return;
    }
    if (choice.contents === 'other') {
      set({ error: FOLDER_NOT_EMPTY });
      return;
    }
    if (needsWarning(choice) && !acceptWarning) {
      set({ prompt: { kind: 'confirmLocation', action: 'create', choice }, error: null });
      return;
    }
    if (await perform(() => libraryService.create(choice.token, acceptWarning))) {
      await refreshDefault();
    }
  }

  async function open(choice: LocationChoice, acceptMoved: boolean): Promise<void> {
    set({ busy: true, error: null });
    const result = await libraryService.open(choice.token, acceptMoved);
    if (result.ok) {
      set({ status: result.value, busy: false });
      return;
    }
    const movedTo = result.error.details?.['path'];
    if (result.error.code === 'LIBRARY_MOVED' && movedTo !== undefined && !acceptMoved) {
      set({ busy: false, prompt: { kind: 'movedLibrary', choice, movedTo } });
      return;
    }
    set({ busy: false, error: result.error });
  }

  async function move(choice: LocationChoice, acceptWarning: boolean): Promise<void> {
    if (choice.contents === 'library') {
      set({ error: FOLDER_IS_LIBRARY });
      return;
    }
    if (choice.contents === 'other') {
      set({ error: FOLDER_NOT_EMPTY });
      return;
    }
    if (needsWarning(choice) && !acceptWarning) {
      set({ prompt: { kind: 'confirmLocation', action: 'move', choice }, error: null });
      return;
    }
    const before = get().status;
    const oldPath = before?.state === 'open' ? before.library.displayPath : '';
    set({ status: { state: 'moving' }, progress: null, busy: true, error: null, moveNotice: null });
    const result = await libraryService.move(choice.token, acceptWarning, (progress) => {
      set({ progress });
    });
    if (result.ok) {
      set({ status: result.value, busy: false, progress: null, moveNotice: { oldPath } });
      return;
    }
    // The old library normally stays open; ask Rust for the real state.
    const status = await libraryService.status();
    set({
      status: status.ok ? status.value : before,
      busy: false,
      progress: null,
      error: result.error,
    });
  }

  async function refreshDefault(): Promise<void> {
    const location = await libraryService.defaultLocation();
    set({ defaultLocation: location.ok ? location.value : null });
  }

  return {
    status: null,
    defaultLocation: null,
    busy: false,
    error: null,
    prompt: null,
    progress: null,
    moveNotice: null,

    load: async () => {
      const status = await libraryService.status();
      if (status.ok) {
        set({ status: status.value });
      } else {
        set({
          status: { state: 'closed', configuredPath: null, error: status.error, settingsNotice: null },
        });
      }
      await refreshDefault();
    },

    createAtDefault: async () => {
      const choice = get().defaultLocation;
      if (choice) {
        await create(choice, false);
      }
    },

    chooseAndCreate: async (title) => {
      const choice = await libraryService.chooseLocation('create', title);
      if (!choice.ok) {
        set({ error: choice.error });
      } else if (choice.value) {
        await create(choice.value, false);
      }
    },

    chooseAndOpen: async (title) => {
      const choice = await libraryService.chooseLocation('open', title);
      if (!choice.ok) {
        set({ error: choice.error });
      } else if (choice.value) {
        await open(choice.value, false);
      }
    },

    chooseAndMove: async (title) => {
      const choice = await libraryService.chooseLocation('move', title);
      if (!choice.ok) {
        set({ error: choice.error });
      } else if (choice.value) {
        await move(choice.value, false);
      }
    },

    retry: async () => {
      await perform(() => libraryService.reopen());
    },

    acceptLocation: async () => {
      await perform(() => libraryService.acceptLocation());
    },

    cancelMove: async () => {
      await libraryService.cancelMove();
    },

    confirmPrompt: async () => {
      const prompt = get().prompt;
      set({ prompt: null });
      if (!prompt) {
        return;
      }
      switch (prompt.kind) {
        case 'confirmLocation':
          await (prompt.action === 'create' ? create(prompt.choice, true) : move(prompt.choice, true));
          break;
        case 'existingLibrary':
          await open(prompt.choice, false);
          break;
        case 'movedLibrary':
          await open(prompt.choice, true);
          break;
      }
    },

    dismissPrompt: () => {
      set({ prompt: null });
    },
  };
});
