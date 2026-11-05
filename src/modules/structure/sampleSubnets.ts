/**
 * Beispiel-Subnetze beim ersten Start (section 11: "optional"; ADR 0017,
 * decision 15): the first start offers "Projekte", "Wissen" and "Ideen",
 * not chosen in advance. They are made once the new library is open and
 * still has no subnet.
 */
import { create } from 'zustand';
import { translate, useLocale } from '@/i18n';
import { useUi } from '@/modules/settings/uiStore';
import { useStructure } from './structureStore';

export const SAMPLE_SUBNET_KEYS = [
  'subnet.sample.projects',
  'subnet.sample.knowledge',
  'subnet.sample.ideas',
] as const;

export const useSampleSubnets = create<{ wanted: boolean }>(() => ({ wanted: false }));

/** Makes the sample subnets if they were chosen and the library has none. */
export async function createSampleSubnets(): Promise<void> {
  if (!useSampleSubnets.getState().wanted) {
    return;
  }
  useSampleSubnets.setState({ wanted: false });
  const store = useStructure.getState();
  if (store.subnets.length > 0) {
    return;
  }
  const locale = useLocale.getState().locale;
  for (const key of SAMPLE_SUBNET_KEYS) {
    await store.createSubnet({ name: translate(locale, key) });
  }
  // Starting points, not steps one would undo one by one: no notice.
  useUi.getState().showNotice(null);
}
