import 'dart:async';
import 'dart:convert';
import 'dart:io';

Future<void> main() async {
  if (!Platform.isWindows) {
    stderr.writeln('This development watcher currently supports Windows only.');
    exitCode = 1;
    return;
  }

  final root = Directory.current.absolute;
  final watcher = _OpenChatDevWatcher(root);
  await watcher.run();
}

class _OpenChatDevWatcher {
  _OpenChatDevWatcher(this.root);

  final Directory root;
  late final String _rootPath = root.path.replaceAll('\\', '/').toLowerCase();
  final Map<int, Completer<Object?>> _pendingRequests = {};
  StreamSubscription<FileSystemEvent>? _libChanges;
  StreamSubscription<FileSystemEvent>? _nativeChanges;
  StreamSubscription<FileSystemEvent>? _windowsChanges;
  Process? _flutter;
  String? _appId;
  bool _appReady = false;
  Timer? _changeDebounce;
  int _nextRequestId = 0;
  bool _hasDartChanges = false;
  bool _hasLocalizationChanges = false;
  bool _hasNativeChanges = false;
  bool _restartingForNativeChanges = false;
  bool _stopping = false;
  Completer<void>? _nativeRestartDone;

  Future<void> run() async {
    _libChanges = Directory('${root.path}${Platform.pathSeparator}lib')
        .watch(recursive: true)
        .listen(_onFileChange);
    _nativeChanges = Directory(
      '${root.path}${Platform.pathSeparator}native${Platform.pathSeparator}openchat-service',
    ).watch(recursive: true).listen(_onFileChange);
    _windowsChanges = Directory('${root.path}${Platform.pathSeparator}windows')
        .watch(recursive: true)
        .listen(_onFileChange);
    final stdinSubscription = stdin
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen((line) {
          if (line.trim().toLowerCase() == 'q') unawaited(_stopFlutter());
        });
    final signalSubscription = ProcessSignal.sigint.watch().listen((_) {
      unawaited(_stopFlutter());
    });
    try {
      await _startFlutter();
      while (true) {
        final flutter = _flutter;
        if (flutter == null) break;
        final result = await flutter.exitCode;
        if (_flutter != flutter) continue;
        if (_restartingForNativeChanges) {
          await _nativeRestartDone?.future;
          continue;
        }
        if (result != 0) {
          stderr.writeln('Flutter exited with code $result.');
          exitCode = result;
        }
        break;
      }
    } finally {
      await stdinSubscription.cancel();
      await signalSubscription.cancel();
      await _disposeWatchers();
    }
  }

  Future<void> _startFlutter() async {
    stdout.writeln('Starting OpenChat in Flutter debug mode.');
    final flutter = await Process.start(
      'flutter',
      const ['run', '--no-pub', '--machine', '-d', 'windows'],
      workingDirectory: root.path,
      runInShell: true,
    );
    _flutter = flutter;
    _appId = null;
    _appReady = false;
    flutter.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen(_handleFlutterOutput);
    flutter.stderr
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .listen(stderr.writeln);
    unawaited(flutter.exitCode.then(_failPendingRequests));
  }

  void _handleFlutterOutput(String line) {
    Object? decoded;
    try {
      decoded = jsonDecode(line);
    } on FormatException {
      stdout.writeln(line);
      return;
    }
    final messages = decoded is List ? decoded : [decoded];
    for (final message in messages) {
      if (message is! Map<String, dynamic>) continue;
      final event = message['event'];
      final params = message['params'];
      if ((event == 'app.start' || event == 'app.started') &&
          params is Map<String, dynamic>) {
        _appId ??= params['appId'] as String?;
      }
      if (event == 'app.started') {
        _appReady = true;
        stdout.writeln('OpenChat is ready; automatic reload is active.');
        if (_hasNativeChanges) {
          _hasNativeChanges = false;
          unawaited(
            _restartForNativeChanges('a native source changed during startup'),
          );
        } else if (_hasDartChanges) {
          _hasDartChanges = false;
          unawaited(_hotReload());
        }
      }

      final id = message['id'];
      if (id is int) {
        final completer = _pendingRequests.remove(id);
        if (completer != null) {
          if (message.containsKey('error')) {
            completer.completeError(
              StateError('Flutter command failed: ${message['error']}'),
            );
          } else {
            completer.complete(message['result']);
          }
        }
      }
    }
  }

  void _onFileChange(FileSystemEvent event) {
    final path = event.path.replaceAll('\\', '/');
    if (_isDartSource(path)) {
      _hasDartChanges = true;
      _hasLocalizationChanges =
          _hasLocalizationChanges || path.toLowerCase().endsWith('.arb');
    } else if (_isNativeSource(path)) {
      _hasNativeChanges = true;
    } else {
      return;
    }

    _changeDebounce?.cancel();
    _changeDebounce = Timer(const Duration(milliseconds: 700), () {
      if (_hasNativeChanges && !_restartingForNativeChanges) {
        _hasNativeChanges = false;
        _hasDartChanges = false;
        unawaited(_restartForNativeChanges(path));
      } else if (_hasDartChanges && !_restartingForNativeChanges) {
        final regenerateLocalizations = _hasLocalizationChanges;
        _hasDartChanges = false;
        _hasLocalizationChanges = false;
        unawaited(_reloadDartChanges(regenerateLocalizations));
      }
    });
  }

  bool _isDartSource(String path) =>
      _isUnder(path, 'lib/') &&
      !path.toLowerCase().contains('/lib/l10n/generated/') &&
      _hasExtension(path, const ['.dart', '.arb']);

  bool _isNativeSource(String path) =>
      _isRustSource(path) || _isWindowsRunnerSource(path);

  bool _isRustSource(String path) =>
      _isUnder(path, 'native/openchat-service/') &&
      _hasFileSuffix(path, const ['.rs', 'cargo.toml', 'cargo.lock']);

  bool _isWindowsRunnerSource(String path) {
    final relativePath = _relativePath(path, 'windows/');
    if (relativePath == null ||
        relativePath.startsWith('flutter/') ||
        relativePath.startsWith('ephemeral/') ||
        relativePath.startsWith('build/')) {
      return false;
    }
    final isTopLevelCmake = relativePath == 'cmakelists.txt';
    final isRunnerSource =
        relativePath.startsWith('runner/') &&
        _hasFileSuffix(relativePath, const [
          '.cmake',
          'cmakelists.txt',
          '.cpp',
          '.h',
          '.rc',
        ]);
    return isTopLevelCmake || isRunnerSource;
  }

  bool _isUnder(String path, String directory) =>
      _relativePath(path, directory) != null;

  String? _relativePath(String path, String directory) {
    final normalizedPath = path.replaceAll('\\', '/').toLowerCase();
    final prefix = '$_rootPath/$directory';
    return normalizedPath.startsWith(prefix)
        ? normalizedPath.substring(prefix.length)
        : null;
  }

  bool _hasExtension(String path, List<String> extensions) =>
      extensions.any(path.toLowerCase().endsWith);

  bool _hasFileSuffix(String path, List<String> suffixes) =>
      suffixes.any(path.endsWith);

  Future<void> _hotReload() async {
    final appId = _appId;
    if (appId == null || !_appReady) {
      _hasDartChanges = true;
      return;
    }
    try {
      final result = await _request('app.restart', {
        'appId': appId,
        'fullRestart': false,
        'reason': 'file changed',
        'debounce': true,
      });
      if (result is Map && result['code'] != 0) {
        throw StateError('Flutter hot reload failed: ${result['message']}');
      }
      stdout.writeln('Hot reload complete.');
    } on Object catch (error) {
      stderr.writeln('$error');
    }
  }

  Future<void> _reloadDartChanges(bool regenerateLocalizations) async {
    if (regenerateLocalizations) {
      stdout.writeln('Localization files changed. Regenerating translations.');
      final result = await Process.run(
        'flutter',
        const ['gen-l10n'],
        workingDirectory: root.path,
        runInShell: true,
      );
      if (result.exitCode != 0) {
        stderr.writeln(result.stderr);
        stderr.writeln('Localization generation failed; hot reload skipped.');
        return;
      }
    }
    stdout.writeln('Dart change detected. Hot reloading OpenChat.');
    await _hotReload();
  }

  Future<void> _restartForNativeChanges(String changedPath) async {
    final appId = _appId;
    final flutter = _flutter;
    if (appId == null || flutter == null || !_appReady) {
      _hasNativeChanges = true;
      return;
    }

    _restartingForNativeChanges = true;
    _nativeRestartDone = Completer<void>();
    stdout.writeln(
      'Native change detected. Rebuilding and restarting OpenChat.',
    );
    try {
      await _stopFlutter();
      final result = await flutter.exitCode.timeout(
        const Duration(seconds: 30),
      );
      if (result != 0) {
        throw StateError('Flutter exited with code $result before rebuild.');
      }
      await _startFlutter();
    } on Object catch (error) {
      stderr.writeln('Could not rebuild after $changedPath: $error');
      exitCode = 1;
    } finally {
      _restartingForNativeChanges = false;
      _nativeRestartDone?.complete();
      _nativeRestartDone = null;
    }
  }

  Future<void> _stopFlutter() async {
    if (_stopping) return;
    _stopping = true;
    final flutter = _flutter;
    if (flutter == null) {
      _stopping = false;
      return;
    }
    try {
      final appId = _appId;
      if (appId != null) {
        final stopped = await _request('app.stop', {'appId': appId});
        if (stopped != true) {
          throw StateError('Flutter did not stop the running app.');
        }
      }
      final hasExited = await Future.any<bool>([
        flutter.exitCode.then((_) => true),
        Future<bool>.delayed(const Duration(milliseconds: 500), () => false),
      ]);
      if (!hasExited) await _request('daemon.shutdown', const {});
    } catch (error) {
      stderr.writeln('Could not stop Flutter cleanly: $error');
      rethrow;
    } finally {
      _stopping = false;
    }
  }

  Future<Object?> _request(String method, Map<String, Object?> params) {
    final flutter = _flutter;
    if (flutter == null) {
      return Future<Object?>.error(StateError('Flutter is not running.'));
    }
    final id = ++_nextRequestId;
    final completer = Completer<Object?>();
    _pendingRequests[id] = completer;
    flutter.stdin.writeln(
      jsonEncode([
        {'method': method, 'id': id, 'params': params},
      ]),
    );
    return completer.future
        .timeout(const Duration(seconds: 30))
        .whenComplete(() => _pendingRequests.remove(id));
  }

  void _failPendingRequests(int exitCode) {
    final error = StateError('Flutter exited with code $exitCode.');
    for (final completer in _pendingRequests.values) {
      if (!completer.isCompleted) completer.completeError(error);
    }
    _pendingRequests.clear();
  }

  Future<void> _disposeWatchers() async {
    _changeDebounce?.cancel();
    await _libChanges?.cancel();
    await _nativeChanges?.cancel();
    await _windowsChanges?.cancel();
  }
}
