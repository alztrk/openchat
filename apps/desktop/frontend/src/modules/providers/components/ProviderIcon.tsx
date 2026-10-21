import chatGptIconUrl from '@assets/icons/chatgpt.svg?no-inline';
import openCodeIconUrl from '@assets/icons/opencode.svg?no-inline';
import openRouterIconUrl from '@assets/icons/openrouter.svg?no-inline';
import anthropicIconUrl from '@assets/icons/providers/anthropic.svg?no-inline';
import cerebrasIconUrl from '@assets/icons/providers/cerebras.svg?no-inline';
import cohereIconUrl from '@assets/icons/providers/cohere.svg?no-inline';
import dashScopeIconUrl from '@assets/icons/providers/dashscope.svg?no-inline';
import deepInfraIconUrl from '@assets/icons/providers/deepinfra.svg?no-inline';
import deepSeekIconUrl from '@assets/icons/providers/deepseek.svg?no-inline';
import fireworksIconUrl from '@assets/icons/providers/fireworks.svg?no-inline';
import geminiIconUrl from '@assets/icons/providers/gemini.svg?no-inline';
import groqIconUrl from '@assets/icons/providers/groq.svg?no-inline';
import kiloIconUrl from '@assets/icons/providers/kilo.svg?no-inline';
import mistralIconUrl from '@assets/icons/providers/mistral.svg?no-inline';
import perplexityIconUrl from '@assets/icons/providers/perplexity.svg?no-inline';
import sambaNovaIconUrl from '@assets/icons/providers/sambanova.svg?no-inline';
import togetherIconUrl from '@assets/icons/providers/together.svg?no-inline';
import xAiIconUrl from '@assets/icons/providers/xai.svg?no-inline';
import { Github, Network, Star } from 'lucide-react';
import { LocalEngineIcon } from './EngineIcon';

export const providerIconUrls: Record<string, string> = {
  chatgpt: chatGptIconUrl,
  chatgpt_api: chatGptIconUrl,
  opencode: openCodeIconUrl,
  gemini: geminiIconUrl,
  groq: groqIconUrl,
  cerebras: cerebrasIconUrl,
  openrouter: openRouterIconUrl,
  mistral: mistralIconUrl,
  deepseek: deepSeekIconUrl,
  xai: xAiIconUrl,
  perplexity: perplexityIconUrl,
  together: togetherIconUrl,
  sambanova: sambaNovaIconUrl,
  anthropic: anthropicIconUrl,
  fireworks: fireworksIconUrl,
  deepinfra: deepInfraIconUrl,
  dashscope: dashScopeIconUrl,
  cohere: cohereIconUrl,
  kilo: kiloIconUrl,
};

export interface ProviderIconProps {
  providerId: string;
  className?: string;
  size?: number;
  tone?: 'secondary' | 'accent';
}

export function ProviderIcon({ className, providerId, size = 24, tone = 'secondary' }: ProviderIconProps) {
  if (providerId === 'llama_cpp') {
    return <LocalEngineIcon className={className} engineId={providerId} size={size} tone={tone} />;
  }
  if (providerId === 'favorites') {
    return (
      <Star
        aria-hidden="true"
        className={`shrink-0 ${tone === 'accent' ? 'text-accent-icon' : 'text-secondary-icon'}${className ? ` ${className}` : ''}`}
        size={size}
      />
    );
  }
  if (providerId === 'copilot') {
    return (
      <Github
        aria-hidden="true"
        className={`shrink-0 ${tone === 'accent' ? 'text-accent-icon' : 'text-secondary-icon'}${className ? ` ${className}` : ''}`}
        size={size}
      />
    );
  }
  const iconUrl = providerIconUrls[providerId];
  if (iconUrl) {
    if (providerId === 'chatgpt' || providerId === 'chatgpt_api') {
      return (
        <span
          aria-hidden="true"
          className={`provider-icon-mask ${tone === 'accent' ? 'bg-accent-icon' : 'bg-secondary-icon'}${className ? ` ${className}` : ''}`}
          style={{
            height: size,
            maskImage: `url("${iconUrl}")`,
            width: size,
            WebkitMaskImage: `url("${iconUrl}")`,
          }}
        />
      );
    }
    return (
      <img
        alt=""
        aria-hidden="true"
        className={`shrink-0 object-contain${className ? ` ${className}` : ''}`}
        height={size}
        src={iconUrl}
        width={size}
      />
    );
  }
  return (
    <Network
      aria-hidden="true"
      className={`shrink-0 ${tone === 'accent' ? 'text-accent-icon' : 'text-secondary-icon'}${className ? ` ${className}` : ''}`}
      size={size}
    />
  );
}
