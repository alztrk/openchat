import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_svg/flutter_svg.dart';

class LocalEngineIcon extends StatelessWidget {
  const LocalEngineIcon({
    required this.engineId,
    required this.color,
    required this.size,
    super.key,
  });

  final String engineId;
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) {
    final isExllama = engineId == 'exllama';
    final iconWidth = isExllama ? size * 2 : size;
    final Widget icon = switch (engineId) {
      'llama_cpp' => SvgPicture.asset(
        'assets/icons/engines/llama-cpp.svg',
        width: size,
        height: size,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
      'vllm' => SvgPicture.asset(
        'assets/icons/engines/vllm.svg',
        width: size,
        height: size,
        excludeFromSemantics: true,
      ),
      'exllama' => Image.asset(
        'assets/icons/engines/exllama-v3.png',
        width: iconWidth,
        height: size,
        fit: BoxFit.contain,
        excludeFromSemantics: true,
      ),
      _ => Icon(LucideIcons.microchip, size: size, color: color),
    };

    return SizedBox(
      width: iconWidth,
      height: size,
      child: Center(child: ExcludeSemantics(child: icon)),
    );
  }
}
