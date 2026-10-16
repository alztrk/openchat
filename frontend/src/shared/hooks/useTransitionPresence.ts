import { useEffect, useState } from 'react';

const EXIT_DURATION_MS = 120;

export function useTransitionPresence(isOpen: boolean): boolean {
  const [isPresent, setIsPresent] = useState(isOpen);

  useEffect(() => {
    if (isOpen) {
      setIsPresent(true);
      return;
    }

    if (!isPresent) return;
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      setIsPresent(false);
      return;
    }

    const timeoutId = window.setTimeout(() => setIsPresent(false), EXIT_DURATION_MS);
    return () => window.clearTimeout(timeoutId);
  }, [isOpen, isPresent]);

  return isPresent;
}
