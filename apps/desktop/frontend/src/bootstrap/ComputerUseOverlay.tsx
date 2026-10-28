import { useEffect } from 'react';
import './computerUseOverlay.css';

export function ComputerUseOverlay() {
  useEffect(() => {
    const [red = '142', green = '86', blue = '59'] =
      new URLSearchParams(location.search).get('accent')?.split(',') ?? [];
    const channels = [red, green, blue].map(Number);
    if (channels.every((channel) => Number.isInteger(channel) && channel >= 0 && channel <= 255)) {
      document.documentElement.style.setProperty('--computer-use-accent', `rgb(${channels.join(' ')})`);
    }
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
  }, []);

  const showBanner = new URLSearchParams(location.search).get('banner') === 'true';
  return (
    <main aria-live="polite" className="computer-use-overlay">
      {showBanner && (
        <div className="computer-use-overlay__banner">OpenChat is using your computer. Press Esc 2 times to stop</div>
      )}
      {(['top-left', 'top-right', 'bottom-left', 'bottom-right'] as const).map((corner) => (
        <span
          aria-hidden="true"
          className={`computer-use-overlay__corner computer-use-overlay__corner--${corner}`}
          key={corner}
        />
      ))}
    </main>
  );
}
