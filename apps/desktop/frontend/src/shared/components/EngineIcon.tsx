import { Microchip } from 'lucide-react';

const llamaCppIconUrl = new URL('./assets/llama-cpp.svg', import.meta.url).href;
const sglangIconUrl = new URL('./assets/sglang.svg', import.meta.url).href;
const vllmIconUrl = new URL('./assets/vllm.svg', import.meta.url).href;

export interface LocalEngineIconProps {
  engineId: string;
  size?: number;
  tone?: 'secondary' | 'accent';
  className?: string;
}

export function LocalEngineIcon({ className, engineId, size = 22, tone = 'secondary' }: LocalEngineIconProps) {
  if (engineId === 'llama_cpp' || engineId === 'vllm') {
    const iconUrl = engineId === 'llama_cpp' ? llamaCppIconUrl : vllmIconUrl;
    const iconColor = tone === 'accent' ? 'bg-accent' : 'bg-secondary-icon';
    return (
      <span
        aria-hidden="true"
        className={`inline-block shrink-0 ${iconColor}${className ? ` ${className}` : ''}`}
        style={{
          height: size,
          maskImage: `url("${iconUrl}")`,
          maskPosition: 'center',
          maskRepeat: 'no-repeat',
          maskSize: 'contain',
          WebkitMaskImage: `url("${iconUrl}")`,
          WebkitMaskPosition: 'center',
          WebkitMaskRepeat: 'no-repeat',
          WebkitMaskSize: 'contain',
          width: size,
        }}
      />
    );
  }

  if (engineId === 'sglang') {
    return (
      <img
        alt=""
        aria-hidden="true"
        className={`shrink-0 object-contain${className ? ` ${className}` : ''}`}
        height={size}
        src={sglangIconUrl}
        width={size}
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
