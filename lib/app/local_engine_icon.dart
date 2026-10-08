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
      'exllama' => Text(
        'EX',
        style: TextStyle(
          color: color,
          fontSize: size * 0.62,
          fontWeight: FontWeight.w700,
          height: 1,
          letterSpacing: -0.3,
        ),
      ),
      _ => Icon(LucideIcons.microchip, size: size, color: color),
    };

    return SizedBox(
      width: size,
      height: size,
      child: Center(child: ExcludeSemantics(child: icon)),
    );
  }
}
