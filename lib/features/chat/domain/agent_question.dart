import 'dart:convert';

enum AgentQuestionKind { choice, text }

const _maximumQuestionTextBytes = 16 * 1024;
const _maximumQuestionCount = 32;
const _maximumQuestionOptionCount = 128;
const _maximumIdentifierBytes = 128;

bool _isValidIdentifier(String value) =>
    value.isNotEmpty &&
    value.length <= _maximumIdentifierBytes &&
    value.codeUnits.every(
      (unit) =>
          (unit >= 48 && unit <= 57) ||
          (unit >= 65 && unit <= 90) ||
          (unit >= 97 && unit <= 122) ||
          unit == 45 ||
          unit == 95,
    );

bool _isValidText(String value, {required bool allowEmpty}) =>
    (allowEmpty || value.trim().isNotEmpty) &&
    utf8.encode(value).length <= _maximumQuestionTextBytes;

class AgentQuestionOption {
  const AgentQuestionOption({required this.id, required this.label});

  final String id;
  final String label;

  static AgentQuestionOption fromJson(Object? value) {
    if (value is! Map<String, Object?> ||
        value['id'] is! String ||
        value['label'] is! String) {
      throw const FormatException('A question option was invalid.');
    }
    final id = value['id'];
    final label = value['label'];
    if (id is! String ||
        label is! String ||
        !_isValidIdentifier(id) ||
        !_isValidText(label, allowEmpty: false)) {
      throw const FormatException('A question option was invalid.');
    }
    return AgentQuestionOption(id: id, label: label);
  }
}

class AgentQuestion {
  const AgentQuestion({
    required this.id,
    required this.kind,
    required this.title,
    required this.options,
    required this.required,
    this.description,
    this.placeholder,
    this.maxLength,
  });

  final String id;
  final AgentQuestionKind kind;
  final String title;
  final String? description;
  final List<AgentQuestionOption> options;
  final bool required;
  final String? placeholder;
  final int? maxLength;

  static AgentQuestion fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A question was invalid.');
    }
    final id = value['id'];
    final kindValue = value['kind'];
    final title = value['title'];
    final rawOptions = value['options'];
    final isRequired = value['required'];
    final description = value['description'];
    final placeholder = value['placeholder'];
    final maxLength = value['maxLength'];
    if (id is! String ||
        !_isValidIdentifier(id) ||
        title is! String ||
        !_isValidText(title, allowEmpty: false) ||
        kindValue is! String ||
        rawOptions is! List<Object?> ||
        isRequired is! bool ||
        (description != null &&
            (description is! String ||
                !_isValidText(description, allowEmpty: true))) ||
        (placeholder != null &&
            (placeholder is! String ||
                !_isValidText(placeholder, allowEmpty: true))) ||
        (maxLength != null &&
            (maxLength is! int ||
                maxLength < 1 ||
                maxLength > _maximumQuestionTextBytes))) {
      throw const FormatException('A question was invalid.');
    }
    final kind = switch (kindValue) {
      'choice' => AgentQuestionKind.choice,
      'text' => AgentQuestionKind.text,
      _ => throw const FormatException('A question type was invalid.'),
    };
    final options = rawOptions
        .map(AgentQuestionOption.fromJson)
        .toList(growable: false);
    if (options.length > _maximumQuestionOptionCount ||
        (kind == AgentQuestionKind.choice && options.length < 2) ||
        (kind == AgentQuestionKind.text && options.isNotEmpty)) {
      throw const FormatException('A question has invalid options.');
    }
    return AgentQuestion(
      id: id,
      kind: kind,
      title: title,
      description: description is String ? description : null,
      options: options,
      required: isRequired,
      placeholder: placeholder is String ? placeholder : null,
      maxLength: maxLength is int ? maxLength : null,
    );
  }
}

class AgentQuestionGroup {
  const AgentQuestionGroup({
    required this.id,
    required this.runId,
    required this.conversationId,
    required this.revision,
    required this.questions,
    this.isAnswered = false,
    this.savedAnswers = const <AgentQuestionAnswer>[],
    this.assistantMessageId,
    this.toolCallId,
    this.toolName,
    this.toolArguments,
  });

  final String id;
  final String runId;
  final String conversationId;
  final int revision;
  final List<AgentQuestion> questions;
  final bool isAnswered;
  final List<AgentQuestionAnswer> savedAnswers;
  final String? assistantMessageId;
  final String? toolCallId;
  final String? toolName;
  final Object? toolArguments;

  static AgentQuestionGroup fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A question group was invalid.');
    }
    final id = value['groupId'];
    final runId = value['runId'];
    final conversationId = value['conversationId'];
    final revision = value['revision'];
    final status = value['status'];
    final rawAnswers = value['answers'];
    final rawQuestions = value['questions'];
    final assistantMessageId = value['assistantMessageId'];
    final toolCallId = value['toolCallId'];
    final toolName = value['toolName'];
    final toolArguments = value['toolArguments'];
    if (id is! String ||
        !_isValidIdentifier(id) ||
        runId is! String ||
        runId.isEmpty ||
        conversationId is! String ||
        conversationId.isEmpty ||
        revision is! int ||
        revision < 0 ||
        (status != 'pending' && status != 'answered') ||
        (rawAnswers != null && rawAnswers is! List<Object?>) ||
        rawQuestions is! List<Object?> ||
        (assistantMessageId != null && assistantMessageId is! String) ||
        (toolCallId != null && toolCallId is! String) ||
        (toolName != null && toolName is! String) ||
        (toolArguments != null && toolArguments is! Map<String, Object?>) ||
        rawQuestions.isEmpty ||
        rawQuestions.length > _maximumQuestionCount) {
      throw const FormatException('A question group was invalid.');
    }
    final questions = rawQuestions
        .map(AgentQuestion.fromJson)
        .toList(growable: false);
    final ids = questions.map((question) => question.id).toSet();
    if (ids.length != questions.length ||
        utf8.encode(jsonEncode(rawQuestions)).length > 128 * 1024) {
      throw const FormatException('A question group contains duplicate IDs.');
    }
    final savedAnswers = rawAnswers is List<Object?>
        ? rawAnswers.map(AgentQuestionAnswer.fromJson).toList(growable: false)
        : const <AgentQuestionAnswer>[];
    return AgentQuestionGroup(
      id: id,
      runId: runId,
      conversationId: conversationId,
      revision: revision,
      questions: questions,
      isAnswered: status == 'answered',
      savedAnswers: savedAnswers,
      assistantMessageId: assistantMessageId is String
          ? assistantMessageId
          : null,
      toolCallId: toolCallId is String ? toolCallId : null,
      toolName: toolName is String ? toolName : null,
      toolArguments: toolArguments,
    );
  }
}

sealed class AgentQuestionAnswerValue {
  const AgentQuestionAnswerValue();

  Map<String, Object?> toJson();

  static AgentQuestionAnswerValue fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A question answer was invalid.');
    }
    final kind = value['kind'];
    final optionId = value['optionId'];
    final text = value['text'];
    if (kind == 'choice' && optionId is String) {
      return AgentChoiceAnswer(optionId);
    }
    if (kind == 'text' && text is String) return AgentTextAnswer(text);
    throw const FormatException('A question answer was invalid.');
  }
}

class AgentChoiceAnswer extends AgentQuestionAnswerValue {
  const AgentChoiceAnswer(this.optionId);

  final String optionId;

  @override
  Map<String, Object?> toJson() => <String, Object?>{
    'kind': 'choice',
    'optionId': optionId,
  };
}

class AgentTextAnswer extends AgentQuestionAnswerValue {
  const AgentTextAnswer(this.text);

  final String text;

  @override
  Map<String, Object?> toJson() => <String, Object?>{
    'kind': 'text',
    'text': text,
  };
}

class AgentQuestionAnswer {
  const AgentQuestionAnswer({required this.questionId, required this.value});

  final String questionId;
  final AgentQuestionAnswerValue value;

  Map<String, Object?> toJson() => <String, Object?>{
    'questionId': questionId,
    'value': value.toJson(),
  };

  static AgentQuestionAnswer fromJson(Object? value) {
    if (value is! Map<String, Object?> || value['questionId'] is! String) {
      throw const FormatException('A question answer was invalid.');
    }
    final questionId = value['questionId'];
    if (questionId is! String || !_isValidIdentifier(questionId)) {
      throw const FormatException('A question answer was invalid.');
    }
    return AgentQuestionAnswer(
      questionId: questionId,
      value: AgentQuestionAnswerValue.fromJson(value['value']),
    );
  }
}
