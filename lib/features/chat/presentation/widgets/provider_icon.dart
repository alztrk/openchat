import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

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
      'gemini' => Icon(Icons.auto_awesome_rounded, size: size, color: color),
      'groq' => Icon(Icons.bolt_rounded, size: size, color: color),
      'cerebras' => Icon(Icons.memory_rounded, size: size, color: color),
      'openrouter' => SvgPicture.asset(
        'assets/icons/openrouter.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
      'favorites' => Icon(Icons.star_outline_rounded, size: size, color: color),
      _ => Icon(Icons.hub_outlined, size: size, color: color),
    };

    return SizedBox(
      width: size,
      height: size,
      child: Center(child: ExcludeSemantics(child: icon)),
    );
  }
}
