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
      'gemini' => Icon(LucideIcons.sparkles, size: size, color: color),
      'groq' => Icon(LucideIcons.zap, size: size, color: color),
      'cerebras' => Icon(LucideIcons.microchip, size: size, color: color),
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
        filterQuality: FilterQuality.none,
        excludeFromSemantics: true,
      ),
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
