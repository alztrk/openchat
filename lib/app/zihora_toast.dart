import 'package:flutter/material.dart';
import 'package:toastification/toastification.dart';

enum ZihoraToastType { info, success, warning, error }

void showZihoraToast(
  BuildContext context,
  String message, {
  ZihoraToastType type = ZihoraToastType.info,
}) {
  final toastType = switch (type) {
    ZihoraToastType.info => ToastificationType.info,
    ZihoraToastType.success => ToastificationType.success,
    ZihoraToastType.warning => ToastificationType.warning,
    ZihoraToastType.error => ToastificationType.error,
  };

  toastification.show(
    context: context,
    type: toastType,
    style: ToastificationStyle.flatColored,
    alignment: Alignment.topCenter,
    autoCloseDuration: const Duration(seconds: 4),
    title: Text(message),
    showProgressBar: false,
    closeOnClick: true,
    pauseOnHover: true,
  );
}
