import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

class OpenChatBrandMark extends StatelessWidget {
  const OpenChatBrandMark({required this.size, super.key});

  final double size;

  static const _sourceSize = 58.8;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final layerRoot = dark ? 'assets/brand/dark' : 'assets/brand/light';

    return SizedBox(
      width: size,
      height: size,
      child: FittedBox(
        fit: BoxFit.contain,
        child: ClipRect(
          child: SizedBox(
            width: _sourceSize,
            height: _sourceSize,
            child: Stack(
              children: [
                Positioned(
                  left: -11.76,
                  top: -11.76,
                  width: 81.87,
                  height: 81.87,
                  child: SvgPicture.asset('$layerRoot/base.svg'),
                ),
                const Positioned(
                  left: 5.08,
                  top: 8.74,
                  width: 22.67,
                  height: 42.91,
                  child: _BrandLayer(fileName: 'left.svg'),
                ),
                const Positioned(
                  left: 31.02,
                  top: 8.74,
                  width: 22.67,
                  height: 42.91,
                  child: _BrandLayer(fileName: 'right.svg'),
                ),
                const Positioned(
                  left: 22.95,
                  top: 3.98,
                  width: 12.86,
                  height: 36.6,
                  child: _BrandLayer(fileName: 'copper-core.svg'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _BrandLayer extends StatelessWidget {
  const _BrandLayer({required this.fileName});

  final String fileName;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final layerRoot = dark ? 'assets/brand/dark' : 'assets/brand/light';
    return SvgPicture.asset(
      '$layerRoot/$fileName',
      fit: BoxFit.fill,
      excludeFromSemantics: true,
    );
  }
}
