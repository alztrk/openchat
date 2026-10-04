// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class AppLocalizationsDe extends AppLocalizations {
  AppLocalizationsDe([String locale = 'de']) : super(locale);

  @override
  String get appTitle => 'OpenChat';

  @override
  String get newChat => 'Neuer Chat';

  @override
  String get chats => 'Chats';

  @override
  String get collapseSidebars => 'Seitenleisten einklappen';

  @override
  String get showSidebars => 'Seitenleisten anzeigen';

  @override
  String get home => 'Startseite';

  @override
  String get extensions => 'Erweiterungen';

  @override
  String get scheduled => 'Geplant';

  @override
  String get design => 'Design';

  @override
  String get security => 'Sicherheit';

  @override
  String get sectionUnavailable => 'Dieser Bereich ist noch nicht verfügbar.';

  @override
  String get settings => 'Einstellungen';

  @override
  String get settingsDescription =>
      'Verbindungen, Darstellung und lokale Daten';

  @override
  String get connections => 'Verbindungen';

  @override
  String get models => 'Modelle';

  @override
  String get modelsDescription =>
      'Modelle verbundener Anbieter verwalten, ein Standardmodell festlegen und nicht benötigte Modelle ausblenden.';

  @override
  String get localEngines => 'Lokale Engines';

  @override
  String get localEnginesDescription =>
      'Verifizierte lokale Laufzeitumgebungen installieren, eigene Modelldateien registrieren und den Zustand einer Laufzeit prüfen. Modelle in einen Engine-Ordner verschieben oder kopieren oder am aktuellen Speicherort belassen.';

  @override
  String get localEnginesUnavailable =>
      'Der Dienst für lokale Engines ist noch nicht verfügbar.';

  @override
  String get localEnginesLoadFailed =>
      'Informationen zu lokalen Engines konnten nicht geladen werden.';

  @override
  String get localEnginesEmpty =>
      'Keine Releases für lokale Engines verfügbar.';

  @override
  String get localEnginesReload => 'Neu laden';

  @override
  String localEngineRelease(String tag) {
    return 'Release $tag';
  }

  @override
  String get localEngineVariants => 'Pakete';

  @override
  String get localEngineStable => 'Stabil';

  @override
  String get localEnginePreview => 'Vorschau';

  @override
  String get localEngineNightly => 'Nightly';

  @override
  String get localEngineRecommended => 'Empfohlen';

  @override
  String get localEngineAvailable => 'Verfügbar';

  @override
  String get localEngineInstalled => 'Installiert';

  @override
  String get localEngineNotInstalled => 'Nicht installiert';

  @override
  String get localEngineBlocked => 'Blockiert';

  @override
  String get localEngineDeprecated => 'Veraltet';

  @override
  String get localEngineWindowsDeprecatedReason =>
      'Neue Installationen von vLLM und ExLlama sind unter Windows deaktiviert. Vorhandene Laufzeitdateien und Modellregistrierungen bleiben erhalten.';

  @override
  String get localEngineUnsupportedPlatform => 'Nicht unterstützte Plattform';

  @override
  String get localEngineHardwareUnavailable => 'Hardware nicht verfügbar';

  @override
  String get localEngineDriverUnsupported => 'NVIDIA-Treiber aktualisieren';

  @override
  String get localEngineDriverVersionUnavailable =>
      'NVIDIA-Treiberversion konnte nicht überprüft werden';

  @override
  String get localEngineVllmBlockedReason =>
      'vLLM benötigt einen NVIDIA-Treiber ab Version 580 und eine vorhandene Linux-Umgebung. Unter Windows wird eine vorhandene WSL2-Distribution mit GPU-Zugriff verwendet.';

  @override
  String get localEngineExllamaBlockedReason =>
      'Die ExLlamaV3-Laufzeit ist noch nicht für die Installation bereit. OpenChat muss zunächst den vollständigen Satz der TabbyAPI-, PyTorch-, Triton-, Flash-Linear-Attention- und Python-Abhängigkeiten festlegen und verifizieren.';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Anforderungen: $requirements';
  }

  @override
  String get localEngineInstall => 'Installieren';

  @override
  String get localEngineInstalling => 'Installation wird vorbereitet …';

  @override
  String get localEngineInstallProgress =>
      'Fortschritt der Engine-Installation';

  @override
  String get localEngineCancelInstall => 'Installation abbrechen';

  @override
  String get localEngineCancellingInstall => 'Wird abgebrochen …';

  @override
  String get localEngineInstallFailed =>
      'Die lokale Engine konnte nicht installiert werden. Der Katalog wurde aktualisiert.';

  @override
  String get localEngineHealth => 'Laufzeitstatus';

  @override
  String get localEngineExecutable => 'llama-server-Programmdatei';

  @override
  String get localEngineExecutableDescription =>
      'Wähle eine llama-server-Datei, mit der OpenChat registrierte GGUF-Modelle startet. Diese Funktion ist getrennt von der Verbindung zu einem selbst gestarteten Server.';

  @override
  String get localEngineExecutableChoose => 'Programmdatei auswählen';

  @override
  String get localEngineExecutableClear => 'Auswahl entfernen';

  @override
  String get localEngineExecutableNotConfigured =>
      'Keine Programmdatei ausgewählt. OpenChat kann ein installiertes Paket verwenden.';

  @override
  String get localEngineExecutableMissing =>
      'Die gespeicherte Programmdatei wurde nicht gefunden. Wähle sie erneut aus.';

  @override
  String get localEngineExecutableInvalid =>
      'Wähle eine vorhandene llama-server-Programmdatei aus.';

  @override
  String get localEngineSettingsFailed =>
      'Die llama-server-Einstellung konnte nicht gespeichert werden.';

  @override
  String get localEngineExternalServerCheck =>
      'Nach laufendem llama-server suchen';

  @override
  String get localEngineExternalServerNotConnected =>
      'Es ist kein selbst gestarteter Server verbunden. OpenChat startet oder beendet diesen Server nicht.';

  @override
  String get localEngineExternalServerConnecting =>
      'Der ausgewählte lokale Server wird geprüft …';

  @override
  String get localEngineExternalServerFoundTitle =>
      'Laufender llama-server gefunden';

  @override
  String localEngineExternalServerFoundDescription(int port) {
    return 'Ein llama.cpp-Server lauscht auf Port $port. OpenChat verbindet sich und listet seine Modelle. Das Trennen in OpenChat beendet den Server nicht.';
  }

  @override
  String get localEngineExternalServerNotNow => 'Jetzt nicht';

  @override
  String get localEngineExternalServerConnect => 'Verbinden';

  @override
  String localEngineExternalServerConnected(int port, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Modelle verfügbar',
      one: '1 Modell verfügbar',
    );
    return 'Mit Port $port verbunden. $_temp0.';
  }

  @override
  String get localEngineManagedModelSection =>
      'Von OpenChat verwaltete Modelle';

  @override
  String localEngineManagedServerModelSection(int port) {
    return 'OpenChat-Server · 127.0.0.1:$port';
  }

  @override
  String localEngineExternalModelSection(int port) {
    return 'Benutzerserver · 127.0.0.1:$port';
  }

  @override
  String get localEngineExternalServerDisconnect => 'Trennen';

  @override
  String get localEngineExternalServerNotFound =>
      'Kein laufender llama-server gefunden.';

  @override
  String get localEngineExternalServerScanFailed =>
      'Laufende llama-server-Prozesse konnten nicht geprüft werden.';

  @override
  String get localEngineExternalServerConnectFailed =>
      'Verbindung fehlgeschlagen. Prüfe, ob llama-server bereit ist und seinen lokalen Modellendpunkt bereitstellt.';

  @override
  String get localEngineExternalServerAuthRequired =>
      'Dieser Server erfordert eine Authentifizierung. OpenChat liest oder übernimmt keine Zugangsdaten aus anderen Prozessen.';

  @override
  String get localEngineRunning => 'Wird ausgeführt';

  @override
  String get localEngineStopped => 'Angehalten';

  @override
  String get localEngineUnhealthy => 'Keine Antwort';

  @override
  String get localEngineUnavailable =>
      'Diese Engine kann noch nicht gestartet werden.';

  @override
  String get localEngineStartModel => 'Modell starten';

  @override
  String get localEngineStopModel => 'Engine stoppen';

  @override
  String get localModels => 'Registrierte Modelle';

  @override
  String get localModelsEmpty =>
      'Für diese Engine sind keine Modelle registriert.';

  @override
  String get localModelsPageTitle => 'Lokale Modelle';

  @override
  String get localModelsPageDescription =>
      'Heruntergeladene und registrierte lokale Modelle anzeigen.';

  @override
  String get localModelsPageEmpty => 'Noch keine lokalen Modelle registriert.';

  @override
  String get localModelsLoadFailed =>
      'Lokale Modelle konnten nicht geladen werden.';

  @override
  String get localModelsRefresh => 'Aktualisieren';

  @override
  String get localModelsDiscover => 'Modelle entdecken';

  @override
  String get localModelAddFile => 'Modelldatei hinzufügen';

  @override
  String get localModelAddFolder => 'Modellordner hinzufügen';

  @override
  String get localModelStorageChoiceTitle =>
      'Speicherort für das Modell auswählen';

  @override
  String localModelStorageChoiceTarget(String folder) {
    return 'Ausgewählter Modellordner: $folder';
  }

  @override
  String get localModelDirectoryTitle => 'Modellordner';

  @override
  String get localModelDirectoryDescription =>
      'Wähle den Speicherort für Modelle dieser Engine. Bereits registrierte Modelle werden dadurch nicht verschoben.';

  @override
  String get localModelChooseDirectory => 'Ordner auswählen';

  @override
  String get localModelUseDefaultDirectory => 'Standard verwenden';

  @override
  String get localModelScanDirectory => 'Ordner durchsuchen';

  @override
  String get localModelScanningDirectory => 'Ordner wird durchsucht …';

  @override
  String get localModelDiscoveryTitle => 'Nicht registrierte Modelle gefunden';

  @override
  String localModelDiscoveryPrompt(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'OpenChat hat $count unterstützte, noch nicht registrierte Modelle in diesem Ordner gefunden. Jetzt registrieren?',
      one: 'OpenChat hat 1 unterstütztes, noch nicht registriertes Modell in diesem Ordner gefunden. Jetzt registrieren?',
    );
    return '$_temp0';
  }

  @override
  String get localModelDiscoveryTruncated =>
      'Die sichere Suchgrenze wurde erreicht. Wähle einen kleineren Ordner, um weitere Modelle zu finden.';

  @override
  String get localModelDiscoveryEmpty =>
      'In diesem Ordner wurden keine neuen unterstützten Modelle gefunden.';

  @override
  String get localModelDiscoveryRegisterAll => 'Gefundene Modelle registrieren';

  @override
  String localModelDiscoveryRegistered(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Modelle registriert.',
      one: '1 Modell registriert.',
    );
    return '$_temp0';
  }

  @override
  String localModelDiscoveryPartial(int registered, int total) {
    return '$registered von $total Modellen registriert. Einige Modelle konnten nicht registriert werden.';
  }

  @override
  String get localModelDirectoryUnavailable =>
      'Dieser Modellordner ist nicht verfügbar. Wähle einen vorhandenen Ordner, auf den OpenChat zugreifen kann.';

  @override
  String get localModelDiscoveryFailed =>
      'Der Modellordner konnte nicht durchsucht werden. Prüfe die Zugriffsrechte und versuche es erneut.';

  @override
  String get localModelMoveToFolder => 'Modell in diesen Ordner verschieben';

  @override
  String get localModelCopyToFolder => 'Modell in diesen Ordner kopieren';

  @override
  String get localModelKeepInPlace =>
      'Modell am aktuellen Speicherort belassen';

  @override
  String get localModelSaving => 'Modell wird gespeichert …';

  @override
  String get localModelTransferError =>
      'Das Modell konnte nicht kopiert oder verschoben werden. Das Original wurde am bisherigen Speicherort belassen.';

  @override
  String get localModelTransferRecoveryError =>
      'Das Modell konnte nicht registriert oder wiederhergestellt werden. Eine vollständige Kopie befindet sich weiterhin im OpenChat-Modellordner. Wählen Sie sie dort zur Registrierung aus.';

  @override
  String get localModelRemove => 'Registrierung entfernen';

  @override
  String get localModelRemoveConfirmation =>
      'Nur die OpenChat-Registrierung wird entfernt. Die Modelldatei bleibt auf dem Datenträger. Fortfahren?';

  @override
  String get localModelCancelStart => 'Startvorgang abbrechen';

  @override
  String get localModelStopping => 'Laufzeit wird angehalten …';

  @override
  String get localModelActionError =>
      'Die Aktion für das lokale Modell konnte nicht abgeschlossen werden.';

  @override
  String get localModelPathMissing =>
      'Der Modellpfad wurde nicht gefunden. Verschieben Sie das Modell zurück oder entfernen Sie seine Registrierung.';

  @override
  String get localModelEngineNotReady =>
      'Verfügbar, sobald diese Engine installiert ist und Modelle ausführen kann.';

  @override
  String get localModelInvalid =>
      'Die ausgewählte Datei oder der ausgewählte Ordner ist kein gültiges Modell für diese Engine.';

  @override
  String get localModelPathError =>
      'Auf die ausgewählte Modelldatei oder den ausgewählten Modellordner konnte nicht zugegriffen werden.';

  @override
  String get localModelStoragePathError =>
      'OpenChat konnte seine Modellordner nicht erstellen. Prüfen Sie Speicherplatz und Berechtigungen und versuchen Sie es erneut.';

  @override
  String get localModelStorageError =>
      'Die Modellregistrierung konnte nicht in der Datenbank gespeichert werden.';

  @override
  String get localModelStartError =>
      'Das Modell konnte nicht gestartet werden. Prüfen Sie die Laufzeitinstallation und die Modelldatei.';

  @override
  String get localModelStartTimeout =>
      'Das Modell wurde nicht rechtzeitig bereit. Versuchen Sie ein kleineres Modell oder prüfen Sie Ihre Hardware.';

  @override
  String get localModelRuntimeUnavailable =>
      'Das lokale Modell reagiert nicht mehr. Starten Sie es unter Einstellungen > Lokale Engines neu und versuchen Sie es erneut.';

  @override
  String get localModelContextUnavailable =>
      'Die lokale Laufzeit hat ihr aktives Kontextfenster nicht gemeldet. Aktualisieren oder installieren Sie llama.cpp erneut und versuchen Sie es noch einmal.';

  @override
  String get localModelInferenceFailed =>
      'Das lokale Modell konnte die Anfrage nicht verarbeiten. Prüfen Sie das Chat-Template und den verfügbaren Arbeitsspeicher.';

  @override
  String get localModelSaved => 'Lokales Modell registriert.';

  @override
  String get localModelRemoved => 'Registrierung des lokalen Modells entfernt.';

  @override
  String get localEngineStageDownloading => 'Wird heruntergeladen';

  @override
  String get localEngineStageVerifying => 'Wird verifiziert';

  @override
  String get localEngineStageExtracting => 'Wird entpackt';

  @override
  String get localEngineStageRuntimeSetup =>
      'Python-Laufzeit wird eingerichtet';

  @override
  String get localEngineStagePublishing => 'Installation wird abgeschlossen';

  @override
  String get localEngineStageReady => 'Bereit';

  @override
  String get defaultModel => 'Standard';

  @override
  String get setDefaultModel => 'Als Standard festlegen';

  @override
  String get clearDefaultModel => 'Standard entfernen';

  @override
  String defaultModelUpdated(String model) {
    return 'Standardmodell aktualisiert: $model';
  }

  @override
  String get defaultModelCleared => 'Standardmodell entfernt.';

  @override
  String get hideModel => 'Ausblenden';

  @override
  String get showModel => 'Anzeigen';

  @override
  String get hiddenModel => 'Ausgeblendet';

  @override
  String modelHidden(String model) {
    return 'Modell ausgeblendet: $model';
  }

  @override
  String modelUnhidden(String model) {
    return 'Modell angezeigt: $model';
  }

  @override
  String get noModelsFound => 'Keine Modelle gefunden.';

  @override
  String get refreshModels => 'Modelle aktualisieren';

  @override
  String get connectedProvidersModels => 'Modelle verbundener Anbieter';

  @override
  String get noConnectedProviders =>
      'Noch keine Anbieter verbunden. Verbinden Sie Konten oder fügen Sie API-Schlüssel im Tab „Verbindungen“ hinzu.';

  @override
  String get sharedInstructions => 'Gemeinsame Anweisungen';

  @override
  String get sharedInstructionsDescription =>
      'Diese Anweisungen werden an jeden verbundenen Anbieter gesendet. Der Zugriff der Dateitools richtet sich nach der Einstellung für den Toolzugriff.';

  @override
  String get sharedInstructionsHint =>
      'Beschreiben Sie, wie Antworten formuliert werden sollen …';

  @override
  String get sharedInstructionsLoadFailed =>
      'Gemeinsame Anweisungen konnten nicht geladen werden. Versuchen Sie es erneut.';

  @override
  String get sharedInstructionsSaveFailed =>
      'Gemeinsame Anweisungen konnten nicht gespeichert werden.';

  @override
  String get sharedInstructionsSaved => 'Gemeinsame Anweisungen gespeichert.';

  @override
  String get sharedInstructionsTooLong =>
      'Gemeinsame Anweisungen dürfen höchstens 4096 Zeichen enthalten.';

  @override
  String get chatGptProvider => 'ChatGPT';

  @override
  String get openCodeProvider => 'OpenCode';

  @override
  String get geminiProvider => 'Gemini';

  @override
  String get groqProvider => 'Groq';

  @override
  String get cerebrasProvider => 'Cerebras';

  @override
  String get openRouterProvider => 'OpenRouter';

  @override
  String get mistralProvider => 'Mistral';

  @override
  String get geminiApiDescription =>
      'Fügen Sie einen Google-AI-Studio-API-Schlüssel hinzu, um die für Ihr Konto verfügbaren Modelle zu verwenden. Der kostenlose Zugriff hängt vom Modell und Ihrem Kontingent ab.';

  @override
  String get groqApiDescription =>
      'Verwenden Sie die für Ihr Konto aktivierten Modelle mit einem Groq-API-Schlüssel. Preise und Nutzungslimits unterscheiden sich je nach Tarif und Modell.';

  @override
  String get cerebrasApiDescription =>
      'Verwenden Sie die für Ihr Konto verfügbaren Modelle mit Tool-Unterstützung und einem Cerebras-API-Schlüssel. Zugriff, Preise und Limits unterscheiden sich je nach Modell und Konto.';

  @override
  String get openRouterApiDescription =>
      'Hier werden nur Modelle mit 0 \$ für Ein- und Ausgabe angezeigt, die Text und Tools unterstützen. Tatsächlicher Zugriff und Limits können sich ändern.';

  @override
  String get mistralApiDescription =>
      'Fügen Sie einen Mistral-API-Schlüssel hinzu, um die für Ihr Konto verfügbaren Chatmodelle zu verwenden. Kostenloser Zugriff, Preise und Nutzungslimits hängen von Ihrem Mistral-Tarif ab.';

  @override
  String get geminiUnpaidDataNotice =>
      'Im kostenlosen Kontingent der Gemini API kann Google eingereichte Inhalte zur Verbesserung seiner Produkte verwenden. Lesen Sie Googles Bedingungen zur Datennutzung, bevor Sie vertrauliche Informationen senden.';

  @override
  String get providerApiKey => 'API-Schlüssel';

  @override
  String get providerKeySaved =>
      'Der API-Schlüssel ist sicher auf diesem Gerät gespeichert.';

  @override
  String providerKeySavedSuffix(Object suffix) {
    return 'Der API-Schlüssel mit der Endung ••••$suffix ist sicher auf diesem Gerät gespeichert.';
  }

  @override
  String get providerNoKey => 'Kein API-Schlüssel verbunden.';

  @override
  String get providerKeyInvalid =>
      'Geben Sie einen gültigen API-Schlüssel für diesen Anbieter ein. Keine Leerzeichen verwenden; der Schlüssel darf höchstens 4096 Zeichen enthalten.';

  @override
  String get providerKeyStorageFailed =>
      'Der API-Schlüssel konnte nicht sicher gelesen oder gespeichert werden.';

  @override
  String get favoriteModels => 'Favoriten';

  @override
  String get favoriteModelsEmpty => 'Noch keine bevorzugten Modelle.';

  @override
  String get modelSearchHint => 'Modelle suchen …';

  @override
  String get modelSearchNoResults => 'Keine Modelle passen zu Ihrer Suche.';

  @override
  String get addModelFavorite => 'Zu bevorzugten Modellen hinzufügen';

  @override
  String get removeModelFavorite => 'Aus Favoriten entfernen';

  @override
  String get openCodeConsole => 'OpenCode Console';

  @override
  String get openCodeConsoleDescription =>
      'Kostenlose Modelle funktionieren ohne Schlüssel. Fügen Sie für kostenpflichtige Modelle einen Console-API-Schlüssel hinzu; jede Anfrage wird Ihrem Console-Guthaben belastet.';

  @override
  String openCodeKeySaved(Object suffix) {
    return 'Der Console-API-Schlüssel mit der Endung ••••$suffix ist sicher auf diesem Gerät gespeichert.';
  }

  @override
  String get openCodeNoKey =>
      'Kein Console-API-Schlüssel. Kostenlose Modelle sind verfügbar.';

  @override
  String get openCodeApiKey => 'OpenCode-Console-API-Schlüssel';

  @override
  String get openCodeKeyInvalid =>
      'Geben Sie einen nicht leeren API-Schlüssel ohne Leerzeichen oder Zeilenumbrüche ein (höchstens 4096 Zeichen).';

  @override
  String get openCodeKeyStorageFailed =>
      'Der Console-API-Schlüssel konnte nicht sicher gelesen oder gespeichert werden.';

  @override
  String get openCodePaidModel => 'Kostenpflichtig';

  @override
  String get openCodeFreeModel => 'Kostenlos';

  @override
  String get modelSourceApi => 'API';

  @override
  String get modelSourceOAuth => 'OAuth';

  @override
  String get openCodeFreeModels => 'Kostenlose Modelle';

  @override
  String get openCodeApiModels => 'API-Modelle';

  @override
  String modelContextWindow(String value) {
    return 'Kontextfenster · $value Tokens';
  }

  @override
  String openCodeModelContextWindow(String value) {
    return 'OpenCode-Katalog (Models.dev) · Kontext: $value Tokens';
  }

  @override
  String get add => 'Hinzufügen';

  @override
  String get edit => 'Bearbeiten';

  @override
  String get exportConversation => 'Unterhaltung exportieren';

  @override
  String get deleteConversation => 'Unterhaltung löschen';

  @override
  String get confirmDeleteConversationTitle => 'Diese Unterhaltung löschen?';

  @override
  String confirmDeleteConversation(String title) {
    return '„$title“ und alle zugehörigen Nachrichten werden dauerhaft von diesem Gerät gelöscht.';
  }

  @override
  String get stopResponseBeforeDelete =>
      'Beenden Sie die aktive Antwort, bevor Sie diese Unterhaltung löschen.';

  @override
  String get conversationDeleted => 'Unterhaltung gelöscht.';

  @override
  String get conversationDeleteFailed =>
      'Die Unterhaltung konnte nicht gelöscht werden. Versuchen Sie es erneut.';

  @override
  String get conversationExported =>
      'Unterhaltung als Markdown-Datei exportiert.';

  @override
  String get conversationExportFailed =>
      'Die Unterhaltung konnte nicht exportiert werden. Versuchen Sie es erneut.';

  @override
  String get conversationExportProvider => 'Anbieter';

  @override
  String get conversationExportModel => 'Modell';

  @override
  String get conversationExportCreated => 'Erstellt';

  @override
  String get conversationExportStatus => 'Status';

  @override
  String get toolPermissions => 'Toolzugriff';

  @override
  String get toolPermissionsDescription =>
      'Legen Sie fest, wo die Dateitools der KI arbeiten dürfen und ob jeder Aufruf Ihre Zustimmung erfordert.';

  @override
  String get toolPermissionRequireApproval => 'Zustimmung anfordern';

  @override
  String get selectedModelDoesNotSupportToolCalls =>
      'Dieses Modell unterstützt keine Tool-Aufrufe. Die Tool-Zugriffseinstellungen gelten dafür nicht.';

  @override
  String get selectedModelToolSupportUnknown =>
      'Dieses Modell meldet keine Unterstützung für Tool-Aufrufe. OpenChat versucht, Tools zu senden; der Anbieter kann die Anfrage ablehnen.';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Dateitools fragen vor jedem Aufruf nach und sind auf den Projektordner und %LOCALAPPDATA%\\OpenChat beschränkt. Die Befehlsausführung ist nicht verfügbar.';

  @override
  String get toolPermissionFullAccess => 'Vollzugriff';

  @override
  String get toolPermissionFullAccessDescription =>
      'Dateitools können Dateien in jedem Ordner lesen und ändern, ohne nachzufragen. Die Befehlsausführung ist nicht verfügbar.';

  @override
  String get toolPermissionSettingsLoadFailed =>
      'Die Einstellungen für den Toolzugriff konnten nicht geladen werden.';

  @override
  String get toolPermissionSettingsSaveFailed =>
      'Die Einstellungen für den Toolzugriff konnten nicht gespeichert werden. Versuchen Sie es erneut.';

  @override
  String get toolPermissionRequestTitle => 'Toolberechtigung';

  @override
  String get toolPermissionRequestDescription =>
      'Die KI möchte dieses Tool am ausgewählten Speicherort verwenden. Die Berechtigung gilt nur für diesen Aufruf.';

  @override
  String get toolPermissionRequestExpired =>
      'Diese Toolberechtigungsanfrage ist nicht mehr aktiv.';

  @override
  String get toolPermissionResponseFailed =>
      'Ihre Auswahl konnte nicht gesendet werden. Versuchen Sie es erneut.';

  @override
  String get toolPermissionContent => 'Zu schreibender Inhalt';

  @override
  String get toolPermissionOldText => 'Zu suchender Text';

  @override
  String get toolPermissionNewText => 'Ersatztext';

  @override
  String get toolPermissionQuery => 'Suchtext';

  @override
  String get toolPermissionOffset => 'Startposition';

  @override
  String get toolPermissionLimit => 'Maximale Ergebnisse';

  @override
  String get toolPermissionStartLine => 'Startzeile';

  @override
  String get toolPermissionLineCount => 'Anzahl der Zeilen';

  @override
  String get toolPermissionIncludeHidden => 'Versteckte Dateien einbeziehen';

  @override
  String get commonYes => 'Ja';

  @override
  String get commonNo => 'Nein';

  @override
  String get toolPermissionTarget => 'Zugriffsort';

  @override
  String get toolPermissionTool => 'Tool';

  @override
  String get toolPermissionArguments => 'Anfragedetails';

  @override
  String get toolPermissionDeny => 'Ablehnen';

  @override
  String get toolPermissionStopResponse => 'Antwort stoppen';

  @override
  String get toolPermissionAllowOnce => 'Diesen Aufruf zulassen';

  @override
  String get toolDenied => 'Abgelehnt';

  @override
  String get toolCancelled => 'Abgebrochen';

  @override
  String get toolAwaitingApproval => 'Wartet auf Zustimmung';

  @override
  String get conversationExportToolActivity => 'Toolaktivität';

  @override
  String get responseReplaceFailed =>
      'Die neue Antwort wurde gespeichert, aber die vorherige Antwort konnte nicht ersetzt werden.';

  @override
  String get responseRetryNotCompleted =>
      'Die neue Antwort wurde nicht abgeschlossen. Die vorherige Antwort blieb erhalten.';

  @override
  String get responseRetryCleanupFailed =>
      'Der erneute Versuch konnte nicht bereinigt werden. Aktualisieren Sie den Unterhaltungsverlauf.';

  @override
  String get responseInProgress => 'Antwort wird erstellt';

  @override
  String get responseRetryUnavailable =>
      'Diese Antwort kann nicht erneut versucht werden. Senden Sie stattdessen eine neue Nachricht.';

  @override
  String get providerRateLimited =>
      'Der Anbieter meldet ein Nutzungslimit. Versuchen Sie es später erneut.';

  @override
  String get providerAuthenticationRequired =>
      'Der Anbieter hat die Anfrage abgelehnt. Prüfen Sie die Verbindung und den Modellzugriff.';

  @override
  String get providerRequestFailed =>
      'Der Anbieter konnte die Antwort nicht erstellen. Ihre gespeicherten Nachrichten sind weiterhin verfügbar.';

  @override
  String get providerToolRequestRejected =>
      'Der Anbieter hat eine Anfrage mit Tools abgelehnt. Prüfen Sie die Tool-Unterstützung des Modells oder wählen Sie ein Modell mit ausgewiesener Tool-Unterstützung.';

  @override
  String get providerNetworkUnavailable =>
      'Der Anbieter ist nicht erreichbar. Prüfen Sie Ihre Verbindung und versuchen Sie es erneut.';

  @override
  String get contextWindowExceeded =>
      'Die Unterhaltung ist zu groß für das Kontextfenster dieses Modells. Kürzen Sie die letzte Nachricht oder wählen Sie ein Modell mit einem größeren Kontextfenster. Ihr Chatverlauf ist gespeichert.';

  @override
  String get openCodeFreeTierRestricted =>
      'Kostenlose OpenCode-Modelle sind nur innerhalb der OpenCode-App verfügbar.';

  @override
  String get apiKey => 'API-Schlüssel';

  @override
  String get apiKeyInputLabel => 'API-Schlüssel';

  @override
  String get apiKeyRequired => 'Geben Sie einen API-Schlüssel ein.';

  @override
  String get apiKeyInvalidFormat =>
      'Geben Sie einen gültigen OpenAI-API-Schlüssel ein. Er muss mit sk- beginnen.';

  @override
  String get apiKeyAlreadySaved =>
      'Dieser API-Schlüssel ist bereits gespeichert.';

  @override
  String get apiKeySaved =>
      'API-Schlüssel sicher auf diesem Gerät gespeichert.';

  @override
  String get apiKeySaveFailed =>
      'Der API-Schlüssel konnte nicht sicher gespeichert werden. Versuchen Sie es erneut.';

  @override
  String get apiKeyLoadFailed =>
      'Gespeicherte API-Schlüssel konnten nicht geladen werden.';

  @override
  String get savedApiKey => 'Gespeicherter Schlüssel';

  @override
  String savedApiKeyWithSuffix(String suffix) {
    return 'Schlüssel mit der Endung ••••$suffix';
  }

  @override
  String get showApiKey => 'API-Schlüssel anzeigen';

  @override
  String get hideApiKey => 'API-Schlüssel ausblenden';

  @override
  String get retry => 'Erneut versuchen';

  @override
  String get save => 'Speichern';

  @override
  String get saving => 'Wird gespeichert …';

  @override
  String get oauth => 'OAuth';

  @override
  String get oauthSigningIn => 'Anmeldung läuft …';

  @override
  String get oauthBrowserWaiting =>
      'Schließen Sie die Anmeldung im Browser ab.';

  @override
  String get oauthConnectionsLoadFailed =>
      'ChatGPT-OAuth-Verbindungen konnten nicht geladen werden.';

  @override
  String get oauthSignInFailed =>
      'Die ChatGPT-Anmeldung konnte nicht abgeschlossen werden. Prüfen Sie den Browser und versuchen Sie es erneut.';

  @override
  String get oauthConnectionAdded => 'ChatGPT-Konto verbunden.';

  @override
  String get oldCredentialCleanupFailed =>
      'Das Konto wurde verbunden, aber ein älterer gespeicherter Zugang konnte nicht entfernt werden. Starten Sie OpenChat neu und versuchen Sie es erneut.';

  @override
  String get connectionSelectionFailed =>
      'Das ChatGPT-Konto konnte nicht ausgewählt werden.';

  @override
  String get removeChatGptConnection => 'ChatGPT-Verbindung entfernen';

  @override
  String get removeConnectionAction => 'Entfernen';

  @override
  String confirmRemoveConnection(String name) {
    return '$name und die gespeicherten Zugangsdaten entfernen? Vorhandene Chats und Nachrichten bleiben auf diesem Gerät erhalten.';
  }

  @override
  String get connectionRemoveSucceeded =>
      'Verbindung entfernt. Vorhandene Chats und Nachrichten sind weiterhin verfügbar.';

  @override
  String get connectionRemoveFailed =>
      'Die Verbindung konnte nicht entfernt werden. Versuchen Sie es erneut.';

  @override
  String get workspaceSelectionFailed =>
      'Der ChatGPT-Arbeitsbereich konnte nicht ausgewählt werden.';

  @override
  String get chatGptAccount => 'ChatGPT-Konto';

  @override
  String get planUnavailable => 'Tarif nicht verfügbar';

  @override
  String accountPlan(String plan) {
    return 'Tarif: $plan';
  }

  @override
  String get connectionNeedsSignIn =>
      'Melden Sie sich erneut an, um dieses Konto zu verwenden.';

  @override
  String get connectionSelected => 'Ausgewählt';

  @override
  String get useConnection => 'Konto verwenden';

  @override
  String get selectWorkspace => 'Arbeitsbereich auswählen';

  @override
  String get workspaceWithoutName => 'Arbeitsbereich';

  @override
  String get workspace => 'Arbeitsbereich';

  @override
  String get workspaceUnavailable =>
      'Keine Informationen zum Arbeitsbereich verfügbar.';

  @override
  String get selectAccountForWorkspace =>
      'Wählen Sie dieses Konto aus, um seinen Arbeitsbereich auszuwählen.';

  @override
  String get accountEmailUnavailable => 'E-Mail-Adresse nicht verfügbar';

  @override
  String accountUsage(String plan) {
    return 'Nutzung · $plan';
  }

  @override
  String get refreshUsage => 'Nutzung aktualisieren';

  @override
  String get ordinaryUsageAvailable => 'Die reguläre Nutzung ist verfügbar.';

  @override
  String get ordinaryUsageUnavailable =>
      'Die reguläre Nutzung ist derzeit nicht verfügbar.';

  @override
  String get ordinaryUsageUnknown =>
      'Die Verfügbarkeit der Nutzung konnte nicht ermittelt werden.';

  @override
  String usageUpdatedAt(String time) {
    return 'Aktualisiert $time';
  }

  @override
  String get usageLoadFailed =>
      'Nutzungsinformationen konnten nicht geladen werden.';

  @override
  String get usageFiveHour => '5 Stunden';

  @override
  String get usageWeekly => 'Wöchentlich';

  @override
  String get usageMonthly => 'Monatlich';

  @override
  String workspaceNumbered(int number) {
    return 'Arbeitsbereich $number';
  }

  @override
  String quotaResetsAt(String time) {
    return 'Wird zurückgesetzt $time';
  }

  @override
  String creditExpiresAt(String time) {
    return 'Läuft ab $time';
  }

  @override
  String creditGrantedAt(String time) {
    return 'Gewährt $time';
  }

  @override
  String get noResetCredits => 'Keine Reset-Guthaben verfügbar.';

  @override
  String usageUsedPercent(String percent) {
    return '$percent% verwendet';
  }

  @override
  String get resetCreditCountUnavailable =>
      'Die Anzahl der Reset-Guthaben ist nicht verfügbar.';

  @override
  String resetCreditsAvailable(int count) {
    return 'Verfügbare Reset-Guthaben: $count';
  }

  @override
  String get resetCredit => 'Reset-Guthaben';

  @override
  String get statusUnavailable => 'Status nicht verfügbar';

  @override
  String get resetCreditDetailsUnavailable =>
      'Details zum Reset-Guthaben wurden nicht zurückgegeben.';

  @override
  String get resetCreditAvailableStatus => 'Verfügbar';

  @override
  String get useResetCredit => 'Guthaben verwenden';

  @override
  String get resetCreditRedeeming => 'Wird verwendet …';

  @override
  String get confirmResetCreditTitle => 'Dieses Reset-Guthaben verwenden?';

  @override
  String get confirmResetCreditMessage =>
      'Ein Reset-Guthaben wird eingelöst. Dieser Vorgang kann nicht rückgängig gemacht werden. Fortfahren?';

  @override
  String get confirmResetCreditAction => 'Guthaben verwenden';

  @override
  String get resetCreditApplied => 'Das Nutzungslimit wurde zurückgesetzt.';

  @override
  String get resetCreditAlreadyUsed =>
      'Dieses Reset-Guthaben wurde bereits verwendet.';

  @override
  String get resetCreditNothingToReset =>
      'Derzeit gibt es kein Nutzungslimit zum Zurücksetzen.';

  @override
  String get resetCreditNoLongerAvailable =>
      'Dieses Reset-Guthaben ist nicht mehr verfügbar. Aktualisieren Sie die Nutzungsinformationen.';

  @override
  String get resetCreditOutcomeUnknown =>
      'Das Ergebnis konnte nicht bestätigt werden. Aktualisieren Sie die Nutzungsinformationen, bevor Sie dieses Guthaben erneut verwenden.';

  @override
  String get resetCreditRefreshRequired =>
      'Das vorherige Ergebnis konnte nicht bestätigt werden. Aktualisieren Sie die Nutzungsinformationen, bevor Sie es erneut versuchen.';

  @override
  String get resetCreditRejected =>
      'ChatGPT hat die Rücksetzanfrage für dieses Konto abgelehnt.';

  @override
  String get resetCreditSignInRequired =>
      'Melden Sie sich erneut bei Ihrem ChatGPT-Konto an und versuchen Sie es dann erneut.';

  @override
  String get noChatGptConnections => 'Noch keine ChatGPT-Verbindungen.';

  @override
  String get apiKeyConnectionUnavailable =>
      'Es wurde noch keine API-Schlüsselverbindung hinzugefügt.';

  @override
  String get oauthConnectionUnavailable =>
      'Es wurde noch keine OAuth-Verbindung hinzugefügt.';

  @override
  String get titleGenerationTarget => 'Automatische Chattitel';

  @override
  String get titleGenerationTargetDescription =>
      'Wenn für das Konto der Unterhaltung kein anderes Modell verfügbar ist, kann OpenChat das hier ausgewählte Konto verwenden. Die Titelerstellung wird übersprungen, wenn die reguläre Nutzung nicht verfügbar ist.';

  @override
  String get titleUseConversationAccount => 'Konto der Unterhaltung verwenden';

  @override
  String get titleAccountUnavailable =>
      'Das ausgewählte Titelkonto ist nicht verfügbar';

  @override
  String get titleWorkspaceHint => 'Arbeitsbereich für Titel auswählen';

  @override
  String get titleWorkspaceRequired =>
      'Wählen Sie einen Arbeitsbereich aus, bevor dieses Konto Titel erstellen kann.';

  @override
  String get titlePreferenceLoadFailed =>
      'Die Einstellung für das Titelkonto konnte nicht geladen werden.';

  @override
  String get titlePreferenceSaveFailed =>
      'Die Einstellung für das Titelkonto konnte nicht gespeichert werden.';

  @override
  String get appearance => 'Darstellung';

  @override
  String get themeSettingDescription =>
      'Wählen Sie das Erscheinungsbild der App.';

  @override
  String get conversationWidth => 'Antwortbreite';

  @override
  String get conversationWidthDescription =>
      'Wählen Sie die Zeilenbreite für Chatantworten.';

  @override
  String get widthNarrow => 'Schmal';

  @override
  String get widthNormal => 'Normal';

  @override
  String get widthWide => 'Breit';

  @override
  String get conversationTextSize => 'Textgröße';

  @override
  String get conversationTextSizeDescription =>
      'Wählen Sie die in der App verwendete Textgröße.';

  @override
  String get textSizeSmall => 'Klein';

  @override
  String get textSizeNormal => 'Normal';

  @override
  String get textSizeLarge => 'Groß';

  @override
  String get appFont => 'App-Schrift';

  @override
  String get appFontDescription =>
      'Wählen Sie die in OpenChat verwendete Schriftart.';

  @override
  String get appearancePreferenceSaveFailed =>
      'Die Darstellungseinstellung konnte nicht gespeichert werden. Versuchen Sie es erneut.';

  @override
  String get language => 'App-Sprache';

  @override
  String get languageSettingDescription => 'Wählen Sie die Sprache der App.';

  @override
  String get systemLanguage => 'Gerätesprache';

  @override
  String get englishLanguage => 'Englisch';

  @override
  String get turkishLanguage => 'Türkisch';

  @override
  String get spanishLanguage => 'Spanisch';

  @override
  String get germanLanguage => 'Deutsch';

  @override
  String get frenchLanguage => 'Französisch';

  @override
  String get languageSaveFailed =>
      'Die Spracheinstellung konnte nicht gespeichert werden. Versuchen Sie es erneut.';

  @override
  String get systemTheme => 'System';

  @override
  String get lightTheme => 'Hell';

  @override
  String get darkTheme => 'Dunkel';

  @override
  String get localData => 'Lokale Daten';

  @override
  String get conversationHistory => 'Chatverlauf';

  @override
  String get historyDeviceDescription =>
      'Unterhaltungen werden auf diesem Gerät gespeichert.';

  @override
  String get historyDeviceStatus => 'Auf diesem Gerät';

  @override
  String get historyCheckingDescription =>
      'Lokaler Chatverlauf wird vorbereitet.';

  @override
  String get historyCheckingStatus => 'Wird vorbereitet';

  @override
  String get historyStorageUnavailableDescription =>
      'Der lokale Chatverlauf konnte nicht geöffnet werden.';

  @override
  String get historyStorageCorruptDescription =>
      'Die lokale Datenbank ist beschädigt. Es wurden keine Schemaänderungen vorgenommen. Stelle ein geprüftes Backup wieder her, um fortzufahren.';

  @override
  String get historyStorageBackupFailedDescription =>
      'OpenChat konnte kein geprüftes Datenbank-Backup vor dem Update erstellen und hat das Update abgebrochen. Prüfe den freien Speicherplatz und versuche es erneut.';

  @override
  String get historyStorageUnavailableStatus => 'Nicht verfügbar';

  @override
  String get historyLoading => 'Unterhaltungen werden geladen …';

  @override
  String get historyLoadFailed =>
      'Der Chatverlauf konnte nicht geladen werden. Starten Sie die App neu.';

  @override
  String get messageHistoryLoadFailed =>
      'Nachrichten für diese Unterhaltung konnten nicht geladen werden.';

  @override
  String get messageHistoryLoading =>
      'Nachrichten der Unterhaltung werden geladen.';

  @override
  String get clearConversationHistory => 'Gesamten Verlauf löschen';

  @override
  String get clearConversationHistoryDescription =>
      'Auf diesem Gerät gespeicherte Chats und Nachrichten dauerhaft löschen.';

  @override
  String get confirmClearHistoryTitle => 'Gesamten Chatverlauf löschen?';

  @override
  String get confirmClearHistoryBody =>
      'Dadurch werden alle auf diesem Gerät gespeicherten Chats und Nachrichten dauerhaft gelöscht. Dieser Vorgang kann nicht rückgängig gemacht werden.';

  @override
  String get cancel => 'Abbrechen';

  @override
  String get deleteAll => 'Alle löschen';

  @override
  String get clearingHistory => 'Wird gelöscht …';

  @override
  String get clearHistorySucceeded => 'Chatverlauf gelöscht.';

  @override
  String get clearHistoryFailed =>
      'Der Chatverlauf konnte nicht gelöscht werden. Versuchen Sie es erneut.';

  @override
  String get themeSaveFailed =>
      'Die Theme-Einstellung konnte nicht gespeichert werden. Versuchen Sie es erneut.';

  @override
  String get searchChats => 'Chats suchen';

  @override
  String get searchChatsHint => 'Ihre Chats durchsuchen';

  @override
  String get projects => 'Projekte';

  @override
  String get noProjects => 'Noch keine Projekte';

  @override
  String get createProject => 'Projekt erstellen';

  @override
  String get projectName => 'Projektname';

  @override
  String get projectNameRequired => 'Geben Sie einen Projektnamen ein.';

  @override
  String get projectFolder => 'Projektordner';

  @override
  String get chooseProjectFolder => 'Ordner auswählen';

  @override
  String get projectFolderNotSelected =>
      'Wählen Sie einen Ordner aus, um fortzufahren.';

  @override
  String get projectFolderSelectionFailed =>
      'Der Ordner konnte nicht ausgewählt werden.';

  @override
  String get projectCreateFailed => 'Das Projekt konnte nicht erstellt werden.';

  @override
  String get projectCreated => 'Projekt erstellt.';

  @override
  String get projectLoadFailed => 'Projekte konnten nicht geladen werden.';

  @override
  String get projectMoveFailed =>
      'Der Chat konnte nicht in das Projekt verschoben werden.';

  @override
  String get pinnedChats => 'Angeheftet';

  @override
  String get noPinnedChats => 'Noch keine angehefteten Chats';

  @override
  String get noChatsTitle => 'Noch keine Chats';

  @override
  String get noChatsSearchTitle => 'Keine passenden Chats';

  @override
  String get showMore => 'Mehr anzeigen';

  @override
  String get projectOptions => 'Projektoptionen';

  @override
  String get newProjectConversation => 'Neuen Projektchat starten';

  @override
  String get newConversation => 'Neue Unterhaltung starten';

  @override
  String get conversationTitle => 'Neuer Chat';

  @override
  String get conversationMemory => 'Unterhaltungsspeicher';

  @override
  String get conversationMemoryDescription =>
      'Überprüfen Sie den komprimierten Kontext dieser Unterhaltung und durchsuchen Sie ältere Nachrichten.';

  @override
  String conversationMemoryCurrentConversation(String title) {
    return 'Ausgewählte Unterhaltung: $title';
  }

  @override
  String get conversationMemoryNoConversation =>
      'Öffnen Sie eine Unterhaltung, um ihren Speicher zu prüfen.';

  @override
  String get contextUsageTitle => 'Kontextnutzung';

  @override
  String contextUsageUsed(String count) {
    return 'Verwendung: ungefähr $count Tokens';
  }

  @override
  String contextUsageSummary(String used, String limit, String percent) {
    return '~$used / $limit Tokens ($percent)';
  }

  @override
  String contextUsageModelLimit(String count) {
    return '$count Tokens';
  }

  @override
  String get contextUsageNoModelLimit => 'Modelllimit unbekannt.';

  @override
  String contextUsageProviderMeasurement(String count) {
    return 'Letzte Messung des Anbieters: $count Tokens';
  }

  @override
  String contextUsageInstructionsEstimate(String count, String percent) {
    return 'Anweisungen: $count Tokens · $percent';
  }

  @override
  String contextUsageToolDefinitionsEstimate(String count, String percent) {
    return 'Tooldefinitionen: $count Tokens · $percent';
  }

  @override
  String contextUsageMessagesEstimate(String count, String percent) {
    return 'Nachrichten: $count Tokens · $percent';
  }

  @override
  String contextUsageAttachmentsEstimate(String count, String percent) {
    return 'Anhänge: $count Tokens · $percent';
  }

  @override
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count Tokens · $percent';
  }

  @override
  String contextUsageDraftAttachment(String name) {
    return 'Entwurf · $name';
  }

  @override
  String contextUsageUserMessagesEstimate(String count, String percent) {
    return 'Benutzer: $count Tokens · $percent';
  }

  @override
  String contextUsageAssistantMessagesEstimate(String count, String percent) {
    return 'Assistent: $count Tokens · $percent';
  }

  @override
  String contextUsageToolsEstimate(String count, String percent) {
    return 'Toolverwendung: $count Tokens · $percent';
  }

  @override
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count Tokens · $percent';
  }

  @override
  String contextUsageMemoryEstimate(String count, String percent) {
    return 'Komprimierter Speicher: $count Tokens · $percent';
  }

  @override
  String contextUsageDraftEstimate(String count, String percent) {
    return 'Entwurf: $count Tokens · $percent';
  }

  @override
  String contextUsageFreeSpaceEstimate(String count, String percent) {
    return 'Freier Speicher: $count Tokens · $percent';
  }

  @override
  String get contextUsageOverLimit => 'Modelllimit überschritten.';

  @override
  String get contextUsageMeasurementUnavailable =>
      'Speicherdetails konnten nicht geladen werden.';

  @override
  String get contextUsageInstructionUnavailable =>
      'Details zu den Anweisungen konnten nicht geladen werden.';

  @override
  String get contextUsageConfigurationUnavailable =>
      'Schätzungen für Anweisungen und Tooldefinitionen konnten nicht geladen werden.';

  @override
  String get contextUsageConfigurationLoading =>
      'Schätzungen für Anweisungen und Tooldefinitionen werden vorbereitet …';

  @override
  String get conversationMemorySemanticTitle => 'Semantische Suche';

  @override
  String get conversationMemorySemanticDescription =>
      'Laden Sie ein mehrsprachiges Modell mit etwa 136 MB herunter, um ältere Nachrichten mit anderer Formulierung zu finden.';

  @override
  String get conversationMemorySemanticPrepare => 'Vorbereiten';

  @override
  String get conversationMemorySemanticChecking =>
      'Lokale semantische Suche wird geprüft …';

  @override
  String get conversationMemorySemanticPreparing =>
      'Modell wird heruntergeladen und verifiziert …';

  @override
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  ) {
    return '$percent% heruntergeladen · $downloaded / $total MB';
  }

  @override
  String get conversationMemorySemanticIndexing =>
      'Lokaler Archivindex wird erstellt …';

  @override
  String get conversationMemorySemanticCancelling =>
      'Download wird abgebrochen …';

  @override
  String get conversationMemorySemanticDownloadCancelled =>
      'Download abgebrochen. Die Stichwortsuche bleibt verfügbar.';

  @override
  String get conversationMemorySemanticPrepareFailed =>
      'Das Modell konnte nicht vorbereitet werden. Versuchen Sie es erneut; gültige heruntergeladene Teile werden wiederverwendet.';

  @override
  String get conversationMemorySemanticKeywordSearchFallback =>
      'Die Stichwortsuche bleibt vor der Vorbereitung verfügbar.';

  @override
  String get conversationMemorySemanticReady =>
      'Lokale semantische Suche ist bereit';

  @override
  String get conversationMemorySemanticIndexNotice =>
      'Das Modell läuft auf diesem Gerät. Die erste Suche kann ältere Nachrichten und gespeicherte Tooldetails lokal indizieren und länger dauern.';

  @override
  String get conversationMemorySummaryTitle => 'Komprimierter Kontext';

  @override
  String get conversationMemoryNoSummary =>
      'Noch keine komprimierte Zusammenfassung vorhanden.';

  @override
  String get conversationMemoryCheckpointDescription =>
      'Der Anbieter speichert den Kontext als wiederverwendbaren Checkpoint statt als lesbaren Zusammenfassungstext. Der vollständige Nachrichtenverlauf bleibt im Archiv.';

  @override
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  ) {
    return 'Letzte Anfrage · $provider · $model · $count Eingabetokens';
  }

  @override
  String get conversationMemorySearchTitle => 'Archiv durchsuchen';

  @override
  String get conversationMemorySearchHint =>
      'Älteres Thema oder eine Formulierung eingeben …';

  @override
  String get conversationMemorySearchAction => 'Suchen';

  @override
  String get conversationMemorySearchQueryTooShort =>
      'Geben Sie mindestens zwei Zeichen für die Suche ein.';

  @override
  String get conversationMemorySearchInstruction =>
      'Abgeschlossene Nachrichten und Toolergebnisse werden nur in dieser Unterhaltung durchsucht.';

  @override
  String get conversationMemorySearchNoResults =>
      'Keine passenden Archiveinträge.';

  @override
  String get conversationMemorySearchFailed =>
      'Das Unterhaltungsarchiv konnte nicht durchsucht werden. Versuchen Sie es erneut.';

  @override
  String get conversationMemoryLoadFailed =>
      'Der komprimierte Kontext konnte nicht geladen werden. Versuchen Sie es erneut.';

  @override
  String get conversationMemoryResetAction => 'Kontext zurücksetzen';

  @override
  String get conversationMemoryResetTitle =>
      'Komprimierten Kontext zurücksetzen?';

  @override
  String get conversationMemoryResetConfirmation =>
      'Der gespeicherte komprimierte Kontext und die Messung der letzten Anfrage werden entfernt. Der vollständige Nachrichtenverlauf und das Archiv bleiben erhalten; bei einer späteren Anfrage kann wieder ein komprimierter Kontext erstellt werden.';

  @override
  String get conversationMemoryResetConfirm => 'Zurücksetzen';

  @override
  String get conversationMemoryResetFailed =>
      'Der komprimierte Kontext konnte nicht zurückgesetzt werden.';

  @override
  String get conversationMemoryUserMessage => 'Benutzernachricht';

  @override
  String get conversationMemoryAssistantMessage => 'Assistentenantwort';

  @override
  String get renameConversation => 'Chat-Titel bearbeiten';

  @override
  String get pinConversation => 'Chat anheften';

  @override
  String get unpinConversation => 'Chat loslösen';

  @override
  String get conversationTitleRequired =>
      'Der Chat-Titel darf nicht leer sein.';

  @override
  String get conversationTitleSaveFailed =>
      'Der Chat-Titel konnte nicht gespeichert werden.';

  @override
  String get conversationModelSaveFailed =>
      'Das Chatmodell konnte nicht gespeichert werden. Das vorherige Modell ist weiterhin ausgewählt.';

  @override
  String get moreOptions => 'Weitere Optionen';

  @override
  String get emptyChatWelcomeTitle => 'Wie kann ich helfen?';

  @override
  String get emptyChatWelcomeBody =>
      'Stellen Sie eine Frage, um den Chat zu starten.';

  @override
  String get switchToDarkMode => 'Zum dunklen Theme wechseln';

  @override
  String get switchToLightMode => 'Zum hellen Theme wechseln';

  @override
  String get theme => 'Theme';

  @override
  String get keyboardHint =>
      'Enter zum Senden · Shift+Enter für eine neue Zeile';

  @override
  String get minimizeWindow => 'Fenster minimieren';

  @override
  String get maximizeWindow => 'Fenster maximieren';

  @override
  String get restoreWindow => 'Fenster wiederherstellen';

  @override
  String get modelSelection => 'Modell auswählen';

  @override
  String get noModelConnected => 'Noch kein Modell verbunden';

  @override
  String get noModelConnectedBody =>
      'Verbinden Sie einen Anbieter und wählen Sie eines seiner Modelle aus, um eine Unterhaltung zu starten.';

  @override
  String get close => 'Schließen';

  @override
  String get reasoning => 'Denken';

  @override
  String get reasoningMedium => 'Mittel';

  @override
  String get reasoningMinimal => 'Minimal';

  @override
  String get reasoningLow => 'Niedrig';

  @override
  String get reasoningHigh => 'Hoch';

  @override
  String get reasoningExtraHigh => 'Sehr hoch';

  @override
  String get reasoningMax => 'Maximum';

  @override
  String get reasoningUltra => 'Ultra';

  @override
  String get reasoningDefault => 'Standard';

  @override
  String get reasoningDefaultHint =>
      'Es wird keine eigene Denkstufe gesendet. Das Standardverhalten des Anbieters wird verwendet; die Stufe wird nicht an den Schwierigkeitsgrad der Aufgabe angepasst.';

  @override
  String get stop => 'Stoppen';

  @override
  String get modelsLoading => 'Modelle werden geladen …';

  @override
  String get modelsUnavailable => 'Modelle nicht verfügbar';

  @override
  String get noModelsAvailable =>
      'Für dieses Konto sind derzeit keine Modelle verfügbar.';

  @override
  String get providerDataUnavailable =>
      'Der Anbieter hat Daten zurückgegeben, die OpenChat nicht lesen konnte. Aktualisieren Sie die Verbindung und versuchen Sie es erneut.';

  @override
  String get oauthResponseInvalid =>
      'Das Anmeldeergebnis konnte nicht gelesen werden. Versuchen Sie erneut, die Verbindung herzustellen.';

  @override
  String get modelCatalogUnavailable =>
      'Die Modellliste ist nicht verfügbar. Aktualisieren Sie die Anbieter-Verbindung und versuchen Sie es erneut.';

  @override
  String get messageSaveFailed =>
      'Ihre Nachricht konnte nicht im lokalen Chatverlauf gespeichert werden.';

  @override
  String get chatHistoryUnavailable =>
      'Der Chatverlauf konnte nicht aktualisiert werden. Versuchen Sie es erneut.';

  @override
  String get chatRequestFailed =>
      'Die Antwort konnte nicht abgeschlossen werden. Ihre gespeicherten Nachrichten sind weiterhin verfügbar.';

  @override
  String get cachedCatalog => 'zwischengespeicherte Modelle';

  @override
  String get attachFile => 'Datei anhängen';

  @override
  String get attachmentsUnavailable =>
      'Dateianhänge sind noch nicht verfügbar.';

  @override
  String get removeAttachment => 'Anhang entfernen';

  @override
  String get previewImage => 'Bild vergrößern';

  @override
  String get attachmentUnavailable => 'Dieser Anhang ist nicht verfügbar.';

  @override
  String get attachmentCountExceeded =>
      'Sie können bis zu 10 Dateien und 3 Bilder anhängen.';

  @override
  String get attachmentFileTooLarge =>
      'Die Datei überschreitet die zulässige Größe.';

  @override
  String get attachmentTotalTooLarge =>
      'Anhänge dürfen insgesamt höchstens 14 MB groß sein.';

  @override
  String get unsupportedAttachmentFile =>
      'Dieser Dateityp wird nicht unterstützt.';

  @override
  String get attachmentReadFailed =>
      'Die Datei konnte nicht gelesen werden. Wählen Sie sie erneut aus und versuchen Sie es wieder.';

  @override
  String get attachmentSaveFailed =>
      'Der Anhang konnte nicht gespeichert werden. Wählen Sie ihn erneut aus und versuchen Sie es wieder.';

  @override
  String get attachmentMustBeUtf8 => 'Textanhänge müssen UTF-8 verwenden.';

  @override
  String get attachmentInvalidImage =>
      'Das Bildformat konnte nicht verifiziert werden.';

  @override
  String get modelDoesNotSupportImages =>
      'Das ausgewählte Modell unterstützt keine Bildanhänge.';

  @override
  String get messageHint => 'Nachricht schreiben …';

  @override
  String get sendMessage => 'Nachricht senden';

  @override
  String get send => 'Senden';

  @override
  String get welcomeTitle => 'Unterhaltung starten';

  @override
  String get welcomeBody =>
      'Verbinden Sie ein KI-Modell, bevor Sie eine Unterhaltung starten.';

  @override
  String get historyOpen => 'Chatverlauf öffnen';

  @override
  String get assistantDisclaimer =>
      'KI-Antworten können ungenau sein. Prüfen Sie wichtige Informationen.';

  @override
  String get modelRequired =>
      'Verbinden Sie ein Modell, bevor Sie eine Nachricht senden.';

  @override
  String get selectedModelUnavailable =>
      'Dieses Modell ist nicht mehr verfügbar. Wählen Sie ein anderes Modell.';

  @override
  String get messageModelUnavailable => 'Modell nicht verfügbar';

  @override
  String get userMessage => 'Benutzer';

  @override
  String get copyMessage => 'Nachricht kopieren';

  @override
  String get messageCopied => 'Nachricht in die Zwischenablage kopiert.';

  @override
  String get messageCopyFailed =>
      'Die Nachricht konnte nicht in die Zwischenablage kopiert werden.';

  @override
  String get today => 'Heute';

  @override
  String get unavailableTime => '—:—';

  @override
  String get unavailableValue => '—';

  @override
  String responseMetadata(String rate, String tokens, String time) {
    return '$rate Tok/s · $tokens Tokens · $time';
  }

  @override
  String get responseCompleted => 'Antwort abgeschlossen';

  @override
  String responseCompletedWithDuration(String duration) {
    return 'Antwort abgeschlossen · $duration';
  }

  @override
  String get reasoningSummary => 'Zusammenfassung des Denkens';

  @override
  String reasoningSummaryWithDuration(String duration) {
    return 'Zusammenfassung des Denkens · $duration';
  }

  @override
  String get reasoningSummaryTooltip =>
      'Eine vom Modell bereitgestellte Zusammenfassung. Die Dauer gibt an, wie lange die Übertragung gedauert hat; es handelt sich nicht um verborgene Gedankengänge.';

  @override
  String get toolRunning => 'Wird ausgeführt';

  @override
  String get toolWaitingForUser => 'Wartet auf deine Antwort';

  @override
  String get toolCompleted => 'Abgeschlossen';

  @override
  String get toolFailed => 'Fehlgeschlagen';

  @override
  String get toolInput => 'Eingabe';

  @override
  String get toolOutput => 'Ausgabe';

  @override
  String get toolListFiles => 'Dateien auflisten';

  @override
  String get toolSearchFiles => 'Dateien durchsuchen';

  @override
  String get toolReadFile => 'Datei lesen';

  @override
  String get toolGetFileInfo => 'Dateiinformationen abrufen';

  @override
  String get toolWriteFile => 'In Datei schreiben';

  @override
  String get toolEditFile => 'Datei bearbeiten';

  @override
  String get toolExecuteCommand => 'Befehl ausführen';

  @override
  String get toolSendTerminalInput => 'Terminaleingabe senden';

  @override
  String get toolWebSearch => 'Websuche';

  @override
  String get toolReadUrlContent => 'Webseite lesen';

  @override
  String get toolSearchQuery => 'Suchanfrage';

  @override
  String get toolWebSearchNoResults => 'Keine Webergebnisse gefunden.';

  @override
  String toolWebSearchResultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Ergebnisse',
      one: '1 Ergebnis',
      zero: '0 Ergebnisse',
    );
    return '$_temp0';
  }

  @override
  String get toolUrl => 'URL';

  @override
  String toolReadUrlLength(int count) {
    return '$count Zeichen';
  }

  @override
  String get toolOpenUrl => 'Im Browser öffnen';

  @override
  String get toolCopyUrl => 'URL kopieren';

  @override
  String get toolCopyContent => 'Inhalt kopieren';

  @override
  String get toolCopyFailed => 'Der Inhalt konnte nicht kopiert werden.';

  @override
  String get toolOperationWorking => 'Dieser Vorgang läuft.';

  @override
  String get toolOperationFailed =>
      'Der Vorgang konnte nicht abgeschlossen werden.';

  @override
  String get toolOperationUnavailable =>
      'Das Ergebnis konnte nicht angezeigt werden.';

  @override
  String get toolOperationTruncated =>
      'Es ist nur ein Teil des Ergebnisses verfügbar.';

  @override
  String get toolSearchNoMatches => 'Keine Treffer gefunden.';

  @override
  String get toolSearchMoreResults => 'Weitere Treffer sind verfügbar.';

  @override
  String toolSearchMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Treffer',
      one: '1 Treffer',
      zero: '0 Treffer',
    );
    return '$_temp0';
  }

  @override
  String get toolReadNoLines => 'In diesem Bereich gibt es keine Zeilen.';

  @override
  String get toolReadMoreLines => 'Weitere Zeilen sind verfügbar.';

  @override
  String toolReadLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Zeilen',
      one: '1 Zeile',
      zero: '0 Zeilen',
    );
    return '$_temp0';
  }

  @override
  String get toolFileTypeFile => 'Datei';

  @override
  String get toolFileTypeDirectory => 'Ordner';

  @override
  String toolWriteSuccess(String size) {
    return '$size geschrieben';
  }

  @override
  String get toolFilePreview => 'Vorschau des geschriebenen Inhalts';

  @override
  String toolEditSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# Änderungen angewendet',
      one: '# Änderung angewendet',
    );
    return '$_temp0';
  }

  @override
  String get toolEditBefore => 'Vorher';

  @override
  String get toolEditAfter => 'Nachher';

  @override
  String get toolPermissionCommand => 'Befehl';

  @override
  String get toolPermissionTerminalId => 'Terminal-ID';

  @override
  String get toolPermissionInput => 'Terminaleingabe';

  @override
  String get toolTerminalNoOutput => 'Keine Ausgabe erzeugt.';

  @override
  String get toolTerminalWaitingOutput => 'Warten auf Ausgabe oder Eingabe …';

  @override
  String get toolTerminalRunning => 'Wird ausgeführt …';

  @override
  String get toolTerminalTerminated => 'Beendet';

  @override
  String get toolTerminalWaitingForInput => 'Warten auf Eingabe';

  @override
  String toolTerminalExitCode(int code) {
    return 'Exit-Code: $code';
  }

  @override
  String get toolTerminalCopied => 'In die Zwischenablage kopiert';

  @override
  String get toolTechnicalDetails => 'Details';

  @override
  String toolFileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Elemente',
      one: '1 Element',
      zero: 'Keine Elemente',
    );
    return '$_temp0';
  }

  @override
  String get toolEmptyListing =>
      'In diesem Ordner gibt es keine Elemente zum Anzeigen.';

  @override
  String get toolListingUnavailable =>
      'Diese Dateiliste konnte nicht angezeigt werden.';

  @override
  String get toolListingIncomplete =>
      'Einige Elemente konnten nicht aufgelistet werden.';

  @override
  String get toolMoreFilesAvailable => 'Weitere Elemente sind verfügbar.';

  @override
  String get toolDesktopLocation => 'Desktop';

  @override
  String get toolProjectLocation => 'Projektordner';

  @override
  String get toolOpenChatLocation => 'Anwendungsdatenordner';

  @override
  String get responseFailed => 'Antwort fehlgeschlagen';

  @override
  String get responseStopped => 'Antwort gestoppt';

  @override
  String secondsShort(int count) {
    return '$count Sek.';
  }

  @override
  String get usageQuotas => 'Nutzungskontingente';

  @override
  String get usageQuotasDescription =>
      'Verbleibende Kontingente, Nutzungslimits und Rücksetzzeiten für alle verbundenen ChatGPT-Konten anzeigen.';

  @override
  String get refreshAll => 'Alle aktualisieren';

  @override
  String get noChatGptAccountsForQuota =>
      'Keine verbundenen ChatGPT-Konten gefunden.';

  @override
  String get noChatGptAccountsForQuotaDescription =>
      'Verbinden Sie Ihr ChatGPT-Konto im Tab „Verbindungen“, um Nutzungslimits und Kontingente anzuzeigen.';

  @override
  String get goToConnections => 'Zu den Verbindungen';

  @override
  String get activeAccountBadge => 'Aktiv';

  @override
  String workspaceQuotaLabel(String name) {
    return 'Arbeitsbereich: $name';
  }

  @override
  String get modelsPageDescription =>
      'Auf Hugging Face gehostete Modelle finden und herunterladen.';

  @override
  String get modelSortDownloads => 'Am häufigsten heruntergeladen';

  @override
  String get modelSortLikes => 'Am meisten geliked';

  @override
  String get modelSortRecentlyUpdated => 'Zuletzt aktualisiert';

  @override
  String get modelPreviousPage => 'Zurück';

  @override
  String get modelNextPage => 'Weiter';

  @override
  String modelPageLabel(int page) {
    return 'Seite $page';
  }

  @override
  String get modelFormatGguf => 'GGUF · llama.cpp';

  @override
  String get modelFormatTransformers => 'Transformers · vLLM';

  @override
  String get modelFormatExllama => 'ExLlama · EXL3';

  @override
  String get huggingFaceModelSearchHint => 'Hugging-Face-Modelle durchsuchen';

  @override
  String get modelSearchRefresh => 'Modellergebnisse aktualisieren';

  @override
  String get modelSearchEmpty => 'Keine Modelle passen zu dieser Suche.';

  @override
  String get modelSearchFailed =>
      'Hugging-Face-Modelle konnten nicht geladen werden.';

  @override
  String get modelSearchUnavailable =>
      'Hugging Face konnte nicht erreicht werden. Prüfen Sie Ihre Verbindung und versuchen Sie es erneut.';

  @override
  String get modelSearchRateLimited =>
      'Hugging Face erhält zu viele Anfragen. Warten Sie einen Moment und versuchen Sie es erneut.';

  @override
  String get modelSearchInvalidResponse =>
      'Hugging Face hat Modelldaten zurückgegeben, die OpenChat nicht lesen konnte. Versuchen Sie es in Kürze erneut.';

  @override
  String get modelSearchTimedOut =>
      'Hugging Face hat zu lange für eine Antwort gebraucht. Versuchen Sie es erneut.';

  @override
  String get modelChooseForDetails =>
      'Wählen Sie ein Modell aus, um seine Dateien zu prüfen.';

  @override
  String get modelDownloadsLabel => 'Downloads';

  @override
  String get modelLikesLabel => 'Likes';

  @override
  String get modelLicenseLabel => 'Lizenz';

  @override
  String get modelRevisionLabel => 'Revision';

  @override
  String get modelFilesLabel => 'Modelldateien';

  @override
  String get modelVisionComponentsLabel => 'Vision-Komponenten';

  @override
  String get modelMtpComponentsLabel => 'MTP-Komponenten';

  @override
  String get modelAuxiliaryComponentsLabel => 'Weitere Zusatzkomponenten';

  @override
  String get modelDownloadComponentButton => 'Diese Komponente herunterladen';

  @override
  String get modelComponentDownloaded => 'Komponente heruntergeladen';

  @override
  String get modelShowMoreComponents => 'Weitere Komponenten anzeigen';

  @override
  String get modelReadmeLabel => 'Modellbeschreibung';

  @override
  String get modelReadmeMissing => 'Dieses Modell hat keine README.';

  @override
  String get modelReadmeAccessDenied =>
      'Für die Beschreibung ist Zugriff auf dieses Repository erforderlich.';

  @override
  String get modelReadmeTooLarge => 'Die README ist zu groß für die Anzeige.';

  @override
  String get modelReadmeUnavailable =>
      'Die Modellbeschreibung konnte nicht geladen werden.';

  @override
  String get modelDownloadOptionsLabel => 'Downloadoptionen';

  @override
  String get modelDownloadGroupLabel => 'Dateisatz';

  @override
  String get modelDownloadSizeLabel => 'Größe';

  @override
  String get modelDownloadButton => 'Modell herunterladen';

  @override
  String get modelCancelDownload => 'Download abbrechen';

  @override
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount) {
    return 'Datei $fileIndex von $fileCount: $fileName';
  }

  @override
  String get modelDownloadComplete =>
      'Modell heruntergeladen und zu den lokalen Modellen hinzugefügt.';

  @override
  String get modelDownloadCancelled =>
      'Modelldownload abgebrochen. Sie können ihn später fortsetzen.';

  @override
  String get modelDownloadFailed =>
      'Das Modell konnte nicht heruntergeladen werden.';

  @override
  String get modelDownloadProgressUnavailable =>
      'Der Downloadfortschritt konnte nicht gelesen werden.';

  @override
  String get modelRevisionChanged =>
      'Dieses Modell wurde auf Hugging Face geändert. Laden Sie seine Dateien neu und versuchen Sie es erneut.';

  @override
  String get modelDownloadAccessNeeded =>
      'Dieses Repository ist geschützt oder privat und benötigt Hugging-Face-Zugriff.';

  @override
  String get modelNoCompatibleFiles =>
      'In diesem Repository wurden keine vollständigen kompatiblen Modelldateien gefunden.';

  @override
  String get modelUnknownDownloadSize =>
      'Die Dateigröße ist nicht verfügbar. Dieser Download kann daher nicht sicher gestartet werden.';

  @override
  String get modelDetailsLoading => 'Modelldateien werden geladen …';

  @override
  String get modelNoFiles =>
      'Für dieses Format sind keine kompatiblen Dateien verfügbar.';

  @override
  String get modelGatedBadge => 'Zugriff erforderlich';

  @override
  String get modelPrivateBadge => 'Privat';

  @override
  String get modelSavedToFolder =>
      'Downloads werden im Bereich Einstellungen im ausgewählten Ordner für diese Engine gespeichert.';

  @override
  String get userQuestionTitle => 'Der Assistent wartet auf deine Antwort';

  @override
  String get userQuestionRequiredHint => 'Pflichtfragen sind gekennzeichnet';

  @override
  String get userQuestionSubmit => 'Antwort senden';

  @override
  String get userQuestionResuming => 'Der Assistent wird fortgesetzt';

  @override
  String get userQuestionUnavailable =>
      'Diese Frage ist nicht mehr verfügbar. Lade den Chat neu.';

  @override
  String get userQuestionRequiredValidation =>
      'Beantworte alle Pflichtfragen, um fortzufahren.';

  @override
  String get userQuestionSubmitFailed =>
      'Deine Antwort konnte nicht gespeichert werden. Versuche es erneut.';

  @override
  String get userQuestionRequiredLabel => 'Erforderlich';

  @override
  String get userQuestionContinue => 'Assistenten fortsetzen';

  @override
  String get userQuestionSaved =>
      'Deine Antwort ist gespeichert. Setze fort, wenn du bereit bist.';

  @override
  String get userQuestionLoadFailed =>
      'Die offene Frage konnte nicht geladen werden. Versuche es erneut.';

  @override
  String get userQuestionResumeFailed =>
      'Die Antwort ist gespeichert, aber der Assistent konnte nicht fortfahren. Versuche es erneut.';

  @override
  String get userQuestionNotificationTitle => 'OpenChat wartet auf dich';

  @override
  String get userQuestionNotificationBody => 'Die KI wartet auf deine Antwort.';
}
