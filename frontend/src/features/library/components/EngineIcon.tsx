import { Microchip } from 'lucide-react';

const llamaCppIconUrl = new URL(
  '@/features/library/assets/llama-cpp.svg',
  import.meta.url,
).href;

export interface LocalEngineIconProps {
  engineId: string;
  size?: number;
  tone?: 'secondary' | 'accent';
  className?: string;
}

export function LocalEngineIcon({
  className,
  engineId,
  size = 22,
  tone = 'secondary',
}: LocalEngineIconProps) {
  if (engineId === 'llama_cpp') {
    const iconColor = tone === 'accent' ? 'bg-accent' : 'bg-secondary-icon';
    return (
      <span
        aria-hidden="true"
        className={`inline-block shrink-0 ${iconColor}${className ? ` ${className}` : ''}`}
        style={{
          height: size,
          maskImage: `url("${llamaCppIconUrl}")`,
          maskPosition: 'center',
          maskRepeat: 'no-repeat',
          maskSize: 'contain',
          WebkitMaskImage: `url("${llamaCppIconUrl}")`,
          WebkitMaskPosition: 'center',
          WebkitMaskRepeat: 'no-repeat',
          WebkitMaskSize: 'contain',
          width: size,
        }}
      />
    );
  }

  return (
    <Microchip
      aria-hidden="true"
      className={`shrink-0 ${tone === 'accent' ? 'text-accent-icon' : 'text-secondary-icon'}${className ? ` ${className}` : ''}`}
      size={size}
    />
  );
}
