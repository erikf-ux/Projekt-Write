import { create } from 'zustand';
import { runSystemCheck } from '@/services/system';
import type { CommandError } from '@/types/generated/CommandError';
import type { SystemCheckResponse } from '@/types/generated/SystemCheckResponse';

export type SystemStatus =
  | { phase: 'running' }
  | { phase: 'done'; response: SystemCheckResponse }
  | { phase: 'error'; error: CommandError };

interface SystemStatusState {
  status: SystemStatus;
  run: () => Promise<void>;
}

export const useSystemStatus = create<SystemStatusState>((set, get) => ({
  status: { phase: 'running' },
  run: async () => {
    set({ status: { phase: 'running' } });
    const result = await runSystemCheck();
    // A newer run may have started meanwhile; the last call wins either way.
    if (get().status.phase === 'running') {
      set({
        status: result.ok ? { phase: 'done', response: result.value } : { phase: 'error', error: result.error },
      });
    }
  },
}));
