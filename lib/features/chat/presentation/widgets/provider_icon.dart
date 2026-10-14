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
    final iconWidth = size;
    final icon = switch (providerId) {
      'chatgpt' || 'chatgpt_api' => _providerSvg('assets/icons/chatgpt.svg'),
      'opencode' => _providerSvg('assets/icons/opencode.svg'),
      'gemini' => _providerSvg(
        'assets/icons/providers/gemini.svg',
        preserveBrandColors: true,
      ),
      'groq' => _providerSvg('assets/icons/providers/groq.svg'),
      'cerebras' => _providerSvg(
        'assets/icons/providers/cerebras.svg',
        preserveBrandColors: true,
      ),
      'openrouter' => _providerSvg('assets/icons/openrouter.svg'),
      'mistral' => _providerSvg(
        'assets/icons/providers/mistral.svg',
        preserveBrandColors: true,
      ),
      'deepseek' => _providerSvg(
        'assets/icons/providers/deepseek.svg',
        preserveBrandColors: true,
      ),
      'xai' => _providerSvg('assets/icons/providers/xai.svg'),
      'perplexity' => _providerSvg(
        'assets/icons/providers/perplexity.svg',
        preserveBrandColors: true,
      ),
      'together' => _providerSvg(
        'assets/icons/providers/together.svg',
        preserveBrandColors: true,
      ),
      'sambanova' => _providerSvg(
        'assets/icons/providers/sambanova.svg',
        preserveBrandColors: true,
      ),
      'anthropic' => _providerSvg('assets/icons/providers/anthropic.svg'),
      'fireworks' => _providerSvg(
        'assets/icons/providers/fireworks.svg',
        preserveBrandColors: true,
      ),
      'deepinfra' => _providerSvg(
        'assets/icons/providers/deepinfra.svg',
        preserveBrandColors: true,
      ),
      'dashscope' => _providerSvg(
        'assets/icons/providers/dashscope.svg',
        preserveBrandColors: true,
      ),
      'cohere' => _providerSvg(
        'assets/icons/providers/cohere.svg',
        preserveBrandColors: true,
      ),
      'llama_cpp' => LocalEngineIcon(
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

  Widget _providerSvg(String assetPath, {bool preserveBrandColors = false}) {
    return SvgPicture.asset(
      assetPath,
      width: size,
      height: size,
      colorFilter: preserveBrandColors
          ? null
          : ColorFilter.mode(color, BlendMode.srcIn),
      excludeFromSemantics: true,
    );
  }
}
