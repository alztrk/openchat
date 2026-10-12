import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/local_engine_icon.dart';

class ProviderIcon extends StatelessWidget {
  const ProviderIcon({
    required this.providerId,
    required this.color,
    required this.size,
    super.key,
  });

  final String providerId;
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) {
    final iconWidth = providerId == 'exllama' ? size * 2 : size;
    final icon = switch (providerId) {
      'chatgpt' => SvgPicture.asset(
        'assets/icons/chatgpt.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
      'opencode' => SvgPicture.asset(
        'assets/icons/opencode.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
      'gemini' => _ProviderMonogram(label: 'Ge', color: color, size: size),
      'groq' => _ProviderMonogram(label: 'Gr', color: color, size: size),
      'cerebras' => _ProviderMonogram(label: 'Ce', color: color, size: size),
      'openrouter' => SvgPicture.asset(
        'assets/icons/openrouter.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
      'mistral' => Image.asset(
        'assets/icons/mistral.png',
        width: size,
        height: size,
        fit: BoxFit.contain,
        filterQuality: FilterQuality.medium,
        excludeFromSemantics: true,
      ),
      'deepseek' => _ProviderMonogram(label: 'Ds', color: color, size: size),
      'xai' => _ProviderMonogram(label: 'xA', color: color, size: size),
      'perplexity' => _ProviderMonogram(label: 'Px', color: color, size: size),
      'together' => _ProviderMonogram(label: 'To', color: color, size: size),
      'sambanova' => _ProviderMonogram(label: 'Sn', color: color, size: size),
      'anthropic' => _ProviderMonogram(label: 'An', color: color, size: size),
      'fireworks' => _ProviderMonogram(label: 'Fw', color: color, size: size),
      'deepinfra' => _ProviderMonogram(label: 'Di', color: color, size: size),
      'dashscope' => _ProviderMonogram(label: 'Qw', color: color, size: size),
      'cohere' => _ProviderMonogram(label: 'Co', color: color, size: size),
      'ollama' => _ProviderMonogram(label: 'Ol', color: color, size: size),
      'lmstudio' => _ProviderMonogram(label: 'Lm', color: color, size: size),
      'llama_cpp' || 'vllm' || 'exllama' => LocalEngineIcon(
        engineId: providerId,
        color: color,
        size: size,
      ),
      'favorites' => Icon(LucideIcons.star, size: size, color: color),
      _ => Icon(LucideIcons.network, size: size, color: color),
    };

    return SizedBox(
      width: iconWidth,
      height: size,
      child: Center(child: ExcludeSemantics(child: icon)),
    );
  }
}

class _ProviderMonogram extends StatelessWidget {
  const _ProviderMonogram({
    required this.label,
    required this.color,
    required this.size,
  });

  final String label;
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: size,
      child: Center(
        child: Text(
          label,
          maxLines: 1,
          style: TextStyle(
            color: color,
            fontSize: size * 0.62,
            fontWeight: FontWeight.w700,
            height: 1,
            letterSpacing: -0.3,
          ),
        ),
      ),
    );
  }
}
