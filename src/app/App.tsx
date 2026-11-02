import { useEffect } from 'react';
import { useLocale } from '@/i18n';
import { StartScreen } from './StartScreen';

export function App() {
  const locale = useLocale((state) => state.locale);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  return <StartScreen />;
}
