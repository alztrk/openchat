import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/agent_question.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class UserQuestionCard extends StatefulWidget {
  const UserQuestionCard({
    required this.group,
    required this.onSubmit,
    this.focusOnBuild = false,
    this.isResuming = false,
    super.key,
  });

  final AgentQuestionGroup group;
  final Future<String?> Function(List<AgentQuestionAnswer> answers) onSubmit;
  final bool focusOnBuild;
  final bool isResuming;

  @override
  State<UserQuestionCard> createState() => _UserQuestionCardState();
}

class _UserQuestionCardState extends State<UserQuestionCard> {
  final Map<String, String> _answers = <String, String>{};
  final Map<String, TextEditingController> _textControllers =
      <String, TextEditingController>{};
  bool _isSubmitting = false;
  String? _submitError;

  @override
  void initState() {
    super.initState();
    _syncTextControllers();
    _restoreSavedAnswers();
  }

  @override
  void didUpdateWidget(covariant UserQuestionCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.group.id != widget.group.id) {
      _answers.clear();
      _disposeTextControllers();
      _syncTextControllers();
      _restoreSavedAnswers();
      _isSubmitting = false;
    } else if (oldWidget.group.revision != widget.group.revision ||
        oldWidget.group.isAnswered != widget.group.isAnswered ||
        oldWidget.group.savedAnswers.length !=
            widget.group.savedAnswers.length) {
      _answers.clear();
      for (final controller in _textControllers.values) {
        controller.clear();
      }
      _restoreSavedAnswers();
      _submitError = null;
      _isSubmitting = false;
    }
  }

  void _restoreSavedAnswers() {
    for (final answer in widget.group.savedAnswers) {
      final value = answer.value;
      final answerText = switch (value) {
        AgentChoiceAnswer(:final optionId) => optionId,
        AgentTextAnswer(:final text) => text,
      };
      _answers[answer.questionId] = answerText;
      _textControllers[answer.questionId]?.text = answerText;
    }
  }

  void _syncTextControllers() {
    for (final question in widget.group.questions) {
      if (question.kind == AgentQuestionKind.text) {
        _textControllers[question.id] = TextEditingController();
      }
    }
  }

  void _disposeTextControllers() {
    for (final controller in _textControllers.values) {
      controller.dispose();
    }
    _textControllers.clear();
  }

  @override
  void dispose() {
    _disposeTextControllers();
    super.dispose();
  }

  List<AgentQuestionAnswer>? _collectAnswers() {
    final answers = <AgentQuestionAnswer>[];
    for (final question in widget.group.questions) {
      final rawValue = _answers[question.id];
      final value = rawValue?.trim();
      if ((value == null || value.isEmpty) && question.required) {
        return null;
      }
      if (value == null || value.isEmpty) continue;
      if (question.kind == AgentQuestionKind.choice) {
        if (!question.options.any((option) => option.id == value)) {
          return null;
        }
        answers.add(
          AgentQuestionAnswer(
            questionId: question.id,
            value: AgentChoiceAnswer(value),
          ),
        );
      } else {
        if (question.maxLength case final maxLength?
            when value.length > maxLength) {
          return null;
        }
        answers.add(
          AgentQuestionAnswer(
            questionId: question.id,
            value: AgentTextAnswer(value),
          ),
        );
      }
    }
    return answers;
  }

  Future<void> _submit() async {
    final l10n = context.openchatL10n;
    final answers = _collectAnswers();
    if (answers == null) {
      setState(() => _submitError = l10n.userQuestionRequiredValidation);
      return;
    }
    setState(() => _isSubmitting = true);
    var errorMessage = l10n.userQuestionSubmitFailed;
    try {
      errorMessage = await widget.onSubmit(answers) ?? '';
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'user_questions',
          context: ErrorDescription('while submitting an AI question answer'),
        ),
      );
    } finally {
      if (mounted) {
        setState(() {
          _isSubmitting = false;
          _submitError = errorMessage.isEmpty ? null : errorMessage;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final l10n = context.openchatL10n;
    final isBusy = _isSubmitting || widget.isResuming;
    final submitError = _submitError;

    return ChatSurfaceCard(
      key: ValueKey<String>('user-question-${widget.group.id}'),
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                LucideIcons.messagesSquare,
                size: 18,
                color: palette.accentIcon,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  isBusy
                      ? l10n.userQuestionResuming
                      : widget.group.isAnswered
                      ? l10n.userQuestionSaved
                      : l10n.userQuestionTitle,
                  style: theme.textTheme.titleSmall?.copyWith(
                    color: palette.text,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          for (final (index, question) in widget.group.questions.indexed) ...[
            if (index > 0) const SizedBox(height: 14),
            _QuestionInput(
              question: question,
              answer: _answers[question.id],
              controller: _textControllers[question.id],
              enabled: !isBusy && !widget.group.isAnswered,
              autofocus: widget.focusOnBuild && index == 0,
              requiredLabel: l10n.userQuestionRequiredLabel,
              onChanged: (value) =>
                  setState(() => _answers[question.id] = value),
            ),
          ],
          if (submitError != null && submitError.isNotEmpty) ...[
            const SizedBox(height: 10),
            Semantics(
              liveRegion: true,
              child: Text(
                submitError,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ),
          ],
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: FilledButton.icon(
              autofocus: widget.focusOnBuild && widget.group.isAnswered,
              onPressed: isBusy ? null : _submit,
              icon: isBusy
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(LucideIcons.send, size: 16),
              label: Text(
                isBusy
                    ? l10n.userQuestionResuming
                    : widget.group.isAnswered
                    ? l10n.userQuestionContinue
                    : l10n.userQuestionSubmit,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _QuestionInput extends StatelessWidget {
  const _QuestionInput({
    required this.question,
    required this.answer,
    required this.controller,
    required this.enabled,
    required this.autofocus,
    required this.requiredLabel,
    required this.onChanged,
  });

  final AgentQuestion question;
  final String? answer;
  final TextEditingController? controller;
  final bool enabled;
  final bool autofocus;
  final String requiredLabel;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Text(
                question.title,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: palette.text,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (question.required) ...[
              const SizedBox(width: 8),
              Text(
                requiredLabel,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: palette.secondaryText,
                ),
              ),
            ],
          ],
        ),
        if (question.description case final description?) ...[
          const SizedBox(height: 3),
          Text(
            description,
            style: theme.textTheme.bodySmall?.copyWith(
              color: palette.secondaryText,
            ),
          ),
        ],
        const SizedBox(height: 6),
        if (question.kind == AgentQuestionKind.choice)
          RadioGroup<String>(
            groupValue: answer,
            onChanged: enabled
                ? (value) {
                    if (value != null) onChanged(value);
                  }
                : (_) {},
            child: Column(
              children: [
                for (final (index, option) in question.options.indexed)
                  Material(
                    color: Colors.transparent,
                    child: RadioListTile<String>(
                      key: ValueKey<String>('${question.id}-${option.id}'),
                      value: option.id,
                      title: Text(option.label),
                      autofocus: autofocus && index == 0,
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      visualDensity: VisualDensity.compact,
                      enabled: enabled,
                    ),
                  ),
              ],
            ),
          )
        else
          TextField(
            controller: controller,
            enabled: enabled,
            autofocus: autofocus,
            maxLength: question.maxLength,
            minLines: 2,
            maxLines: 5,
            onChanged: onChanged,
            decoration: InputDecoration(
              hintText: question.placeholder,
              border: const OutlineInputBorder(),
            ),
          ),
      ],
    );
  }
}
