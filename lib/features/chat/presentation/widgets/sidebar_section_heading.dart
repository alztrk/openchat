import 'package:flutter/material.dart';

class SidebarSectionHeading extends StatelessWidget {
  const SidebarSectionHeading({required this.title, super.key});

  final String title;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 18,
      child: Text(
        title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
          fontSize: 12,
          fontWeight: FontWeight.w500,
          height: 18 / 12,
        ),
      ),
    );
  }
}
