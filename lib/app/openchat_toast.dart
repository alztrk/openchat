import 'package:flutter/material.dart';
import 'package:toastification/toastification.dart';

enum OpenChatToastType { info, success, warning, error }

void showOpenChatToast(
  BuildContext context,
  String message, {
  OpenChatToastType type = OpenChatToastType.info,
}) {
  final toastType = switch (type) {
    OpenChatToastType.info => ToastificationType.info,
    OpenChatToastType.success => ToastificationType.success,
    OpenChatToastType.warning => ToastificationType.warning,
    OpenChatToastType.error => ToastificationType.error,
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
