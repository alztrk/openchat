import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';

class ZihoraServiceClient {
  static const _requestTimeout = Duration(seconds: 15);
  static const _maximumMessageBytes = 1024 * 1024;

  final Map<int, _PendingServiceCall> _pendingRequests = {};
  final Completer<ZihoraServiceException?> _databaseInitialization =
      Completer<ZihoraServiceException?>();
  final StreamController<ZihoraServiceEvent> _events =
      StreamController<ZihoraServiceEvent>.broadcast();
  Future<void> _stdinWriteQueue = Future<void>.value();
  Process? _process;
  StreamSubscription<String>? _stdoutSubscription;
  StreamSubscription<String>? _stderrSubscription;
  int _nextRequestId = 0;
  bool _isClosing = false;

  Stream<ZihoraServiceEvent> get events => _events.stream;

  void completeDatabaseInitialization({ZihoraServiceException? error}) {
    if (_databaseInitialization.isCompleted) return;
    _databaseInitialization.complete(error);
  }

  Future<void> start() async {
    if (!Platform.isWindows) {
      throw const ZihoraServiceException(
        code: 'unsupported_platform',
        message: 'The local service is currently available on Windows only.',
      );
    }
    if (_process != null) return;

    final executable = File(
      '${File(Platform.resolvedExecutable).parent.path}${Platform.pathSeparator}zihora_service.exe',
    );
    if (!await executable.exists()) {
      throw const ZihoraServiceException(
        code: 'service_executable_missing',
        message: 'The local service executable was not found beside Zihora.',
      );
    }

    try {
      final process = await Process.start(
        executable.path,
        const <String>[],
        runInShell: false,
      );
      _process = process;
      _stdoutSubscription = process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(_handleResponseLine, onError: _handleOutputError);
      _stderrSubscription = process.stderr
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(_handleDiagnosticLine, onError: _handleOutputError);
      unawaited(process.exitCode.then(_handleProcessExit));
    } on ProcessException {
      throw const ZihoraServiceException(
        code: 'service_start_failed',
        message: 'The local service could not be started.',
      );
    }
  }

  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
  }) async {
    final operation = await startOperation(method, params: params);
    try {
      return await operation.result.timeout(_requestTimeout);
    } on TimeoutException {
      _forgetOperation(operation.id);
      unawaited(operation.cancel());
      throw ZihoraServiceException(
        code: 'service_timeout',
        message: 'The local service did not answer $method in time.',
        retryable: true,
      );
    }
  }

  Future<ZihoraServiceOperation> startOperation(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
  }) async {
    if (method != 'system.health' && method != 'system.initialize') {
      final initializationError = await _databaseInitialization.future;
      if (initializationError != null) throw initializationError;
    }

    final process = _process;
    if (process == null || _isClosing) {
      throw const ZihoraServiceException(
        code: 'service_unavailable',
        message: 'The local service is not running.',
      );
    }

    final id = ++_nextRequestId;
    final pending = _PendingServiceCall();
    _pendingRequests[id] = pending;
    final encoded = jsonEncode(<String, Object?>{
      'id': id,
      'method': method,
      'params': params,
    });
    if (utf8.encode(encoded).length > _maximumMessageBytes) {
      _forgetOperation(id);
      throw const ZihoraServiceException(
        code: 'request_too_large',
        message: 'The local service request exceeded its size limit.',
      );
    }

    try {
      await _writeRequest(process, encoded);
    } on IOException {
      _forgetOperation(id);
      throw const ZihoraServiceException(
        code: 'service_unavailable',
        message: 'The local service connection was interrupted.',
      );
    } on StateError {
      _forgetOperation(id);
      throw const ZihoraServiceException(
        code: 'service_unavailable',
        message: 'The local service connection was interrupted.',
      );
    }

    return ZihoraServiceOperation._(
      id: id,
      events: pending.events.stream,
      result: pending.result.future,
      cancel: () => cancel(id),
    );
  }

  Future<void> _writeRequest(Process process, String encoded) async {
    final previousWrite = _stdinWriteQueue;
    final writeFinished = Completer<void>();
    _stdinWriteQueue = writeFinished.future;

    try {
      await previousWrite;
      process.stdin.writeln(encoded);
      await process.stdin.flush();
    } finally {
      writeFinished.complete();
    }
  }

  Future<bool> cancel(int requestId) async {
    try {
      final response = await call(
        'system.cancel',
        params: <String, Object?>{'targetId': requestId},
      );
      return response['cancelled'] == true;
    } on ZihoraServiceException {
      return false;
    }
  }

  Future<void> close() async {
    final process = _process;
    if (process == null) {
      await _events.close();
      return;
    }
    _isClosing = true;

    try {
      await _callWhileClosing(process);
    } on ZihoraServiceException {
      // A dead service needs no shutdown request.
    }

    await process.stdin.close();
    try {
      await process.exitCode.timeout(const Duration(seconds: 2));
    } on TimeoutException {
      process.kill();
      await process.exitCode;
    }
    await _stdoutSubscription?.cancel();
    await _stderrSubscription?.cancel();
    _process = null;
    _failPending(
      const ZihoraServiceException(
        code: 'service_closed',
        message: 'The local service stopped.',
      ),
    );
    await _events.close();
  }

  Future<void> _callWhileClosing(Process process) async {
    final id = ++_nextRequestId;
    final pending = _PendingServiceCall();
    _pendingRequests[id] = pending;
    process.stdin.writeln(
      jsonEncode(<String, Object?>{
        'id': id,
        'method': 'system.shutdown',
        'params': const <String, Object?>{},
      }),
    );
    await process.stdin.flush();
    try {
      await pending.result.future.timeout(const Duration(seconds: 2));
    } on TimeoutException {
      throw const ZihoraServiceException(
        code: 'service_shutdown_timeout',
        message: 'The local service did not stop in time.',
        retryable: true,
      );
    } finally {
      _forgetOperation(id);
    }
  }

  void _handleResponseLine(String line) {
    Object? decoded;
    try {
      decoded = jsonDecode(line);
    } on FormatException {
      _failPending(
        const ZihoraServiceException(
          code: 'invalid_service_response',
          message: 'The local service returned an invalid response.',
        ),
      );
      return;
    }
    if (decoded is! Map) {
      _failPending(
        const ZihoraServiceException(
          code: 'invalid_service_response',
          message: 'The local service returned an invalid response.',
        ),
      );
      return;
    }

    final response = Map<String, Object?>.from(decoded);
    final id = response['id'];
    if (id is! int) {
      _failPending(
        const ZihoraServiceException(
          code: 'invalid_service_response',
          message: 'The local service returned an invalid response identifier.',
        ),
      );
      return;
    }

    final eventName = response['event'];
    if (eventName is String) {
      final data = response['data'];
      if (data is! Map) {
        _failPending(
          const ZihoraServiceException(
            code: 'invalid_service_response',
            message: 'The local service returned an invalid event.',
          ),
        );
        return;
      }
      final event = ZihoraServiceEvent(
        name: eventName,
        data: Map<String, Object?>.from(data),
      );
      if (id == 0) {
        if (!_events.isClosed) _events.add(event);
      } else {
        _pendingRequests[id]?.events.add(event);
      }
      return;
    }

    final pending = _pendingRequests.remove(id);
    if (pending == null) return;
    final error = response['error'];
    if (error is Map) {
      final serviceError = Map<String, Object?>.from(error);
      final code = serviceError['code'];
      final message = serviceError['message'];
      final exception = ZihoraServiceException(
        code: code is String ? code : 'service_error',
        message: message is String
            ? message
            : 'The local service request failed.',
        retryable: serviceError['retryable'] == true,
      );
      pending.result.completeError(exception);
      pending.events.addError(exception);
      unawaited(pending.events.close());
      return;
    }

    final result = response['result'];
    if (result is! Map) {
      final exception = const ZihoraServiceException(
        code: 'invalid_service_response',
        message: 'The local service response did not contain a result.',
      );
      pending.result.completeError(exception);
      pending.events.addError(exception);
      unawaited(pending.events.close());
      return;
    }
    pending.result.complete(Map<String, Object?>.from(result));
    unawaited(pending.events.close());
  }

  void _handleDiagnosticLine(String line) {
    if (line.isEmpty) return;
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: const ZihoraServiceException(
          code: 'service_diagnostic',
          message: 'The local service reported a startup or runtime error.',
        ),
        library: 'local_service',
      ),
    );
  }

  void _handleOutputError(Object error, StackTrace stackTrace) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        library: 'local_service',
        context: ErrorDescription('while reading a local service pipe'),
      ),
    );
    _failPending(
      const ZihoraServiceException(
        code: 'service_pipe_failed',
        message: 'Communication with the local service failed.',
      ),
    );
  }

  void _handleProcessExit(int exitCode) {
    if (_isClosing) return;
    _process = null;
    unawaited(_stdoutSubscription?.cancel());
    unawaited(_stderrSubscription?.cancel());
    _stdoutSubscription = null;
    _stderrSubscription = null;
    _failPending(
      ZihoraServiceException(
        code: 'service_exited',
        message:
            'The local service stopped unexpectedly (exit code $exitCode).',
      ),
    );
  }

  void _forgetOperation(int id) {
    final pending = _pendingRequests.remove(id);
    if (pending == null) return;
    if (!pending.result.isCompleted) {
      pending.result.completeError(
        const ZihoraServiceException(
          code: 'service_cancelled',
          message: 'The local service request was cancelled.',
        ),
      );
    }
    unawaited(pending.events.close());
  }

  void _failPending(ZihoraServiceException error) {
    final pending = List<_PendingServiceCall>.of(_pendingRequests.values);
    _pendingRequests.clear();
    for (final call in pending) {
      if (!call.result.isCompleted) call.result.completeError(error);
      call.events.addError(error);
      unawaited(call.events.close());
    }
  }
}

class _PendingServiceCall {
  final Completer<Map<String, Object?>> result =
      Completer<Map<String, Object?>>();
  final StreamController<ZihoraServiceEvent> events =
      StreamController<ZihoraServiceEvent>();
}

class ZihoraServiceEvent {
  const ZihoraServiceEvent({required this.name, required this.data});

  final String name;
  final Map<String, Object?> data;
}

class ZihoraServiceOperation {
  const ZihoraServiceOperation._({
    required this.id,
    required this.events,
    required this.result,
    required this.cancel,
  });

  final int id;
  final Stream<ZihoraServiceEvent> events;
  final Future<Map<String, Object?>> result;
  final Future<bool> Function() cancel;
}

class ZihoraServiceException implements Exception {
  const ZihoraServiceException({
    required this.code,
    required this.message,
    this.retryable = false,
  });

  final String code;
  final String message;
  final bool retryable;

  @override
  String toString() => 'ZihoraServiceException($code): $message';
}
