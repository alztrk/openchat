import 'package:openchat/features/settings/domain/chat_gpt_usage_snapshot.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

String chatGptUsageBucketLabel(
  ChatGptUsageBucket bucket,
  AppLocalizations l10n,
) {
  final windowSeconds = bucket.windowSeconds;
  if (windowSeconds == null) return bucket.limitId;

  return switch (windowSeconds) {
    18000 => l10n.usageFiveHour,
    604800 => l10n.usageWeekly,
    >= 2419200 && <= 2678400 => l10n.usageMonthly,
    _ => bucket.limitId,
  };
}
