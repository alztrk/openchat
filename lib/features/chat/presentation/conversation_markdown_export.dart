import 'dart:convert';

import '../../../l10n/generated/app_localizations.dart';
import '../domain/chat_conversation.dart';
import '../domain/chat_message.dart' as chat;

String buildConversationMarkdown(
  ChatConversation conversation,
  List<chat.ChatMessage> messages,
  AppLocalizations l10n,
) {
  final providerLabel = conversation.providerId == 'opencode'
      ? l10n.openCodeProvider
      : l10n.chatGptProvider;
  final modelLabel = conversation.modelId ?? l10n.messageModelUnavailable;
  final markdown = StringBuffer()
    ..writeln('# ${conversation.title}')
    ..writeln()
    ..writeln('**${l10n.conversationExportProvider}:** $providerLabel')
    ..writeln('**${l10n.conversationExportModel}:** $modelLabel')
    ..writeln(
      '**${l10n.conversationExportCreated}:** ${conversation.createdAt.toUtc().toIso8601String()}',
    )
    ..writeln()
    ..writeln('---')
    ..writeln();

  for (final message in messages) {
    final speaker = message.role == chat.ChatMessageRole.user
        ? l10n.userMessage
        : modelLabel;
    final timestamp = message.createdAt?.toUtc().toIso8601String();
    markdown
      ..writeln('## $speaker${timestamp == null ? '' : ' · $timestamp'}')
      ..writeln();
    if (message.status != chat.ChatMessageStatus.completed) {
      final status = switch (message.status) {
        chat.ChatMessageStatus.streaming => l10n.responseInProgress,
        chat.ChatMessageStatus.failed => l10n.responseFailed,
        chat.ChatMessageStatus.stopped => l10n.responseStopped,
        chat.ChatMessageStatus.completed => '',
      };
      markdown
        ..writeln('> **${l10n.conversationExportStatus}:** $status')
        ..writeln();
    }
    for (final summary in message.reasoningSummaries) {
      if (summary.content.trim().isEmpty) continue;
      final elapsed = summary.elapsed;
      final duration = elapsed == null
          ? ''
          : ' · ${l10n.secondsShort(elapsed.inSeconds)}';
      markdown
        ..writeln('### ${l10n.reasoningSummary}$duration')
        ..writeln()
        ..writeln(summary.content)
        ..writeln();
    }
    for (final activity in message.toolActivities) {
      final toolData = <String, Object?>{
        'name': activity.name,
        'status': switch (activity.status) {
          chat.ChatToolActivityStatus.awaitingApproval =>
            l10n.toolAwaitingApproval,
          chat.ChatToolActivityStatus.running => l10n.toolRunning,
          chat.ChatToolActivityStatus.completed => l10n.toolCompleted,
          chat.ChatToolActivityStatus.failed => l10n.toolFailed,
          chat.ChatToolActivityStatus.denied => l10n.toolDenied,
          chat.ChatToolActivityStatus.cancelled => l10n.toolCancelled,
        },
        'arguments': activity.arguments,
        if (activity.targetPath != null) 'targetPath': activity.targetPath,
        if (activity.output != null) 'output': activity.output,
      };
      final formattedToolData = const JsonEncoder.withIndent('  ')
          .convert(toolData);
      final codeFence = _markdownCodeFence(formattedToolData);
      markdown
        ..writeln('### ${l10n.conversationExportToolActivity}')
        ..writeln()
        ..writeln('${codeFence}json')
        ..writeln(formattedToolData)
        ..writeln(codeFence)
        ..writeln();
    }
    markdown
      ..writeln(message.content)
      ..writeln();
  }
  return markdown.toString();
}

String conversationExportFileName(String title) {
  final cleaned = title
      .trim()
      .replaceAll(RegExp(r'[<>:"/\\|?*\x00-\x1f]'), '_')
      .replaceAll(RegExp(r'\s+'), '_')
      .replaceAll(RegExp(r'[. ]+$'), '');
  if (cleaned.isEmpty) return 'conversation.md';
  final baseName = cleaned.length > 80 ? cleaned.substring(0, 80) : cleaned;
  return '$baseName.md';
}

String _markdownCodeFence(String content) {
  var longestBacktickRun = 2;
  for (final match in RegExp(r'`+').allMatches(content)) {
    final runLength = match.group(0)?.length;
    if (runLength != null && runLength > longestBacktickRun) {
      longestBacktickRun = runLength;
    }
  }
  return '`' * (longestBacktickRun + 1);
}
