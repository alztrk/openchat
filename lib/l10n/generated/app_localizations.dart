import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_tr.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('de'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('tr'),
  ];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'OpenChat'**
  String get appTitle;

  /// No description provided for @newChat.
  ///
  /// In en, this message translates to:
  /// **'New chat'**
  String get newChat;

  /// No description provided for @chats.
  ///
  /// In en, this message translates to:
  /// **'Chats'**
  String get chats;

  /// No description provided for @collapseSidebars.
  ///
  /// In en, this message translates to:
  /// **'Collapse sidebars'**
  String get collapseSidebars;

  /// No description provided for @showSidebars.
  ///
  /// In en, this message translates to:
  /// **'Show sidebars'**
  String get showSidebars;

  /// No description provided for @home.
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get home;

  /// No description provided for @extensions.
  ///
  /// In en, this message translates to:
  /// **'Extensions'**
  String get extensions;

  /// No description provided for @scheduled.
  ///
  /// In en, this message translates to:
  /// **'Scheduled'**
  String get scheduled;

  /// No description provided for @design.
  ///
  /// In en, this message translates to:
  /// **'Design'**
  String get design;

  /// No description provided for @security.
  ///
  /// In en, this message translates to:
  /// **'Security'**
  String get security;

  /// No description provided for @sectionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This section is not available yet.'**
  String get sectionUnavailable;

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @settingsDescription.
  ///
  /// In en, this message translates to:
  /// **'Connections, appearance, and local data'**
  String get settingsDescription;

  /// No description provided for @connections.
  ///
  /// In en, this message translates to:
  /// **'Connections'**
  String get connections;

  /// No description provided for @models.
  ///
  /// In en, this message translates to:
  /// **'Models'**
  String get models;

  /// No description provided for @modelsDescription.
  ///
  /// In en, this message translates to:
  /// **'Manage models from connected providers, set a default model, and hide models you don\'t need.'**
  String get modelsDescription;

  /// No description provided for @localEngines.
  ///
  /// In en, this message translates to:
  /// **'Local engines'**
  String get localEngines;

  /// No description provided for @localEnginesDescription.
  ///
  /// In en, this message translates to:
  /// **'Install verified local runtimes, register your own model files, and check whether a runtime is healthy. Move or copy models into an engine folder, or keep them where they are.'**
  String get localEnginesDescription;

  /// No description provided for @localEnginesUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The local engine service is not available yet.'**
  String get localEnginesUnavailable;

  /// No description provided for @localEnginesLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Local engine information could not be loaded.'**
  String get localEnginesLoadFailed;

  /// No description provided for @localEnginesEmpty.
  ///
  /// In en, this message translates to:
  /// **'No local engine releases are available.'**
  String get localEnginesEmpty;

  /// No description provided for @localEnginesReload.
  ///
  /// In en, this message translates to:
  /// **'Reload'**
  String get localEnginesReload;

  /// No description provided for @localEngineRelease.
  ///
  /// In en, this message translates to:
  /// **'Release {tag}'**
  String localEngineRelease(String tag);

  /// No description provided for @localEngineVariants.
  ///
  /// In en, this message translates to:
  /// **'Packages'**
  String get localEngineVariants;

  /// No description provided for @localEngineStable.
  ///
  /// In en, this message translates to:
  /// **'Stable'**
  String get localEngineStable;

  /// No description provided for @localEnginePreview.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get localEnginePreview;

  /// No description provided for @localEngineNightly.
  ///
  /// In en, this message translates to:
  /// **'Nightly'**
  String get localEngineNightly;

  /// No description provided for @localEngineRecommended.
  ///
  /// In en, this message translates to:
  /// **'Recommended'**
  String get localEngineRecommended;

  /// No description provided for @localEngineAvailable.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get localEngineAvailable;

  /// No description provided for @localEngineInstalled.
  ///
  /// In en, this message translates to:
  /// **'Installed'**
  String get localEngineInstalled;

  /// No description provided for @localEngineNotInstalled.
  ///
  /// In en, this message translates to:
  /// **'Not installed'**
  String get localEngineNotInstalled;

  /// No description provided for @localEngineBlocked.
  ///
  /// In en, this message translates to:
  /// **'Blocked'**
  String get localEngineBlocked;

  /// No description provided for @localEngineDeprecated.
  ///
  /// In en, this message translates to:
  /// **'Deprecated'**
  String get localEngineDeprecated;

  /// No description provided for @localEngineWindowsDeprecatedReason.
  ///
  /// In en, this message translates to:
  /// **'New vLLM and ExLlama installs are disabled on Windows. Existing runtime files and model registrations are kept.'**
  String get localEngineWindowsDeprecatedReason;

  /// No description provided for @localEngineUnsupportedPlatform.
  ///
  /// In en, this message translates to:
  /// **'Unsupported platform'**
  String get localEngineUnsupportedPlatform;

  /// No description provided for @localEngineHardwareUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Hardware unavailable'**
  String get localEngineHardwareUnavailable;

  /// No description provided for @localEngineDriverUnsupported.
  ///
  /// In en, this message translates to:
  /// **'Update NVIDIA driver'**
  String get localEngineDriverUnsupported;

  /// No description provided for @localEngineDriverVersionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'NVIDIA driver version could not be verified'**
  String get localEngineDriverVersionUnavailable;

  /// No description provided for @localEngineVllmBlockedReason.
  ///
  /// In en, this message translates to:
  /// **'vLLM requires an NVIDIA driver version 580 or newer and an existing Linux environment. On Windows, it uses an existing WSL2 distribution with GPU access.'**
  String get localEngineVllmBlockedReason;

  /// No description provided for @localEngineExllamaBlockedReason.
  ///
  /// In en, this message translates to:
  /// **'The ExLlamaV3 runtime is not ready for installation yet. OpenChat must pin and verify the complete TabbyAPI, PyTorch, Triton, Flash Linear Attention, and Python dependency set before offering it.'**
  String get localEngineExllamaBlockedReason;

  /// No description provided for @localEngineRuntimeRequirements.
  ///
  /// In en, this message translates to:
  /// **'Requirements: {requirements}'**
  String localEngineRuntimeRequirements(String requirements);

  /// No description provided for @localEngineInstall.
  ///
  /// In en, this message translates to:
  /// **'Install'**
  String get localEngineInstall;

  /// No description provided for @localEngineInstalling.
  ///
  /// In en, this message translates to:
  /// **'Preparing installation...'**
  String get localEngineInstalling;

  /// No description provided for @localEngineInstallProgress.
  ///
  /// In en, this message translates to:
  /// **'Local engine installation progress'**
  String get localEngineInstallProgress;

  /// No description provided for @localEngineCancelInstall.
  ///
  /// In en, this message translates to:
  /// **'Cancel installation'**
  String get localEngineCancelInstall;

  /// No description provided for @localEngineCancellingInstall.
  ///
  /// In en, this message translates to:
  /// **'Cancelling...'**
  String get localEngineCancellingInstall;

  /// No description provided for @localEngineInstallFailed.
  ///
  /// In en, this message translates to:
  /// **'The local engine could not be installed. The catalog was refreshed.'**
  String get localEngineInstallFailed;

  /// No description provided for @localEngineHealth.
  ///
  /// In en, this message translates to:
  /// **'Runtime status'**
  String get localEngineHealth;

  /// No description provided for @localEngineExecutable.
  ///
  /// In en, this message translates to:
  /// **'llama-server executable'**
  String get localEngineExecutable;

  /// No description provided for @localEngineExecutableDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose a llama-server binary for OpenChat to launch registered GGUF models. This is separate from connecting to a server you started yourself.'**
  String get localEngineExecutableDescription;

  /// No description provided for @localEngineExecutableChoose.
  ///
  /// In en, this message translates to:
  /// **'Choose executable'**
  String get localEngineExecutableChoose;

  /// No description provided for @localEngineExecutableClear.
  ///
  /// In en, this message translates to:
  /// **'Clear selection'**
  String get localEngineExecutableClear;

  /// No description provided for @localEngineExecutableNotConfigured.
  ///
  /// In en, this message translates to:
  /// **'No executable selected. OpenChat can use an installed package.'**
  String get localEngineExecutableNotConfigured;

  /// No description provided for @localEngineExecutableMissing.
  ///
  /// In en, this message translates to:
  /// **'The saved executable was not found. Choose it again.'**
  String get localEngineExecutableMissing;

  /// No description provided for @localEngineExecutableInvalid.
  ///
  /// In en, this message translates to:
  /// **'Choose an existing llama-server executable.'**
  String get localEngineExecutableInvalid;

  /// No description provided for @localEngineSettingsFailed.
  ///
  /// In en, this message translates to:
  /// **'The llama-server executable setting could not be saved.'**
  String get localEngineSettingsFailed;

  /// No description provided for @localEngineExternalServerCheck.
  ///
  /// In en, this message translates to:
  /// **'Check for running llama-server'**
  String get localEngineExternalServerCheck;

  /// No description provided for @localEngineExternalServerNotConnected.
  ///
  /// In en, this message translates to:
  /// **'No user-started server is connected. OpenChat will not start or stop this server.'**
  String get localEngineExternalServerNotConnected;

  /// No description provided for @localEngineExternalServerConnecting.
  ///
  /// In en, this message translates to:
  /// **'Checking the selected local server...'**
  String get localEngineExternalServerConnecting;

  /// No description provided for @localEngineExternalServerFoundTitle.
  ///
  /// In en, this message translates to:
  /// **'Running llama-server found'**
  String get localEngineExternalServerFoundTitle;

  /// No description provided for @localEngineExternalServerFoundDescription.
  ///
  /// In en, this message translates to:
  /// **'A llama.cpp server is listening on port {port}. OpenChat will connect to it and list its models. Disconnecting in OpenChat will not stop the server.'**
  String localEngineExternalServerFoundDescription(int port);

  /// No description provided for @localEngineExternalServerNotNow.
  ///
  /// In en, this message translates to:
  /// **'Not now'**
  String get localEngineExternalServerNotNow;

  /// No description provided for @localEngineExternalServerConnect.
  ///
  /// In en, this message translates to:
  /// **'Connect'**
  String get localEngineExternalServerConnect;

  /// No description provided for @localEngineExternalServerConnected.
  ///
  /// In en, this message translates to:
  /// **'Connected on port {port}. {count, plural, =1{1 model available} other{{count} models available}}.'**
  String localEngineExternalServerConnected(int port, int count);

  /// No description provided for @localEngineManagedModelSection.
  ///
  /// In en, this message translates to:
  /// **'OpenChat-managed models'**
  String get localEngineManagedModelSection;

  /// No description provided for @localEngineManagedServerModelSection.
  ///
  /// In en, this message translates to:
  /// **'OpenChat server · 127.0.0.1:{port}'**
  String localEngineManagedServerModelSection(int port);

  /// No description provided for @localEngineExternalModelSection.
  ///
  /// In en, this message translates to:
  /// **'User server · 127.0.0.1:{port}'**
  String localEngineExternalModelSection(int port);

  /// No description provided for @localEngineExternalServerDisconnect.
  ///
  /// In en, this message translates to:
  /// **'Disconnect'**
  String get localEngineExternalServerDisconnect;

  /// No description provided for @localEngineExternalServerNotFound.
  ///
  /// In en, this message translates to:
  /// **'No running llama-server was found.'**
  String get localEngineExternalServerNotFound;

  /// No description provided for @localEngineExternalServerScanFailed.
  ///
  /// In en, this message translates to:
  /// **'Running llama-server processes could not be checked.'**
  String get localEngineExternalServerScanFailed;

  /// No description provided for @localEngineExternalServerConnectFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not connect. Check that llama-server is ready and exposes its local model endpoint.'**
  String get localEngineExternalServerConnectFailed;

  /// No description provided for @localEngineExternalServerAuthRequired.
  ///
  /// In en, this message translates to:
  /// **'This server requires authentication. OpenChat does not read or reuse credentials from other processes.'**
  String get localEngineExternalServerAuthRequired;

  /// No description provided for @localEngineRunning.
  ///
  /// In en, this message translates to:
  /// **'Running'**
  String get localEngineRunning;

  /// No description provided for @localEngineStopped.
  ///
  /// In en, this message translates to:
  /// **'Stopped'**
  String get localEngineStopped;

  /// No description provided for @localEngineUnhealthy.
  ///
  /// In en, this message translates to:
  /// **'Not responding'**
  String get localEngineUnhealthy;

  /// No description provided for @localEngineUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This engine cannot be started yet.'**
  String get localEngineUnavailable;

  /// No description provided for @localEngineStartModel.
  ///
  /// In en, this message translates to:
  /// **'Start model'**
  String get localEngineStartModel;

  /// No description provided for @localEngineStopModel.
  ///
  /// In en, this message translates to:
  /// **'Stop engine'**
  String get localEngineStopModel;

  /// No description provided for @localModels.
  ///
  /// In en, this message translates to:
  /// **'Registered models'**
  String get localModels;

  /// No description provided for @localModelsEmpty.
  ///
  /// In en, this message translates to:
  /// **'No models are registered for this engine.'**
  String get localModelsEmpty;

  /// No description provided for @localModelsPageTitle.
  ///
  /// In en, this message translates to:
  /// **'Local models'**
  String get localModelsPageTitle;

  /// No description provided for @localModelsPageDescription.
  ///
  /// In en, this message translates to:
  /// **'View downloaded and registered local models.'**
  String get localModelsPageDescription;

  /// No description provided for @localModelsPageEmpty.
  ///
  /// In en, this message translates to:
  /// **'No local models are registered yet.'**
  String get localModelsPageEmpty;

  /// No description provided for @localModelsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Local models could not be loaded.'**
  String get localModelsLoadFailed;

  /// No description provided for @localModelsRefresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get localModelsRefresh;

  /// No description provided for @localModelsDiscover.
  ///
  /// In en, this message translates to:
  /// **'Discover models'**
  String get localModelsDiscover;

  /// No description provided for @localModelAddFile.
  ///
  /// In en, this message translates to:
  /// **'Add model file'**
  String get localModelAddFile;

  /// No description provided for @localModelAddFolder.
  ///
  /// In en, this message translates to:
  /// **'Add model folder'**
  String get localModelAddFolder;

  /// No description provided for @localModelStorageChoiceTitle.
  ///
  /// In en, this message translates to:
  /// **'Choose where to keep the model'**
  String get localModelStorageChoiceTitle;

  /// No description provided for @localModelStorageChoiceTarget.
  ///
  /// In en, this message translates to:
  /// **'Selected model folder: {folder}'**
  String localModelStorageChoiceTarget(String folder);

  /// No description provided for @localModelDirectoryTitle.
  ///
  /// In en, this message translates to:
  /// **'Model folder'**
  String get localModelDirectoryTitle;

  /// No description provided for @localModelDirectoryDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose where models for this engine are stored. Changing this folder does not move models already registered.'**
  String get localModelDirectoryDescription;

  /// No description provided for @localModelChooseDirectory.
  ///
  /// In en, this message translates to:
  /// **'Choose folder'**
  String get localModelChooseDirectory;

  /// No description provided for @localModelUseDefaultDirectory.
  ///
  /// In en, this message translates to:
  /// **'Use default'**
  String get localModelUseDefaultDirectory;

  /// No description provided for @localModelScanDirectory.
  ///
  /// In en, this message translates to:
  /// **'Scan folder'**
  String get localModelScanDirectory;

  /// No description provided for @localModelScanningDirectory.
  ///
  /// In en, this message translates to:
  /// **'Scanning folder...'**
  String get localModelScanningDirectory;

  /// No description provided for @localModelDiscoveryTitle.
  ///
  /// In en, this message translates to:
  /// **'Unregistered models found'**
  String get localModelDiscoveryTitle;

  /// No description provided for @localModelDiscoveryPrompt.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{OpenChat found 1 supported model in this folder that is not registered. Register it?} other{OpenChat found {count} supported models in this folder that are not registered. Register them?}}'**
  String localModelDiscoveryPrompt(int count);

  /// No description provided for @localModelDiscoveryTruncated.
  ///
  /// In en, this message translates to:
  /// **'The scan reached its safe limit. Choose a smaller folder to find more models.'**
  String get localModelDiscoveryTruncated;

  /// No description provided for @localModelDiscoveryEmpty.
  ///
  /// In en, this message translates to:
  /// **'No new supported models were found in this folder.'**
  String get localModelDiscoveryEmpty;

  /// No description provided for @localModelDiscoveryRegisterAll.
  ///
  /// In en, this message translates to:
  /// **'Register found models'**
  String get localModelDiscoveryRegisterAll;

  /// No description provided for @localModelDiscoveryRegistered.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Registered 1 model.} other{Registered {count} models.}}'**
  String localModelDiscoveryRegistered(int count);

  /// No description provided for @localModelDiscoveryPartial.
  ///
  /// In en, this message translates to:
  /// **'Registered {registered} of {total} models. Some models could not be registered.'**
  String localModelDiscoveryPartial(int registered, int total);

  /// No description provided for @localModelDirectoryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This model folder is unavailable. Choose a folder that exists and that OpenChat can access.'**
  String get localModelDirectoryUnavailable;

  /// No description provided for @localModelDiscoveryFailed.
  ///
  /// In en, this message translates to:
  /// **'The model folder could not be scanned. Check access permissions and try again.'**
  String get localModelDiscoveryFailed;

  /// No description provided for @localModelMoveToFolder.
  ///
  /// In en, this message translates to:
  /// **'Move model to this folder'**
  String get localModelMoveToFolder;

  /// No description provided for @localModelCopyToFolder.
  ///
  /// In en, this message translates to:
  /// **'Copy model to this folder'**
  String get localModelCopyToFolder;

  /// No description provided for @localModelKeepInPlace.
  ///
  /// In en, this message translates to:
  /// **'Keep the model where it is'**
  String get localModelKeepInPlace;

  /// No description provided for @localModelSaving.
  ///
  /// In en, this message translates to:
  /// **'Saving model...'**
  String get localModelSaving;

  /// No description provided for @localModelTransferError.
  ///
  /// In en, this message translates to:
  /// **'The model could not be copied or moved. The original model was left in place.'**
  String get localModelTransferError;

  /// No description provided for @localModelTransferRecoveryError.
  ///
  /// In en, this message translates to:
  /// **'The model could not be registered or restored. A complete model copy remains in the OpenChat model folder; select it there to register it.'**
  String get localModelTransferRecoveryError;

  /// No description provided for @localModelRemove.
  ///
  /// In en, this message translates to:
  /// **'Remove registration'**
  String get localModelRemove;

  /// No description provided for @localModelRemoveConfirmation.
  ///
  /// In en, this message translates to:
  /// **'Only the OpenChat registration will be removed. The model file will stay on disk. Continue?'**
  String get localModelRemoveConfirmation;

  /// No description provided for @localModelCancelStart.
  ///
  /// In en, this message translates to:
  /// **'Cancel startup'**
  String get localModelCancelStart;

  /// No description provided for @localModelStopping.
  ///
  /// In en, this message translates to:
  /// **'Stopping runtime...'**
  String get localModelStopping;

  /// No description provided for @localModelActionError.
  ///
  /// In en, this message translates to:
  /// **'The local model action could not be completed.'**
  String get localModelActionError;

  /// No description provided for @localModelPathMissing.
  ///
  /// In en, this message translates to:
  /// **'The model path could not be found. Move it back or remove its registration.'**
  String get localModelPathMissing;

  /// No description provided for @localModelEngineNotReady.
  ///
  /// In en, this message translates to:
  /// **'Available after this engine is installed and can run models.'**
  String get localModelEngineNotReady;

  /// No description provided for @localModelInvalid.
  ///
  /// In en, this message translates to:
  /// **'The selected file or folder is not a valid model for this engine.'**
  String get localModelInvalid;

  /// No description provided for @localModelPathError.
  ///
  /// In en, this message translates to:
  /// **'The selected model file or folder could not be accessed.'**
  String get localModelPathError;

  /// No description provided for @localModelStoragePathError.
  ///
  /// In en, this message translates to:
  /// **'OpenChat could not create its model folders. Check available storage and permissions, then try again.'**
  String get localModelStoragePathError;

  /// No description provided for @localModelStorageError.
  ///
  /// In en, this message translates to:
  /// **'The model registration could not be written to the database.'**
  String get localModelStorageError;

  /// No description provided for @localModelStartError.
  ///
  /// In en, this message translates to:
  /// **'The model could not be started. Check the runtime installation and model file.'**
  String get localModelStartError;

  /// No description provided for @localModelStartTimeout.
  ///
  /// In en, this message translates to:
  /// **'The model did not become ready in time. Try a smaller model or check your hardware.'**
  String get localModelStartTimeout;

  /// No description provided for @localModelRuntimeUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The local model stopped responding. Restart it from Settings > Local engines and try again.'**
  String get localModelRuntimeUnavailable;

  /// No description provided for @localModelContextUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The local runtime did not report its active context window. Update or reinstall llama.cpp, then try again.'**
  String get localModelContextUnavailable;

  /// No description provided for @localModelInferenceFailed.
  ///
  /// In en, this message translates to:
  /// **'The local model could not handle this request. Check its chat template and available memory.'**
  String get localModelInferenceFailed;

  /// No description provided for @localModelSaved.
  ///
  /// In en, this message translates to:
  /// **'Local model registered.'**
  String get localModelSaved;

  /// No description provided for @localModelRemoved.
  ///
  /// In en, this message translates to:
  /// **'Local model registration removed.'**
  String get localModelRemoved;

  /// No description provided for @localEngineStageDownloading.
  ///
  /// In en, this message translates to:
  /// **'Downloading'**
  String get localEngineStageDownloading;

  /// No description provided for @localEngineStageVerifying.
  ///
  /// In en, this message translates to:
  /// **'Verifying'**
  String get localEngineStageVerifying;

  /// No description provided for @localEngineStageExtracting.
  ///
  /// In en, this message translates to:
  /// **'Extracting'**
  String get localEngineStageExtracting;

  /// No description provided for @localEngineStageRuntimeSetup.
  ///
  /// In en, this message translates to:
  /// **'Setting up the Python runtime'**
  String get localEngineStageRuntimeSetup;

  /// No description provided for @localEngineStagePublishing.
  ///
  /// In en, this message translates to:
  /// **'Finishing installation'**
  String get localEngineStagePublishing;

  /// No description provided for @localEngineStageReady.
  ///
  /// In en, this message translates to:
  /// **'Ready'**
  String get localEngineStageReady;

  /// No description provided for @defaultModel.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get defaultModel;

  /// No description provided for @setDefaultModel.
  ///
  /// In en, this message translates to:
  /// **'Set as default'**
  String get setDefaultModel;

  /// No description provided for @clearDefaultModel.
  ///
  /// In en, this message translates to:
  /// **'Remove default'**
  String get clearDefaultModel;

  /// No description provided for @defaultModelUpdated.
  ///
  /// In en, this message translates to:
  /// **'Default model updated: {model}'**
  String defaultModelUpdated(String model);

  /// No description provided for @defaultModelCleared.
  ///
  /// In en, this message translates to:
  /// **'Default model removed.'**
  String get defaultModelCleared;

  /// No description provided for @hideModel.
  ///
  /// In en, this message translates to:
  /// **'Hide'**
  String get hideModel;

  /// No description provided for @showModel.
  ///
  /// In en, this message translates to:
  /// **'Show'**
  String get showModel;

  /// No description provided for @hiddenModel.
  ///
  /// In en, this message translates to:
  /// **'Hidden'**
  String get hiddenModel;

  /// No description provided for @modelHidden.
  ///
  /// In en, this message translates to:
  /// **'Model hidden: {model}'**
  String modelHidden(String model);

  /// No description provided for @modelUnhidden.
  ///
  /// In en, this message translates to:
  /// **'Model shown: {model}'**
  String modelUnhidden(String model);

  /// No description provided for @noModelsFound.
  ///
  /// In en, this message translates to:
  /// **'No models found.'**
  String get noModelsFound;

  /// No description provided for @refreshModels.
  ///
  /// In en, this message translates to:
  /// **'Refresh models'**
  String get refreshModels;

  /// No description provided for @connectedProvidersModels.
  ///
  /// In en, this message translates to:
  /// **'Connected provider models'**
  String get connectedProvidersModels;

  /// No description provided for @noConnectedProviders.
  ///
  /// In en, this message translates to:
  /// **'No providers connected yet. Connect accounts or add API keys in the Connections tab.'**
  String get noConnectedProviders;

  /// No description provided for @sharedInstructions.
  ///
  /// In en, this message translates to:
  /// **'Shared instructions'**
  String get sharedInstructions;

  /// No description provided for @sharedInstructionsDescription.
  ///
  /// In en, this message translates to:
  /// **'These instructions are sent with every connected provider. File tool access follows the Tool access setting.'**
  String get sharedInstructionsDescription;

  /// No description provided for @sharedInstructionsHint.
  ///
  /// In en, this message translates to:
  /// **'Describe how you want responses to be written...'**
  String get sharedInstructionsHint;

  /// No description provided for @sharedInstructionsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Shared instructions could not be loaded. Try again.'**
  String get sharedInstructionsLoadFailed;

  /// No description provided for @sharedInstructionsSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Shared instructions could not be saved.'**
  String get sharedInstructionsSaveFailed;

  /// No description provided for @sharedInstructionsSaved.
  ///
  /// In en, this message translates to:
  /// **'Shared instructions saved.'**
  String get sharedInstructionsSaved;

  /// No description provided for @sharedInstructionsTooLong.
  ///
  /// In en, this message translates to:
  /// **'Shared instructions cannot exceed 4096 characters.'**
  String get sharedInstructionsTooLong;

  /// No description provided for @chatGptProvider.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT'**
  String get chatGptProvider;

  /// No description provided for @openCodeProvider.
  ///
  /// In en, this message translates to:
  /// **'OpenCode'**
  String get openCodeProvider;

  /// No description provided for @geminiProvider.
  ///
  /// In en, this message translates to:
  /// **'Gemini'**
  String get geminiProvider;

  /// No description provided for @groqProvider.
  ///
  /// In en, this message translates to:
  /// **'Groq'**
  String get groqProvider;

  /// No description provided for @cerebrasProvider.
  ///
  /// In en, this message translates to:
  /// **'Cerebras'**
  String get cerebrasProvider;

  /// No description provided for @openRouterProvider.
  ///
  /// In en, this message translates to:
  /// **'OpenRouter'**
  String get openRouterProvider;

  /// No description provided for @mistralProvider.
  ///
  /// In en, this message translates to:
  /// **'Mistral'**
  String get mistralProvider;

  /// No description provided for @geminiApiDescription.
  ///
  /// In en, this message translates to:
  /// **'Add a Google AI Studio API key to use the models available to your account. Free access depends on the model and your quota.'**
  String get geminiApiDescription;

  /// No description provided for @groqApiDescription.
  ///
  /// In en, this message translates to:
  /// **'Use the models enabled for your account with a Groq API key. Pricing and usage limits vary by plan and model.'**
  String get groqApiDescription;

  /// No description provided for @cerebrasApiDescription.
  ///
  /// In en, this message translates to:
  /// **'Use the tool-capable models available to your account with a Cerebras API key. Access, pricing, and limits vary by model and account.'**
  String get cerebrasApiDescription;

  /// No description provided for @openRouterApiDescription.
  ///
  /// In en, this message translates to:
  /// **'Only models listed at \$0 for input and output with text and tool support appear here. Actual access and limits can change.'**
  String get openRouterApiDescription;

  /// No description provided for @mistralApiDescription.
  ///
  /// In en, this message translates to:
  /// **'Add a Mistral API key to use the chat models available to your account. Free access, pricing, and usage limits depend on your Mistral plan.'**
  String get mistralApiDescription;

  /// No description provided for @geminiUnpaidDataNotice.
  ///
  /// In en, this message translates to:
  /// **'On Gemini API\'s free tier, Google may use submitted content to improve its products. Review Google\'s data-use terms before sending sensitive information.'**
  String get geminiUnpaidDataNotice;

  /// No description provided for @providerApiKey.
  ///
  /// In en, this message translates to:
  /// **'API key'**
  String get providerApiKey;

  /// No description provided for @providerKeySaved.
  ///
  /// In en, this message translates to:
  /// **'API key is saved securely on this device.'**
  String get providerKeySaved;

  /// No description provided for @providerKeySavedSuffix.
  ///
  /// In en, this message translates to:
  /// **'API key ending in ••••{suffix} is saved securely on this device.'**
  String providerKeySavedSuffix(Object suffix);

  /// No description provided for @providerNoKey.
  ///
  /// In en, this message translates to:
  /// **'No API key is connected.'**
  String get providerNoKey;

  /// No description provided for @providerKeyInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a valid API key for this provider. Do not include spaces; the key can be up to 4096 characters.'**
  String get providerKeyInvalid;

  /// No description provided for @providerKeyStorageFailed.
  ///
  /// In en, this message translates to:
  /// **'The API key could not be read or saved securely.'**
  String get providerKeyStorageFailed;

  /// No description provided for @favoriteModels.
  ///
  /// In en, this message translates to:
  /// **'Favorites'**
  String get favoriteModels;

  /// No description provided for @favoriteModelsEmpty.
  ///
  /// In en, this message translates to:
  /// **'No favorite models yet.'**
  String get favoriteModelsEmpty;

  /// No description provided for @modelSearchHint.
  ///
  /// In en, this message translates to:
  /// **'Search models...'**
  String get modelSearchHint;

  /// No description provided for @modelSearchNoResults.
  ///
  /// In en, this message translates to:
  /// **'No models match your search.'**
  String get modelSearchNoResults;

  /// No description provided for @addModelFavorite.
  ///
  /// In en, this message translates to:
  /// **'Add to favorite models'**
  String get addModelFavorite;

  /// No description provided for @removeModelFavorite.
  ///
  /// In en, this message translates to:
  /// **'Remove from favorites'**
  String get removeModelFavorite;

  /// No description provided for @openCodeConsole.
  ///
  /// In en, this message translates to:
  /// **'OpenCode Console'**
  String get openCodeConsole;

  /// No description provided for @openCodeConsoleDescription.
  ///
  /// In en, this message translates to:
  /// **'Free models work without a key. Add a Console API key for paid models; each request is charged to your Console balance.'**
  String get openCodeConsoleDescription;

  /// No description provided for @openCodeKeySaved.
  ///
  /// In en, this message translates to:
  /// **'Console API key ending in ••••{suffix} is saved securely on this device.'**
  String openCodeKeySaved(Object suffix);

  /// No description provided for @openCodeNoKey.
  ///
  /// In en, this message translates to:
  /// **'No Console API key. Free models are available.'**
  String get openCodeNoKey;

  /// No description provided for @openCodeApiKey.
  ///
  /// In en, this message translates to:
  /// **'OpenCode Console API key'**
  String get openCodeApiKey;

  /// No description provided for @openCodeKeyInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a non-empty API key without spaces or line breaks (up to 4096 characters).'**
  String get openCodeKeyInvalid;

  /// No description provided for @openCodeKeyStorageFailed.
  ///
  /// In en, this message translates to:
  /// **'The Console API key could not be read or saved securely.'**
  String get openCodeKeyStorageFailed;

  /// No description provided for @openCodePaidModel.
  ///
  /// In en, this message translates to:
  /// **'Paid'**
  String get openCodePaidModel;

  /// No description provided for @openCodeFreeModel.
  ///
  /// In en, this message translates to:
  /// **'Free'**
  String get openCodeFreeModel;

  /// No description provided for @modelSourceApi.
  ///
  /// In en, this message translates to:
  /// **'API'**
  String get modelSourceApi;

  /// No description provided for @modelSourceOAuth.
  ///
  /// In en, this message translates to:
  /// **'OAuth'**
  String get modelSourceOAuth;

  /// No description provided for @openCodeFreeModels.
  ///
  /// In en, this message translates to:
  /// **'Free models'**
  String get openCodeFreeModels;

  /// No description provided for @openCodeApiModels.
  ///
  /// In en, this message translates to:
  /// **'API models'**
  String get openCodeApiModels;

  /// No description provided for @modelContextWindow.
  ///
  /// In en, this message translates to:
  /// **'Context window · {value} tokens'**
  String modelContextWindow(String value);

  /// No description provided for @openCodeModelContextWindow.
  ///
  /// In en, this message translates to:
  /// **'OpenCode catalog (Models.dev) · context: {value} tokens'**
  String openCodeModelContextWindow(String value);

  /// No description provided for @add.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get add;

  /// No description provided for @edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// No description provided for @exportConversation.
  ///
  /// In en, this message translates to:
  /// **'Export conversation'**
  String get exportConversation;

  /// No description provided for @deleteConversation.
  ///
  /// In en, this message translates to:
  /// **'Delete conversation'**
  String get deleteConversation;

  /// No description provided for @confirmDeleteConversationTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete this conversation?'**
  String get confirmDeleteConversationTitle;

  /// No description provided for @confirmDeleteConversation.
  ///
  /// In en, this message translates to:
  /// **'This will permanently delete “{title}” and all its messages from this device.'**
  String confirmDeleteConversation(String title);

  /// No description provided for @stopResponseBeforeDelete.
  ///
  /// In en, this message translates to:
  /// **'Stop the active response before deleting this conversation.'**
  String get stopResponseBeforeDelete;

  /// No description provided for @conversationDeleted.
  ///
  /// In en, this message translates to:
  /// **'Conversation deleted.'**
  String get conversationDeleted;

  /// No description provided for @conversationDeleteFailed.
  ///
  /// In en, this message translates to:
  /// **'The conversation could not be deleted. Try again.'**
  String get conversationDeleteFailed;

  /// No description provided for @conversationExported.
  ///
  /// In en, this message translates to:
  /// **'Conversation exported as a Markdown file.'**
  String get conversationExported;

  /// No description provided for @conversationExportFailed.
  ///
  /// In en, this message translates to:
  /// **'The conversation could not be exported. Try again.'**
  String get conversationExportFailed;

  /// No description provided for @conversationExportProvider.
  ///
  /// In en, this message translates to:
  /// **'Provider'**
  String get conversationExportProvider;

  /// No description provided for @conversationExportModel.
  ///
  /// In en, this message translates to:
  /// **'Model'**
  String get conversationExportModel;

  /// No description provided for @conversationExportCreated.
  ///
  /// In en, this message translates to:
  /// **'Created'**
  String get conversationExportCreated;

  /// No description provided for @conversationExportStatus.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get conversationExportStatus;

  /// No description provided for @toolPermissions.
  ///
  /// In en, this message translates to:
  /// **'Tool access'**
  String get toolPermissions;

  /// No description provided for @toolPermissionsDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose where AI file tools can operate and whether each call requires your approval.'**
  String get toolPermissionsDescription;

  /// No description provided for @toolPermissionRequireApproval.
  ///
  /// In en, this message translates to:
  /// **'Ask for approval'**
  String get toolPermissionRequireApproval;

  /// No description provided for @selectedModelDoesNotSupportToolCalls.
  ///
  /// In en, this message translates to:
  /// **'This model does not support tool calls. Tool access settings do not apply to it.'**
  String get selectedModelDoesNotSupportToolCalls;

  /// No description provided for @selectedModelToolSupportUnknown.
  ///
  /// In en, this message translates to:
  /// **'This model does not report tool-call support. OpenChat will try sending tools; the provider may reject the request.'**
  String get selectedModelToolSupportUnknown;

  /// No description provided for @toolPermissionRequireApprovalDescription.
  ///
  /// In en, this message translates to:
  /// **'File tools ask before each call and are limited to the project folder and %LOCALAPPDATA%\\OpenChat. Command execution is unavailable.'**
  String get toolPermissionRequireApprovalDescription;

  /// No description provided for @toolPermissionFullAccess.
  ///
  /// In en, this message translates to:
  /// **'Full access'**
  String get toolPermissionFullAccess;

  /// No description provided for @toolPermissionFullAccessDescription.
  ///
  /// In en, this message translates to:
  /// **'File tools can read and change files in any folder without asking. Command execution is unavailable.'**
  String get toolPermissionFullAccessDescription;

  /// No description provided for @toolPermissionSettingsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Tool access settings could not be loaded.'**
  String get toolPermissionSettingsLoadFailed;

  /// No description provided for @toolPermissionSettingsSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Tool access settings could not be saved. Try again.'**
  String get toolPermissionSettingsSaveFailed;

  /// No description provided for @toolPermissionRequestTitle.
  ///
  /// In en, this message translates to:
  /// **'Tool permission'**
  String get toolPermissionRequestTitle;

  /// No description provided for @toolPermissionRequestDescription.
  ///
  /// In en, this message translates to:
  /// **'The AI wants to use this tool at the selected location. Permission applies to this call only.'**
  String get toolPermissionRequestDescription;

  /// No description provided for @toolPermissionRequestExpired.
  ///
  /// In en, this message translates to:
  /// **'This tool permission request is no longer active.'**
  String get toolPermissionRequestExpired;

  /// No description provided for @toolPermissionResponseFailed.
  ///
  /// In en, this message translates to:
  /// **'Your choice could not be sent. You can try again.'**
  String get toolPermissionResponseFailed;

  /// No description provided for @toolPermissionContent.
  ///
  /// In en, this message translates to:
  /// **'Content to write'**
  String get toolPermissionContent;

  /// No description provided for @toolPermissionOldText.
  ///
  /// In en, this message translates to:
  /// **'Text to find'**
  String get toolPermissionOldText;

  /// No description provided for @toolPermissionNewText.
  ///
  /// In en, this message translates to:
  /// **'Replacement text'**
  String get toolPermissionNewText;

  /// No description provided for @toolPermissionQuery.
  ///
  /// In en, this message translates to:
  /// **'Search text'**
  String get toolPermissionQuery;

  /// No description provided for @toolPermissionOffset.
  ///
  /// In en, this message translates to:
  /// **'Starting position'**
  String get toolPermissionOffset;

  /// No description provided for @toolPermissionLimit.
  ///
  /// In en, this message translates to:
  /// **'Maximum results'**
  String get toolPermissionLimit;

  /// No description provided for @toolPermissionStartLine.
  ///
  /// In en, this message translates to:
  /// **'Starting line'**
  String get toolPermissionStartLine;

  /// No description provided for @toolPermissionLineCount.
  ///
  /// In en, this message translates to:
  /// **'Number of lines'**
  String get toolPermissionLineCount;

  /// No description provided for @toolPermissionIncludeHidden.
  ///
  /// In en, this message translates to:
  /// **'Include hidden files'**
  String get toolPermissionIncludeHidden;

  /// No description provided for @commonYes.
  ///
  /// In en, this message translates to:
  /// **'Yes'**
  String get commonYes;

  /// No description provided for @commonNo.
  ///
  /// In en, this message translates to:
  /// **'No'**
  String get commonNo;

  /// No description provided for @toolPermissionTarget.
  ///
  /// In en, this message translates to:
  /// **'Location to access'**
  String get toolPermissionTarget;

  /// No description provided for @toolPermissionTool.
  ///
  /// In en, this message translates to:
  /// **'Tool'**
  String get toolPermissionTool;

  /// No description provided for @toolPermissionArguments.
  ///
  /// In en, this message translates to:
  /// **'Request details'**
  String get toolPermissionArguments;

  /// No description provided for @toolPermissionDeny.
  ///
  /// In en, this message translates to:
  /// **'Deny'**
  String get toolPermissionDeny;

  /// No description provided for @toolPermissionStopResponse.
  ///
  /// In en, this message translates to:
  /// **'Stop response'**
  String get toolPermissionStopResponse;

  /// No description provided for @toolPermissionAllowOnce.
  ///
  /// In en, this message translates to:
  /// **'Allow this call'**
  String get toolPermissionAllowOnce;

  /// No description provided for @toolDenied.
  ///
  /// In en, this message translates to:
  /// **'Denied'**
  String get toolDenied;

  /// No description provided for @toolCancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get toolCancelled;

  /// No description provided for @toolAwaitingApproval.
  ///
  /// In en, this message translates to:
  /// **'Waiting for approval'**
  String get toolAwaitingApproval;

  /// No description provided for @conversationExportToolActivity.
  ///
  /// In en, this message translates to:
  /// **'Tool activity'**
  String get conversationExportToolActivity;

  /// No description provided for @responseReplaceFailed.
  ///
  /// In en, this message translates to:
  /// **'The new response was saved, but the earlier response could not be replaced.'**
  String get responseReplaceFailed;

  /// No description provided for @responseRetryNotCompleted.
  ///
  /// In en, this message translates to:
  /// **'The new response did not finish. The earlier response was kept.'**
  String get responseRetryNotCompleted;

  /// No description provided for @responseRetryCleanupFailed.
  ///
  /// In en, this message translates to:
  /// **'The retry could not be cleaned up. Refresh the conversation history.'**
  String get responseRetryCleanupFailed;

  /// No description provided for @responseInProgress.
  ///
  /// In en, this message translates to:
  /// **'Response in progress'**
  String get responseInProgress;

  /// No description provided for @responseRetryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This response cannot be retried. Start a new message instead.'**
  String get responseRetryUnavailable;

  /// No description provided for @providerRateLimited.
  ///
  /// In en, this message translates to:
  /// **'The provider reported a usage limit. Try again later.'**
  String get providerRateLimited;

  /// No description provided for @providerAuthenticationRequired.
  ///
  /// In en, this message translates to:
  /// **'The provider rejected the request. Check the connection and model access.'**
  String get providerAuthenticationRequired;

  /// No description provided for @providerRequestFailed.
  ///
  /// In en, this message translates to:
  /// **'The provider could not complete the response. Your saved messages are still available.'**
  String get providerRequestFailed;

  /// No description provided for @providerToolRequestRejected.
  ///
  /// In en, this message translates to:
  /// **'The provider rejected a request containing tools. Check the model\'s tool support or choose a model that advertises tool use.'**
  String get providerToolRequestRejected;

  /// No description provided for @providerNetworkUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The provider could not be reached. Check your connection and try again.'**
  String get providerNetworkUnavailable;

  /// No description provided for @contextWindowExceeded.
  ///
  /// In en, this message translates to:
  /// **'The conversation is too large for this model\'s context window. Shorten the latest message or choose a model with a larger context window. Your chat history is saved.'**
  String get contextWindowExceeded;

  /// No description provided for @openCodeFreeTierRestricted.
  ///
  /// In en, this message translates to:
  /// **'OpenCode free models are only available within the OpenCode app.'**
  String get openCodeFreeTierRestricted;

  /// No description provided for @apiKey.
  ///
  /// In en, this message translates to:
  /// **'API key'**
  String get apiKey;

  /// No description provided for @apiKeyInputLabel.
  ///
  /// In en, this message translates to:
  /// **'API key'**
  String get apiKeyInputLabel;

  /// No description provided for @apiKeyRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter an API key.'**
  String get apiKeyRequired;

  /// No description provided for @apiKeyInvalidFormat.
  ///
  /// In en, this message translates to:
  /// **'Enter a valid OpenAI API key. It must start with sk-.'**
  String get apiKeyInvalidFormat;

  /// No description provided for @apiKeyAlreadySaved.
  ///
  /// In en, this message translates to:
  /// **'This API key is already saved.'**
  String get apiKeyAlreadySaved;

  /// No description provided for @apiKeySaved.
  ///
  /// In en, this message translates to:
  /// **'API key saved securely on this device.'**
  String get apiKeySaved;

  /// No description provided for @apiKeySaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The API key could not be saved securely. Try again.'**
  String get apiKeySaveFailed;

  /// No description provided for @apiKeyLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Saved API keys could not be loaded.'**
  String get apiKeyLoadFailed;

  /// No description provided for @savedApiKey.
  ///
  /// In en, this message translates to:
  /// **'Saved key'**
  String get savedApiKey;

  /// No description provided for @savedApiKeyWithSuffix.
  ///
  /// In en, this message translates to:
  /// **'Key ending in ••••{suffix}'**
  String savedApiKeyWithSuffix(String suffix);

  /// No description provided for @showApiKey.
  ///
  /// In en, this message translates to:
  /// **'Show API key'**
  String get showApiKey;

  /// No description provided for @hideApiKey.
  ///
  /// In en, this message translates to:
  /// **'Hide API key'**
  String get hideApiKey;

  /// No description provided for @retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @saving.
  ///
  /// In en, this message translates to:
  /// **'Saving...'**
  String get saving;

  /// No description provided for @oauth.
  ///
  /// In en, this message translates to:
  /// **'OAuth'**
  String get oauth;

  /// No description provided for @oauthSigningIn.
  ///
  /// In en, this message translates to:
  /// **'Signing in...'**
  String get oauthSigningIn;

  /// No description provided for @oauthBrowserWaiting.
  ///
  /// In en, this message translates to:
  /// **'Complete sign-in in the browser.'**
  String get oauthBrowserWaiting;

  /// No description provided for @oauthConnectionsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT OAuth connections could not be loaded.'**
  String get oauthConnectionsLoadFailed;

  /// No description provided for @oauthSignInFailed.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT sign-in could not be completed. Check the browser and try again.'**
  String get oauthSignInFailed;

  /// No description provided for @oauthConnectionAdded.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT account connected.'**
  String get oauthConnectionAdded;

  /// No description provided for @oldCredentialCleanupFailed.
  ///
  /// In en, this message translates to:
  /// **'The account connected, but an older saved credential could not be removed. Restart OpenChat and try again.'**
  String get oldCredentialCleanupFailed;

  /// No description provided for @connectionSelectionFailed.
  ///
  /// In en, this message translates to:
  /// **'The ChatGPT account could not be selected.'**
  String get connectionSelectionFailed;

  /// No description provided for @removeChatGptConnection.
  ///
  /// In en, this message translates to:
  /// **'Remove ChatGPT connection'**
  String get removeChatGptConnection;

  /// No description provided for @removeConnectionAction.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get removeConnectionAction;

  /// No description provided for @confirmRemoveConnection.
  ///
  /// In en, this message translates to:
  /// **'Remove {name} and its saved credentials? Existing chats and messages will remain on this device.'**
  String confirmRemoveConnection(String name);

  /// No description provided for @connectionRemoveSucceeded.
  ///
  /// In en, this message translates to:
  /// **'Connection removed. Existing chats and messages are still available.'**
  String get connectionRemoveSucceeded;

  /// No description provided for @connectionRemoveFailed.
  ///
  /// In en, this message translates to:
  /// **'The connection could not be removed. Try again.'**
  String get connectionRemoveFailed;

  /// No description provided for @workspaceSelectionFailed.
  ///
  /// In en, this message translates to:
  /// **'The ChatGPT workspace could not be selected.'**
  String get workspaceSelectionFailed;

  /// No description provided for @chatGptAccount.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT account'**
  String get chatGptAccount;

  /// No description provided for @planUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Plan unavailable'**
  String get planUnavailable;

  /// No description provided for @accountPlan.
  ///
  /// In en, this message translates to:
  /// **'Plan: {plan}'**
  String accountPlan(String plan);

  /// No description provided for @connectionNeedsSignIn.
  ///
  /// In en, this message translates to:
  /// **'Sign in again to use this account.'**
  String get connectionNeedsSignIn;

  /// No description provided for @connectionSelected.
  ///
  /// In en, this message translates to:
  /// **'Selected'**
  String get connectionSelected;

  /// No description provided for @useConnection.
  ///
  /// In en, this message translates to:
  /// **'Use account'**
  String get useConnection;

  /// No description provided for @selectWorkspace.
  ///
  /// In en, this message translates to:
  /// **'Choose a workspace'**
  String get selectWorkspace;

  /// No description provided for @workspaceWithoutName.
  ///
  /// In en, this message translates to:
  /// **'Workspace'**
  String get workspaceWithoutName;

  /// No description provided for @workspace.
  ///
  /// In en, this message translates to:
  /// **'Workspace'**
  String get workspace;

  /// No description provided for @workspaceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'No workspace information is available.'**
  String get workspaceUnavailable;

  /// No description provided for @selectAccountForWorkspace.
  ///
  /// In en, this message translates to:
  /// **'Select this account to choose its workspace.'**
  String get selectAccountForWorkspace;

  /// No description provided for @accountEmailUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Email address unavailable'**
  String get accountEmailUnavailable;

  /// No description provided for @accountUsage.
  ///
  /// In en, this message translates to:
  /// **'Usage · {plan}'**
  String accountUsage(String plan);

  /// No description provided for @refreshUsage.
  ///
  /// In en, this message translates to:
  /// **'Refresh usage'**
  String get refreshUsage;

  /// No description provided for @ordinaryUsageAvailable.
  ///
  /// In en, this message translates to:
  /// **'Ordinary usage is available.'**
  String get ordinaryUsageAvailable;

  /// No description provided for @ordinaryUsageUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Ordinary usage is currently unavailable.'**
  String get ordinaryUsageUnavailable;

  /// No description provided for @ordinaryUsageUnknown.
  ///
  /// In en, this message translates to:
  /// **'Usage availability could not be determined.'**
  String get ordinaryUsageUnknown;

  /// No description provided for @usageUpdatedAt.
  ///
  /// In en, this message translates to:
  /// **'Updated {time}'**
  String usageUpdatedAt(String time);

  /// No description provided for @usageLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Usage information could not be loaded.'**
  String get usageLoadFailed;

  /// No description provided for @usageFiveHour.
  ///
  /// In en, this message translates to:
  /// **'5-hour'**
  String get usageFiveHour;

  /// No description provided for @usageWeekly.
  ///
  /// In en, this message translates to:
  /// **'Weekly'**
  String get usageWeekly;

  /// No description provided for @usageMonthly.
  ///
  /// In en, this message translates to:
  /// **'Monthly'**
  String get usageMonthly;

  /// No description provided for @workspaceNumbered.
  ///
  /// In en, this message translates to:
  /// **'Workspace {number}'**
  String workspaceNumbered(int number);

  /// No description provided for @quotaResetsAt.
  ///
  /// In en, this message translates to:
  /// **'Resets {time}'**
  String quotaResetsAt(String time);

  /// No description provided for @creditExpiresAt.
  ///
  /// In en, this message translates to:
  /// **'Expires {time}'**
  String creditExpiresAt(String time);

  /// No description provided for @creditGrantedAt.
  ///
  /// In en, this message translates to:
  /// **'Granted {time}'**
  String creditGrantedAt(String time);

  /// No description provided for @noResetCredits.
  ///
  /// In en, this message translates to:
  /// **'No reset credits are available.'**
  String get noResetCredits;

  /// No description provided for @usageUsedPercent.
  ///
  /// In en, this message translates to:
  /// **'{percent}% used'**
  String usageUsedPercent(String percent);

  /// No description provided for @resetCreditCountUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Reset credit count is unavailable.'**
  String get resetCreditCountUnavailable;

  /// No description provided for @resetCreditsAvailable.
  ///
  /// In en, this message translates to:
  /// **'Available reset credits: {count}'**
  String resetCreditsAvailable(int count);

  /// No description provided for @resetCredit.
  ///
  /// In en, this message translates to:
  /// **'Reset credit'**
  String get resetCredit;

  /// No description provided for @statusUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Status unavailable'**
  String get statusUnavailable;

  /// No description provided for @resetCreditDetailsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Reset credit details were not returned.'**
  String get resetCreditDetailsUnavailable;

  /// No description provided for @resetCreditAvailableStatus.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get resetCreditAvailableStatus;

  /// No description provided for @useResetCredit.
  ///
  /// In en, this message translates to:
  /// **'Use credit'**
  String get useResetCredit;

  /// No description provided for @resetCreditRedeeming.
  ///
  /// In en, this message translates to:
  /// **'Using…'**
  String get resetCreditRedeeming;

  /// No description provided for @confirmResetCreditTitle.
  ///
  /// In en, this message translates to:
  /// **'Use this reset credit?'**
  String get confirmResetCreditTitle;

  /// No description provided for @confirmResetCreditMessage.
  ///
  /// In en, this message translates to:
  /// **'A reset credit will be submitted. This action cannot be undone. Continue?'**
  String get confirmResetCreditMessage;

  /// No description provided for @confirmResetCreditAction.
  ///
  /// In en, this message translates to:
  /// **'Use credit'**
  String get confirmResetCreditAction;

  /// No description provided for @resetCreditApplied.
  ///
  /// In en, this message translates to:
  /// **'The usage limit was reset.'**
  String get resetCreditApplied;

  /// No description provided for @resetCreditAlreadyUsed.
  ///
  /// In en, this message translates to:
  /// **'This reset credit has already been used.'**
  String get resetCreditAlreadyUsed;

  /// No description provided for @resetCreditNothingToReset.
  ///
  /// In en, this message translates to:
  /// **'There is no usage limit to reset right now.'**
  String get resetCreditNothingToReset;

  /// No description provided for @resetCreditNoLongerAvailable.
  ///
  /// In en, this message translates to:
  /// **'This reset credit is no longer available. Refresh usage information.'**
  String get resetCreditNoLongerAvailable;

  /// No description provided for @resetCreditOutcomeUnknown.
  ///
  /// In en, this message translates to:
  /// **'The result could not be confirmed. Refresh usage information before using this credit again.'**
  String get resetCreditOutcomeUnknown;

  /// No description provided for @resetCreditRefreshRequired.
  ///
  /// In en, this message translates to:
  /// **'The previous result could not be confirmed. Refresh usage information before trying again.'**
  String get resetCreditRefreshRequired;

  /// No description provided for @resetCreditRejected.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT did not accept the reset request for this account.'**
  String get resetCreditRejected;

  /// No description provided for @resetCreditSignInRequired.
  ///
  /// In en, this message translates to:
  /// **'Sign in to your ChatGPT account again, then retry.'**
  String get resetCreditSignInRequired;

  /// No description provided for @noChatGptConnections.
  ///
  /// In en, this message translates to:
  /// **'No ChatGPT connections yet.'**
  String get noChatGptConnections;

  /// No description provided for @apiKeyConnectionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'API key connection has not been added yet.'**
  String get apiKeyConnectionUnavailable;

  /// No description provided for @oauthConnectionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'OAuth connection has not been added yet.'**
  String get oauthConnectionUnavailable;

  /// No description provided for @titleGenerationTarget.
  ///
  /// In en, this message translates to:
  /// **'Automatic chat titles'**
  String get titleGenerationTarget;

  /// No description provided for @titleGenerationTargetDescription.
  ///
  /// In en, this message translates to:
  /// **'When the conversation account has no different model available, OpenChat can use the account you choose here. It skips title generation when ordinary usage is unavailable.'**
  String get titleGenerationTargetDescription;

  /// No description provided for @titleUseConversationAccount.
  ///
  /// In en, this message translates to:
  /// **'Use the conversation account'**
  String get titleUseConversationAccount;

  /// No description provided for @titleAccountUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Selected title account is unavailable'**
  String get titleAccountUnavailable;

  /// No description provided for @titleWorkspaceHint.
  ///
  /// In en, this message translates to:
  /// **'Choose a workspace for titles'**
  String get titleWorkspaceHint;

  /// No description provided for @titleWorkspaceRequired.
  ///
  /// In en, this message translates to:
  /// **'Choose a workspace before this account can generate titles.'**
  String get titleWorkspaceRequired;

  /// No description provided for @titlePreferenceLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'The title account preference could not be loaded.'**
  String get titlePreferenceLoadFailed;

  /// No description provided for @titlePreferenceSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The title account preference could not be saved.'**
  String get titlePreferenceSaveFailed;

  /// No description provided for @appearance.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get appearance;

  /// No description provided for @themeSettingDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose how the app looks.'**
  String get themeSettingDescription;

  /// No description provided for @conversationWidth.
  ///
  /// In en, this message translates to:
  /// **'Response width'**
  String get conversationWidth;

  /// No description provided for @conversationWidthDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose the line width used for chat responses.'**
  String get conversationWidthDescription;

  /// No description provided for @widthNarrow.
  ///
  /// In en, this message translates to:
  /// **'Narrow'**
  String get widthNarrow;

  /// No description provided for @widthNormal.
  ///
  /// In en, this message translates to:
  /// **'Normal'**
  String get widthNormal;

  /// No description provided for @widthWide.
  ///
  /// In en, this message translates to:
  /// **'Wide'**
  String get widthWide;

  /// No description provided for @conversationTextSize.
  ///
  /// In en, this message translates to:
  /// **'Text size'**
  String get conversationTextSize;

  /// No description provided for @conversationTextSizeDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose the text size used across the app.'**
  String get conversationTextSizeDescription;

  /// No description provided for @textSizeSmall.
  ///
  /// In en, this message translates to:
  /// **'Small'**
  String get textSizeSmall;

  /// No description provided for @textSizeNormal.
  ///
  /// In en, this message translates to:
  /// **'Normal'**
  String get textSizeNormal;

  /// No description provided for @textSizeLarge.
  ///
  /// In en, this message translates to:
  /// **'Large'**
  String get textSizeLarge;

  /// No description provided for @appFont.
  ///
  /// In en, this message translates to:
  /// **'App font'**
  String get appFont;

  /// No description provided for @appFontDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose the typeface used throughout OpenChat.'**
  String get appFontDescription;

  /// No description provided for @appearancePreferenceSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The appearance preference could not be saved. Try again.'**
  String get appearancePreferenceSaveFailed;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'App language'**
  String get language;

  /// No description provided for @languageSettingDescription.
  ///
  /// In en, this message translates to:
  /// **'Choose the language used by the app.'**
  String get languageSettingDescription;

  /// No description provided for @systemLanguage.
  ///
  /// In en, this message translates to:
  /// **'Device language'**
  String get systemLanguage;

  /// No description provided for @englishLanguage.
  ///
  /// In en, this message translates to:
  /// **'English'**
  String get englishLanguage;

  /// No description provided for @turkishLanguage.
  ///
  /// In en, this message translates to:
  /// **'Turkish'**
  String get turkishLanguage;

  /// No description provided for @spanishLanguage.
  ///
  /// In en, this message translates to:
  /// **'Spanish'**
  String get spanishLanguage;

  /// No description provided for @germanLanguage.
  ///
  /// In en, this message translates to:
  /// **'German'**
  String get germanLanguage;

  /// No description provided for @frenchLanguage.
  ///
  /// In en, this message translates to:
  /// **'French'**
  String get frenchLanguage;

  /// No description provided for @languageSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The language preference could not be saved. Try again.'**
  String get languageSaveFailed;

  /// No description provided for @systemTheme.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get systemTheme;

  /// No description provided for @lightTheme.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get lightTheme;

  /// No description provided for @darkTheme.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get darkTheme;

  /// No description provided for @localData.
  ///
  /// In en, this message translates to:
  /// **'Local data'**
  String get localData;

  /// No description provided for @conversationHistory.
  ///
  /// In en, this message translates to:
  /// **'Chat history'**
  String get conversationHistory;

  /// No description provided for @historyDeviceDescription.
  ///
  /// In en, this message translates to:
  /// **'Conversations are stored on this device.'**
  String get historyDeviceDescription;

  /// No description provided for @historyDeviceStatus.
  ///
  /// In en, this message translates to:
  /// **'On this device'**
  String get historyDeviceStatus;

  /// No description provided for @historyCheckingDescription.
  ///
  /// In en, this message translates to:
  /// **'Preparing local chat history.'**
  String get historyCheckingDescription;

  /// No description provided for @historyCheckingStatus.
  ///
  /// In en, this message translates to:
  /// **'Preparing'**
  String get historyCheckingStatus;

  /// No description provided for @historyStorageUnavailableDescription.
  ///
  /// In en, this message translates to:
  /// **'Local chat history could not be opened.'**
  String get historyStorageUnavailableDescription;

  /// No description provided for @historyStorageCorruptDescription.
  ///
  /// In en, this message translates to:
  /// **'The local database is damaged. No schema updates were applied. Restore a verified backup to continue.'**
  String get historyStorageCorruptDescription;

  /// No description provided for @historyStorageBackupFailedDescription.
  ///
  /// In en, this message translates to:
  /// **'OpenChat could not verify a pre-update database backup, so it stopped the update. Check available disk space and retry.'**
  String get historyStorageBackupFailedDescription;

  /// No description provided for @historyStorageUnavailableStatus.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get historyStorageUnavailableStatus;

  /// No description provided for @historyLoading.
  ///
  /// In en, this message translates to:
  /// **'Loading conversations...'**
  String get historyLoading;

  /// No description provided for @historyLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Chat history could not be loaded. Restart the app.'**
  String get historyLoadFailed;

  /// No description provided for @messageHistoryLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Messages for this conversation could not be loaded.'**
  String get messageHistoryLoadFailed;

  /// No description provided for @messageHistoryLoading.
  ///
  /// In en, this message translates to:
  /// **'Loading conversation messages.'**
  String get messageHistoryLoading;

  /// No description provided for @clearConversationHistory.
  ///
  /// In en, this message translates to:
  /// **'Clear all history'**
  String get clearConversationHistory;

  /// No description provided for @clearConversationHistoryDescription.
  ///
  /// In en, this message translates to:
  /// **'Permanently delete chats and messages stored on this device.'**
  String get clearConversationHistoryDescription;

  /// No description provided for @confirmClearHistoryTitle.
  ///
  /// In en, this message translates to:
  /// **'Clear all chat history?'**
  String get confirmClearHistoryTitle;

  /// No description provided for @confirmClearHistoryBody.
  ///
  /// In en, this message translates to:
  /// **'This permanently deletes all chats and messages stored on this device. This action cannot be undone.'**
  String get confirmClearHistoryBody;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @deleteAll.
  ///
  /// In en, this message translates to:
  /// **'Delete all'**
  String get deleteAll;

  /// No description provided for @clearingHistory.
  ///
  /// In en, this message translates to:
  /// **'Deleting...'**
  String get clearingHistory;

  /// No description provided for @clearHistorySucceeded.
  ///
  /// In en, this message translates to:
  /// **'Chat history was deleted.'**
  String get clearHistorySucceeded;

  /// No description provided for @clearHistoryFailed.
  ///
  /// In en, this message translates to:
  /// **'Chat history could not be deleted. Try again.'**
  String get clearHistoryFailed;

  /// No description provided for @themeSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The theme preference could not be saved. Try again.'**
  String get themeSaveFailed;

  /// No description provided for @searchChats.
  ///
  /// In en, this message translates to:
  /// **'Search chats'**
  String get searchChats;

  /// No description provided for @searchChatsHint.
  ///
  /// In en, this message translates to:
  /// **'Search your chats'**
  String get searchChatsHint;

  /// No description provided for @projects.
  ///
  /// In en, this message translates to:
  /// **'Projects'**
  String get projects;

  /// No description provided for @noProjects.
  ///
  /// In en, this message translates to:
  /// **'No projects yet'**
  String get noProjects;

  /// No description provided for @createProject.
  ///
  /// In en, this message translates to:
  /// **'Create project'**
  String get createProject;

  /// No description provided for @projectName.
  ///
  /// In en, this message translates to:
  /// **'Project name'**
  String get projectName;

  /// No description provided for @projectNameRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter a project name.'**
  String get projectNameRequired;

  /// No description provided for @projectFolder.
  ///
  /// In en, this message translates to:
  /// **'Project folder'**
  String get projectFolder;

  /// No description provided for @chooseProjectFolder.
  ///
  /// In en, this message translates to:
  /// **'Choose folder'**
  String get chooseProjectFolder;

  /// No description provided for @projectFolderNotSelected.
  ///
  /// In en, this message translates to:
  /// **'Choose a folder to continue.'**
  String get projectFolderNotSelected;

  /// No description provided for @projectFolderSelectionFailed.
  ///
  /// In en, this message translates to:
  /// **'The folder could not be selected.'**
  String get projectFolderSelectionFailed;

  /// No description provided for @projectCreateFailed.
  ///
  /// In en, this message translates to:
  /// **'The project could not be created.'**
  String get projectCreateFailed;

  /// No description provided for @projectCreated.
  ///
  /// In en, this message translates to:
  /// **'Project created.'**
  String get projectCreated;

  /// No description provided for @projectLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Projects could not be loaded.'**
  String get projectLoadFailed;

  /// No description provided for @projectMoveFailed.
  ///
  /// In en, this message translates to:
  /// **'The chat could not be moved to the project.'**
  String get projectMoveFailed;

  /// No description provided for @pinnedChats.
  ///
  /// In en, this message translates to:
  /// **'Pinned'**
  String get pinnedChats;

  /// No description provided for @noPinnedChats.
  ///
  /// In en, this message translates to:
  /// **'No pinned chats yet'**
  String get noPinnedChats;

  /// No description provided for @noChatsTitle.
  ///
  /// In en, this message translates to:
  /// **'No chats yet'**
  String get noChatsTitle;

  /// No description provided for @noChatsSearchTitle.
  ///
  /// In en, this message translates to:
  /// **'No chats to search'**
  String get noChatsSearchTitle;

  /// No description provided for @showMore.
  ///
  /// In en, this message translates to:
  /// **'Show more'**
  String get showMore;

  /// No description provided for @projectOptions.
  ///
  /// In en, this message translates to:
  /// **'Project options'**
  String get projectOptions;

  /// No description provided for @newProjectConversation.
  ///
  /// In en, this message translates to:
  /// **'Start a new project chat'**
  String get newProjectConversation;

  /// No description provided for @newConversation.
  ///
  /// In en, this message translates to:
  /// **'Start a new conversation'**
  String get newConversation;

  /// No description provided for @conversationTitle.
  ///
  /// In en, this message translates to:
  /// **'New chat'**
  String get conversationTitle;

  /// No description provided for @conversationMemory.
  ///
  /// In en, this message translates to:
  /// **'Conversation memory'**
  String get conversationMemory;

  /// No description provided for @conversationMemoryDescription.
  ///
  /// In en, this message translates to:
  /// **'Review this conversation\'s compacted context and search its older messages.'**
  String get conversationMemoryDescription;

  /// No description provided for @conversationMemoryCurrentConversation.
  ///
  /// In en, this message translates to:
  /// **'Selected conversation: {title}'**
  String conversationMemoryCurrentConversation(String title);

  /// No description provided for @conversationMemoryNoConversation.
  ///
  /// In en, this message translates to:
  /// **'Open a conversation to inspect its memory.'**
  String get conversationMemoryNoConversation;

  /// No description provided for @contextUsageTitle.
  ///
  /// In en, this message translates to:
  /// **'Context usage'**
  String get contextUsageTitle;

  /// No description provided for @contextUsageUsed.
  ///
  /// In en, this message translates to:
  /// **'Usage: about {count} tokens'**
  String contextUsageUsed(String count);

  /// No description provided for @contextUsageSummary.
  ///
  /// In en, this message translates to:
  /// **'~{used} / {limit} tokens ({percent})'**
  String contextUsageSummary(String used, String limit, String percent);

  /// No description provided for @contextUsageModelLimit.
  ///
  /// In en, this message translates to:
  /// **'{count} tokens'**
  String contextUsageModelLimit(String count);

  /// No description provided for @contextUsageNoModelLimit.
  ///
  /// In en, this message translates to:
  /// **'Model limit unknown.'**
  String get contextUsageNoModelLimit;

  /// No description provided for @contextUsageProviderMeasurement.
  ///
  /// In en, this message translates to:
  /// **'Last provider measurement: {count} tokens'**
  String contextUsageProviderMeasurement(String count);

  /// No description provided for @contextUsageInstructionsEstimate.
  ///
  /// In en, this message translates to:
  /// **'Instructions: {count} tokens · {percent}'**
  String contextUsageInstructionsEstimate(String count, String percent);

  /// No description provided for @contextUsageToolDefinitionsEstimate.
  ///
  /// In en, this message translates to:
  /// **'Tool definitions: {count} tokens · {percent}'**
  String contextUsageToolDefinitionsEstimate(String count, String percent);

  /// No description provided for @contextUsageMessagesEstimate.
  ///
  /// In en, this message translates to:
  /// **'Messages: {count} tokens · {percent}'**
  String contextUsageMessagesEstimate(String count, String percent);

  /// No description provided for @contextUsageAttachmentsEstimate.
  ///
  /// In en, this message translates to:
  /// **'Attachments: {count} tokens · {percent}'**
  String contextUsageAttachmentsEstimate(String count, String percent);

  /// No description provided for @contextUsageAttachmentEstimate.
  ///
  /// In en, this message translates to:
  /// **'{name}: {count} tokens · {percent}'**
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  );

  /// No description provided for @contextUsageDraftAttachment.
  ///
  /// In en, this message translates to:
  /// **'Draft · {name}'**
  String contextUsageDraftAttachment(String name);

  /// No description provided for @contextUsageUserMessagesEstimate.
  ///
  /// In en, this message translates to:
  /// **'User: {count} tokens · {percent}'**
  String contextUsageUserMessagesEstimate(String count, String percent);

  /// No description provided for @contextUsageAssistantMessagesEstimate.
  ///
  /// In en, this message translates to:
  /// **'Assistant: {count} tokens · {percent}'**
  String contextUsageAssistantMessagesEstimate(String count, String percent);

  /// No description provided for @contextUsageToolsEstimate.
  ///
  /// In en, this message translates to:
  /// **'Tool use: {count} tokens · {percent}'**
  String contextUsageToolsEstimate(String count, String percent);

  /// No description provided for @contextUsageToolUsageEstimate.
  ///
  /// In en, this message translates to:
  /// **'{name}: {count} tokens · {percent}'**
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  );

  /// No description provided for @contextUsageMemoryEstimate.
  ///
  /// In en, this message translates to:
  /// **'Compacted memory: {count} tokens · {percent}'**
  String contextUsageMemoryEstimate(String count, String percent);

  /// No description provided for @contextUsageDraftEstimate.
  ///
  /// In en, this message translates to:
  /// **'Draft: {count} tokens · {percent}'**
  String contextUsageDraftEstimate(String count, String percent);

  /// No description provided for @contextUsageFreeSpaceEstimate.
  ///
  /// In en, this message translates to:
  /// **'Free space: {count} tokens · {percent}'**
  String contextUsageFreeSpaceEstimate(String count, String percent);

  /// No description provided for @contextUsageOverLimit.
  ///
  /// In en, this message translates to:
  /// **'Model limit exceeded.'**
  String get contextUsageOverLimit;

  /// No description provided for @contextUsageMeasurementUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Memory details could not be loaded.'**
  String get contextUsageMeasurementUnavailable;

  /// No description provided for @contextUsageInstructionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Instruction details could not be loaded.'**
  String get contextUsageInstructionUnavailable;

  /// No description provided for @contextUsageConfigurationUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Instruction and tool definition estimates could not be loaded.'**
  String get contextUsageConfigurationUnavailable;

  /// No description provided for @contextUsageConfigurationLoading.
  ///
  /// In en, this message translates to:
  /// **'Preparing instruction and tool definition estimates…'**
  String get contextUsageConfigurationLoading;

  /// No description provided for @conversationMemorySemanticTitle.
  ///
  /// In en, this message translates to:
  /// **'Semantic search'**
  String get conversationMemorySemanticTitle;

  /// No description provided for @conversationMemorySemanticDescription.
  ///
  /// In en, this message translates to:
  /// **'Download a multilingual model of about 136 MB to find older messages phrased differently.'**
  String get conversationMemorySemanticDescription;

  /// No description provided for @conversationMemorySemanticPrepare.
  ///
  /// In en, this message translates to:
  /// **'Prepare'**
  String get conversationMemorySemanticPrepare;

  /// No description provided for @conversationMemorySemanticChecking.
  ///
  /// In en, this message translates to:
  /// **'Checking local semantic search…'**
  String get conversationMemorySemanticChecking;

  /// No description provided for @conversationMemorySemanticPreparing.
  ///
  /// In en, this message translates to:
  /// **'Downloading and verifying the model…'**
  String get conversationMemorySemanticPreparing;

  /// No description provided for @conversationMemorySemanticDownloadProgress.
  ///
  /// In en, this message translates to:
  /// **'{percent}% downloaded · {downloaded} / {total} MB'**
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  );

  /// No description provided for @conversationMemorySemanticIndexing.
  ///
  /// In en, this message translates to:
  /// **'Building the local archive index…'**
  String get conversationMemorySemanticIndexing;

  /// No description provided for @conversationMemorySemanticCancelling.
  ///
  /// In en, this message translates to:
  /// **'Cancelling the download…'**
  String get conversationMemorySemanticCancelling;

  /// No description provided for @conversationMemorySemanticDownloadCancelled.
  ///
  /// In en, this message translates to:
  /// **'Download cancelled. Keyword search remains available.'**
  String get conversationMemorySemanticDownloadCancelled;

  /// No description provided for @conversationMemorySemanticPrepareFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not prepare the model. Try again; valid downloaded parts will be reused.'**
  String get conversationMemorySemanticPrepareFailed;

  /// No description provided for @conversationMemorySemanticKeywordSearchFallback.
  ///
  /// In en, this message translates to:
  /// **'Keyword search remains available before preparation.'**
  String get conversationMemorySemanticKeywordSearchFallback;

  /// No description provided for @conversationMemorySemanticReady.
  ///
  /// In en, this message translates to:
  /// **'Local semantic search is ready'**
  String get conversationMemorySemanticReady;

  /// No description provided for @conversationMemorySemanticIndexNotice.
  ///
  /// In en, this message translates to:
  /// **'The model runs on this device. The first search may index older messages and saved tool details locally and take longer.'**
  String get conversationMemorySemanticIndexNotice;

  /// No description provided for @conversationMemorySummaryTitle.
  ///
  /// In en, this message translates to:
  /// **'Compacted context'**
  String get conversationMemorySummaryTitle;

  /// No description provided for @conversationMemoryNoSummary.
  ///
  /// In en, this message translates to:
  /// **'There is no compacted summary yet.'**
  String get conversationMemoryNoSummary;

  /// No description provided for @conversationMemoryCheckpointDescription.
  ///
  /// In en, this message translates to:
  /// **'The provider stores context as a reusable checkpoint instead of readable summary text. The full message history remains in the archive.'**
  String get conversationMemoryCheckpointDescription;

  /// No description provided for @conversationMemoryLastPromptTokens.
  ///
  /// In en, this message translates to:
  /// **'Last request · {provider} · {model} · {count} input tokens'**
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  );

  /// No description provided for @conversationMemorySearchTitle.
  ///
  /// In en, this message translates to:
  /// **'Search the archive'**
  String get conversationMemorySearchTitle;

  /// No description provided for @conversationMemorySearchHint.
  ///
  /// In en, this message translates to:
  /// **'Enter an older topic or phrase...'**
  String get conversationMemorySearchHint;

  /// No description provided for @conversationMemorySearchAction.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get conversationMemorySearchAction;

  /// No description provided for @conversationMemorySearchQueryTooShort.
  ///
  /// In en, this message translates to:
  /// **'Enter at least two characters to search.'**
  String get conversationMemorySearchQueryTooShort;

  /// No description provided for @conversationMemorySearchInstruction.
  ///
  /// In en, this message translates to:
  /// **'Completed messages and tool results are searched in this conversation only.'**
  String get conversationMemorySearchInstruction;

  /// No description provided for @conversationMemorySearchNoResults.
  ///
  /// In en, this message translates to:
  /// **'No matching archive entries.'**
  String get conversationMemorySearchNoResults;

  /// No description provided for @conversationMemorySearchFailed.
  ///
  /// In en, this message translates to:
  /// **'The conversation archive could not be searched. Try again.'**
  String get conversationMemorySearchFailed;

  /// No description provided for @conversationMemoryLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Compacted context could not be loaded. Try again.'**
  String get conversationMemoryLoadFailed;

  /// No description provided for @conversationMemoryResetAction.
  ///
  /// In en, this message translates to:
  /// **'Reset context'**
  String get conversationMemoryResetAction;

  /// No description provided for @conversationMemoryResetTitle.
  ///
  /// In en, this message translates to:
  /// **'Reset compacted context?'**
  String get conversationMemoryResetTitle;

  /// No description provided for @conversationMemoryResetConfirmation.
  ///
  /// In en, this message translates to:
  /// **'The saved compacted context and last request measurement will be removed. The full message history and archive will remain; compacted context can be created again during a later request if needed.'**
  String get conversationMemoryResetConfirmation;

  /// No description provided for @conversationMemoryResetConfirm.
  ///
  /// In en, this message translates to:
  /// **'Reset'**
  String get conversationMemoryResetConfirm;

  /// No description provided for @conversationMemoryResetFailed.
  ///
  /// In en, this message translates to:
  /// **'The compacted context could not be reset.'**
  String get conversationMemoryResetFailed;

  /// No description provided for @conversationMemoryUserMessage.
  ///
  /// In en, this message translates to:
  /// **'User message'**
  String get conversationMemoryUserMessage;

  /// No description provided for @conversationMemoryAssistantMessage.
  ///
  /// In en, this message translates to:
  /// **'Assistant response'**
  String get conversationMemoryAssistantMessage;

  /// No description provided for @renameConversation.
  ///
  /// In en, this message translates to:
  /// **'Edit chat title'**
  String get renameConversation;

  /// No description provided for @pinConversation.
  ///
  /// In en, this message translates to:
  /// **'Pin chat'**
  String get pinConversation;

  /// No description provided for @unpinConversation.
  ///
  /// In en, this message translates to:
  /// **'Unpin chat'**
  String get unpinConversation;

  /// No description provided for @conversationTitleRequired.
  ///
  /// In en, this message translates to:
  /// **'Chat title cannot be empty.'**
  String get conversationTitleRequired;

  /// No description provided for @conversationTitleSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Chat title could not be saved.'**
  String get conversationTitleSaveFailed;

  /// No description provided for @conversationModelSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Chat model could not be saved. The previous model is still selected.'**
  String get conversationModelSaveFailed;

  /// No description provided for @moreOptions.
  ///
  /// In en, this message translates to:
  /// **'More options'**
  String get moreOptions;

  /// No description provided for @emptyChatWelcomeTitle.
  ///
  /// In en, this message translates to:
  /// **'How can I help?'**
  String get emptyChatWelcomeTitle;

  /// No description provided for @emptyChatWelcomeBody.
  ///
  /// In en, this message translates to:
  /// **'Ask a question to start chatting.'**
  String get emptyChatWelcomeBody;

  /// No description provided for @switchToDarkMode.
  ///
  /// In en, this message translates to:
  /// **'Switch to dark theme'**
  String get switchToDarkMode;

  /// No description provided for @switchToLightMode.
  ///
  /// In en, this message translates to:
  /// **'Switch to light theme'**
  String get switchToLightMode;

  /// No description provided for @theme.
  ///
  /// In en, this message translates to:
  /// **'Theme'**
  String get theme;

  /// No description provided for @keyboardHint.
  ///
  /// In en, this message translates to:
  /// **'Enter to send · Shift+Enter for a new line'**
  String get keyboardHint;

  /// No description provided for @minimizeWindow.
  ///
  /// In en, this message translates to:
  /// **'Minimize window'**
  String get minimizeWindow;

  /// No description provided for @maximizeWindow.
  ///
  /// In en, this message translates to:
  /// **'Maximize window'**
  String get maximizeWindow;

  /// No description provided for @restoreWindow.
  ///
  /// In en, this message translates to:
  /// **'Restore window'**
  String get restoreWindow;

  /// No description provided for @modelSelection.
  ///
  /// In en, this message translates to:
  /// **'Choose model'**
  String get modelSelection;

  /// No description provided for @noModelConnected.
  ///
  /// In en, this message translates to:
  /// **'No model is connected yet'**
  String get noModelConnected;

  /// No description provided for @noModelConnectedBody.
  ///
  /// In en, this message translates to:
  /// **'Connect a provider and choose one of its models to start a conversation.'**
  String get noModelConnectedBody;

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @reasoning.
  ///
  /// In en, this message translates to:
  /// **'Reasoning'**
  String get reasoning;

  /// No description provided for @reasoningMedium.
  ///
  /// In en, this message translates to:
  /// **'Medium'**
  String get reasoningMedium;

  /// No description provided for @reasoningMinimal.
  ///
  /// In en, this message translates to:
  /// **'Minimal'**
  String get reasoningMinimal;

  /// No description provided for @reasoningLow.
  ///
  /// In en, this message translates to:
  /// **'Low'**
  String get reasoningLow;

  /// No description provided for @reasoningHigh.
  ///
  /// In en, this message translates to:
  /// **'High'**
  String get reasoningHigh;

  /// No description provided for @reasoningExtraHigh.
  ///
  /// In en, this message translates to:
  /// **'Extra high'**
  String get reasoningExtraHigh;

  /// No description provided for @reasoningMax.
  ///
  /// In en, this message translates to:
  /// **'Maximum'**
  String get reasoningMax;

  /// No description provided for @reasoningUltra.
  ///
  /// In en, this message translates to:
  /// **'Ultra'**
  String get reasoningUltra;

  /// No description provided for @reasoningDefault.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get reasoningDefault;

  /// No description provided for @reasoningDefaultHint.
  ///
  /// In en, this message translates to:
  /// **'No custom reasoning level is sent. The provider\'s default behavior is used; the level is not adjusted to task difficulty.'**
  String get reasoningDefaultHint;

  /// No description provided for @stop.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop;

  /// No description provided for @modelsLoading.
  ///
  /// In en, this message translates to:
  /// **'Loading models...'**
  String get modelsLoading;

  /// No description provided for @modelsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Models unavailable'**
  String get modelsUnavailable;

  /// No description provided for @noModelsAvailable.
  ///
  /// In en, this message translates to:
  /// **'No models are currently available for this account.'**
  String get noModelsAvailable;

  /// No description provided for @providerDataUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The provider returned data that OpenChat could not read. Refresh the connection and try again.'**
  String get providerDataUnavailable;

  /// No description provided for @oauthResponseInvalid.
  ///
  /// In en, this message translates to:
  /// **'The sign-in result could not be read. Try connecting again.'**
  String get oauthResponseInvalid;

  /// No description provided for @modelCatalogUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The model list is unavailable. Refresh the provider connection and try again.'**
  String get modelCatalogUnavailable;

  /// No description provided for @messageSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Your message could not be saved to local chat history.'**
  String get messageSaveFailed;

  /// No description provided for @chatHistoryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Chat history could not be updated. Try again.'**
  String get chatHistoryUnavailable;

  /// No description provided for @chatRequestFailed.
  ///
  /// In en, this message translates to:
  /// **'The response could not be completed. Your saved messages are still available.'**
  String get chatRequestFailed;

  /// No description provided for @cachedCatalog.
  ///
  /// In en, this message translates to:
  /// **'cached models'**
  String get cachedCatalog;

  /// No description provided for @attachFile.
  ///
  /// In en, this message translates to:
  /// **'Attach file'**
  String get attachFile;

  /// No description provided for @attachmentsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'File attachments are not available yet.'**
  String get attachmentsUnavailable;

  /// No description provided for @removeAttachment.
  ///
  /// In en, this message translates to:
  /// **'Remove attachment'**
  String get removeAttachment;

  /// No description provided for @previewImage.
  ///
  /// In en, this message translates to:
  /// **'Zoom image'**
  String get previewImage;

  /// No description provided for @attachmentUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This attachment is unavailable.'**
  String get attachmentUnavailable;

  /// No description provided for @attachmentCountExceeded.
  ///
  /// In en, this message translates to:
  /// **'You can attach up to 10 files and 3 images.'**
  String get attachmentCountExceeded;

  /// No description provided for @attachmentFileTooLarge.
  ///
  /// In en, this message translates to:
  /// **'The file exceeds the allowed size limit.'**
  String get attachmentFileTooLarge;

  /// No description provided for @attachmentTotalTooLarge.
  ///
  /// In en, this message translates to:
  /// **'Attachments cannot exceed 14 MB in total.'**
  String get attachmentTotalTooLarge;

  /// No description provided for @unsupportedAttachmentFile.
  ///
  /// In en, this message translates to:
  /// **'This file type is not supported.'**
  String get unsupportedAttachmentFile;

  /// No description provided for @attachmentReadFailed.
  ///
  /// In en, this message translates to:
  /// **'The file could not be read. Select it again and retry.'**
  String get attachmentReadFailed;

  /// No description provided for @attachmentSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'The attachment could not be saved. Select it again and retry.'**
  String get attachmentSaveFailed;

  /// No description provided for @attachmentMustBeUtf8.
  ///
  /// In en, this message translates to:
  /// **'Text attachments must use UTF-8 encoding.'**
  String get attachmentMustBeUtf8;

  /// No description provided for @attachmentInvalidImage.
  ///
  /// In en, this message translates to:
  /// **'The image file format could not be verified.'**
  String get attachmentInvalidImage;

  /// No description provided for @modelDoesNotSupportImages.
  ///
  /// In en, this message translates to:
  /// **'The selected model does not support image attachments.'**
  String get modelDoesNotSupportImages;

  /// No description provided for @messageHint.
  ///
  /// In en, this message translates to:
  /// **'Write a message...'**
  String get messageHint;

  /// No description provided for @sendMessage.
  ///
  /// In en, this message translates to:
  /// **'Send message'**
  String get sendMessage;

  /// No description provided for @send.
  ///
  /// In en, this message translates to:
  /// **'Send'**
  String get send;

  /// No description provided for @welcomeTitle.
  ///
  /// In en, this message translates to:
  /// **'Start a conversation'**
  String get welcomeTitle;

  /// No description provided for @welcomeBody.
  ///
  /// In en, this message translates to:
  /// **'Connect an AI model before starting a conversation.'**
  String get welcomeBody;

  /// No description provided for @historyOpen.
  ///
  /// In en, this message translates to:
  /// **'Open chat history'**
  String get historyOpen;

  /// No description provided for @assistantDisclaimer.
  ///
  /// In en, this message translates to:
  /// **'AI responses can be inaccurate. Check important information.'**
  String get assistantDisclaimer;

  /// No description provided for @modelRequired.
  ///
  /// In en, this message translates to:
  /// **'Connect a model before sending a message.'**
  String get modelRequired;

  /// No description provided for @selectedModelUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This model is no longer available. Choose another model.'**
  String get selectedModelUnavailable;

  /// No description provided for @messageModelUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Model unavailable'**
  String get messageModelUnavailable;

  /// No description provided for @userMessage.
  ///
  /// In en, this message translates to:
  /// **'User'**
  String get userMessage;

  /// No description provided for @copyMessage.
  ///
  /// In en, this message translates to:
  /// **'Copy message'**
  String get copyMessage;

  /// No description provided for @messageCopied.
  ///
  /// In en, this message translates to:
  /// **'Message copied to clipboard.'**
  String get messageCopied;

  /// No description provided for @messageCopyFailed.
  ///
  /// In en, this message translates to:
  /// **'Message could not be copied to clipboard.'**
  String get messageCopyFailed;

  /// No description provided for @today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get today;

  /// No description provided for @unavailableTime.
  ///
  /// In en, this message translates to:
  /// **'—:—'**
  String get unavailableTime;

  /// No description provided for @unavailableValue.
  ///
  /// In en, this message translates to:
  /// **'—'**
  String get unavailableValue;

  /// No description provided for @responseMetadata.
  ///
  /// In en, this message translates to:
  /// **'{rate} tok/s · {tokens} tokens · {time}'**
  String responseMetadata(String rate, String tokens, String time);

  /// No description provided for @responseCompleted.
  ///
  /// In en, this message translates to:
  /// **'Response complete'**
  String get responseCompleted;

  /// No description provided for @responseCompletedWithDuration.
  ///
  /// In en, this message translates to:
  /// **'Response complete · {duration}'**
  String responseCompletedWithDuration(String duration);

  /// No description provided for @reasoningSummary.
  ///
  /// In en, this message translates to:
  /// **'Reasoning summary'**
  String get reasoningSummary;

  /// No description provided for @reasoningSummaryWithDuration.
  ///
  /// In en, this message translates to:
  /// **'Reasoning summary · {duration}'**
  String reasoningSummaryWithDuration(String duration);

  /// No description provided for @reasoningSummaryTooltip.
  ///
  /// In en, this message translates to:
  /// **'A summary provided by the model. The duration is how long it took to arrive in the stream; it is not hidden chain-of-thought data.'**
  String get reasoningSummaryTooltip;

  /// No description provided for @toolRunning.
  ///
  /// In en, this message translates to:
  /// **'Running'**
  String get toolRunning;

  /// No description provided for @toolWaitingForUser.
  ///
  /// In en, this message translates to:
  /// **'Waiting for your answer'**
  String get toolWaitingForUser;

  /// No description provided for @toolCompleted.
  ///
  /// In en, this message translates to:
  /// **'Completed'**
  String get toolCompleted;

  /// No description provided for @toolFailed.
  ///
  /// In en, this message translates to:
  /// **'Failed'**
  String get toolFailed;

  /// No description provided for @toolInput.
  ///
  /// In en, this message translates to:
  /// **'Input'**
  String get toolInput;

  /// No description provided for @toolOutput.
  ///
  /// In en, this message translates to:
  /// **'Output'**
  String get toolOutput;

  /// No description provided for @toolListFiles.
  ///
  /// In en, this message translates to:
  /// **'List files'**
  String get toolListFiles;

  /// No description provided for @toolSearchFiles.
  ///
  /// In en, this message translates to:
  /// **'Search files'**
  String get toolSearchFiles;

  /// No description provided for @toolReadFile.
  ///
  /// In en, this message translates to:
  /// **'Read file'**
  String get toolReadFile;

  /// No description provided for @toolGetFileInfo.
  ///
  /// In en, this message translates to:
  /// **'Get file information'**
  String get toolGetFileInfo;

  /// No description provided for @toolWriteFile.
  ///
  /// In en, this message translates to:
  /// **'Write to file'**
  String get toolWriteFile;

  /// No description provided for @toolEditFile.
  ///
  /// In en, this message translates to:
  /// **'Edit file'**
  String get toolEditFile;

  /// No description provided for @toolExecuteCommand.
  ///
  /// In en, this message translates to:
  /// **'Execute command'**
  String get toolExecuteCommand;

  /// No description provided for @toolSendTerminalInput.
  ///
  /// In en, this message translates to:
  /// **'Send terminal input'**
  String get toolSendTerminalInput;

  /// No description provided for @toolWebSearch.
  ///
  /// In en, this message translates to:
  /// **'Web search'**
  String get toolWebSearch;

  /// No description provided for @toolReadUrlContent.
  ///
  /// In en, this message translates to:
  /// **'Read webpage'**
  String get toolReadUrlContent;

  /// No description provided for @toolSearchQuery.
  ///
  /// In en, this message translates to:
  /// **'Search query'**
  String get toolSearchQuery;

  /// No description provided for @toolWebSearchNoResults.
  ///
  /// In en, this message translates to:
  /// **'No web results found.'**
  String get toolWebSearchNoResults;

  /// No description provided for @toolWebSearchResultCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 results} one{1 result} other{{count} results}}'**
  String toolWebSearchResultCount(int count);

  /// No description provided for @toolUrl.
  ///
  /// In en, this message translates to:
  /// **'URL'**
  String get toolUrl;

  /// No description provided for @toolReadUrlLength.
  ///
  /// In en, this message translates to:
  /// **'{count} characters'**
  String toolReadUrlLength(int count);

  /// No description provided for @toolOpenUrl.
  ///
  /// In en, this message translates to:
  /// **'Open in browser'**
  String get toolOpenUrl;

  /// No description provided for @toolCopyUrl.
  ///
  /// In en, this message translates to:
  /// **'Copy URL'**
  String get toolCopyUrl;

  /// No description provided for @toolCopyContent.
  ///
  /// In en, this message translates to:
  /// **'Copy content'**
  String get toolCopyContent;

  /// No description provided for @toolCopyFailed.
  ///
  /// In en, this message translates to:
  /// **'The content could not be copied.'**
  String get toolCopyFailed;

  /// No description provided for @toolOperationWorking.
  ///
  /// In en, this message translates to:
  /// **'This operation is in progress.'**
  String get toolOperationWorking;

  /// No description provided for @toolOperationFailed.
  ///
  /// In en, this message translates to:
  /// **'The operation could not be completed.'**
  String get toolOperationFailed;

  /// No description provided for @toolOperationUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The result could not be displayed.'**
  String get toolOperationUnavailable;

  /// No description provided for @toolOperationTruncated.
  ///
  /// In en, this message translates to:
  /// **'Only part of the result is available.'**
  String get toolOperationTruncated;

  /// No description provided for @toolSearchNoMatches.
  ///
  /// In en, this message translates to:
  /// **'No matches found.'**
  String get toolSearchNoMatches;

  /// No description provided for @toolSearchMoreResults.
  ///
  /// In en, this message translates to:
  /// **'More matches are available.'**
  String get toolSearchMoreResults;

  /// No description provided for @toolSearchMatchCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 matches} one{1 match} other{{count} matches}}'**
  String toolSearchMatchCount(int count);

  /// No description provided for @toolReadNoLines.
  ///
  /// In en, this message translates to:
  /// **'There are no lines in this range.'**
  String get toolReadNoLines;

  /// No description provided for @toolReadMoreLines.
  ///
  /// In en, this message translates to:
  /// **'More lines are available.'**
  String get toolReadMoreLines;

  /// No description provided for @toolReadLineCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{0 lines} one{1 line} other{{count} lines}}'**
  String toolReadLineCount(int count);

  /// No description provided for @toolFileTypeFile.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get toolFileTypeFile;

  /// No description provided for @toolFileTypeDirectory.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get toolFileTypeDirectory;

  /// No description provided for @toolWriteSuccess.
  ///
  /// In en, this message translates to:
  /// **'{size} written'**
  String toolWriteSuccess(String size);

  /// No description provided for @toolFilePreview.
  ///
  /// In en, this message translates to:
  /// **'Written content preview'**
  String get toolFilePreview;

  /// No description provided for @toolEditSuccess.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, one{# change applied} other{# changes applied}}'**
  String toolEditSuccess(int count);

  /// No description provided for @toolEditBefore.
  ///
  /// In en, this message translates to:
  /// **'Before'**
  String get toolEditBefore;

  /// No description provided for @toolEditAfter.
  ///
  /// In en, this message translates to:
  /// **'After'**
  String get toolEditAfter;

  /// No description provided for @toolPermissionCommand.
  ///
  /// In en, this message translates to:
  /// **'Command'**
  String get toolPermissionCommand;

  /// No description provided for @toolPermissionTerminalId.
  ///
  /// In en, this message translates to:
  /// **'Terminal ID'**
  String get toolPermissionTerminalId;

  /// No description provided for @toolPermissionInput.
  ///
  /// In en, this message translates to:
  /// **'Terminal input'**
  String get toolPermissionInput;

  /// No description provided for @toolTerminalNoOutput.
  ///
  /// In en, this message translates to:
  /// **'No output produced.'**
  String get toolTerminalNoOutput;

  /// No description provided for @toolTerminalWaitingOutput.
  ///
  /// In en, this message translates to:
  /// **'Waiting for output or input...'**
  String get toolTerminalWaitingOutput;

  /// No description provided for @toolTerminalRunning.
  ///
  /// In en, this message translates to:
  /// **'Running...'**
  String get toolTerminalRunning;

  /// No description provided for @toolTerminalTerminated.
  ///
  /// In en, this message translates to:
  /// **'Terminated'**
  String get toolTerminalTerminated;

  /// No description provided for @toolTerminalWaitingForInput.
  ///
  /// In en, this message translates to:
  /// **'Waiting for input'**
  String get toolTerminalWaitingForInput;

  /// No description provided for @toolTerminalExitCode.
  ///
  /// In en, this message translates to:
  /// **'Exit code: {code}'**
  String toolTerminalExitCode(int code);

  /// No description provided for @toolTerminalCopied.
  ///
  /// In en, this message translates to:
  /// **'Copied to clipboard'**
  String get toolTerminalCopied;

  /// No description provided for @toolTechnicalDetails.
  ///
  /// In en, this message translates to:
  /// **'Details'**
  String get toolTechnicalDetails;

  /// No description provided for @toolFileCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{No items} one{1 item} other{{count} items}}'**
  String toolFileCount(int count);

  /// No description provided for @toolEmptyListing.
  ///
  /// In en, this message translates to:
  /// **'There are no items to show in this folder.'**
  String get toolEmptyListing;

  /// No description provided for @toolListingUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This file list could not be displayed.'**
  String get toolListingUnavailable;

  /// No description provided for @toolListingIncomplete.
  ///
  /// In en, this message translates to:
  /// **'Some items could not be listed.'**
  String get toolListingIncomplete;

  /// No description provided for @toolMoreFilesAvailable.
  ///
  /// In en, this message translates to:
  /// **'More items are available.'**
  String get toolMoreFilesAvailable;

  /// No description provided for @toolDesktopLocation.
  ///
  /// In en, this message translates to:
  /// **'Desktop'**
  String get toolDesktopLocation;

  /// No description provided for @toolProjectLocation.
  ///
  /// In en, this message translates to:
  /// **'Project folder'**
  String get toolProjectLocation;

  /// No description provided for @toolOpenChatLocation.
  ///
  /// In en, this message translates to:
  /// **'Application data folder'**
  String get toolOpenChatLocation;

  /// No description provided for @responseFailed.
  ///
  /// In en, this message translates to:
  /// **'Response failed'**
  String get responseFailed;

  /// No description provided for @responseStopped.
  ///
  /// In en, this message translates to:
  /// **'Response stopped'**
  String get responseStopped;

  /// No description provided for @secondsShort.
  ///
  /// In en, this message translates to:
  /// **'{count} sec'**
  String secondsShort(int count);

  /// No description provided for @usageQuotas.
  ///
  /// In en, this message translates to:
  /// **'Usage Quotas'**
  String get usageQuotas;

  /// No description provided for @usageQuotasDescription.
  ///
  /// In en, this message translates to:
  /// **'View remaining quotas, usage limits, and reset times for all your connected ChatGPT accounts.'**
  String get usageQuotasDescription;

  /// No description provided for @refreshAll.
  ///
  /// In en, this message translates to:
  /// **'Refresh all'**
  String get refreshAll;

  /// No description provided for @noChatGptAccountsForQuota.
  ///
  /// In en, this message translates to:
  /// **'No connected ChatGPT accounts found.'**
  String get noChatGptAccountsForQuota;

  /// No description provided for @noChatGptAccountsForQuotaDescription.
  ///
  /// In en, this message translates to:
  /// **'Connect your ChatGPT account in the Connections tab to view your usage limits and quotas.'**
  String get noChatGptAccountsForQuotaDescription;

  /// No description provided for @goToConnections.
  ///
  /// In en, this message translates to:
  /// **'Go to Connections'**
  String get goToConnections;

  /// No description provided for @activeAccountBadge.
  ///
  /// In en, this message translates to:
  /// **'Active'**
  String get activeAccountBadge;

  /// No description provided for @workspaceQuotaLabel.
  ///
  /// In en, this message translates to:
  /// **'Workspace: {name}'**
  String workspaceQuotaLabel(String name);

  /// No description provided for @modelsPageDescription.
  ///
  /// In en, this message translates to:
  /// **'Find and download models hosted on Hugging Face.'**
  String get modelsPageDescription;

  /// No description provided for @modelSortDownloads.
  ///
  /// In en, this message translates to:
  /// **'Most downloaded'**
  String get modelSortDownloads;

  /// No description provided for @modelSortLikes.
  ///
  /// In en, this message translates to:
  /// **'Most liked'**
  String get modelSortLikes;

  /// No description provided for @modelSortRecentlyUpdated.
  ///
  /// In en, this message translates to:
  /// **'Recently updated'**
  String get modelSortRecentlyUpdated;

  /// No description provided for @modelPreviousPage.
  ///
  /// In en, this message translates to:
  /// **'Previous'**
  String get modelPreviousPage;

  /// No description provided for @modelNextPage.
  ///
  /// In en, this message translates to:
  /// **'Next'**
  String get modelNextPage;

  /// No description provided for @modelPageLabel.
  ///
  /// In en, this message translates to:
  /// **'Page {page}'**
  String modelPageLabel(int page);

  /// No description provided for @modelFormatGguf.
  ///
  /// In en, this message translates to:
  /// **'GGUF · llama.cpp'**
  String get modelFormatGguf;

  /// No description provided for @modelFormatTransformers.
  ///
  /// In en, this message translates to:
  /// **'Transformers · vLLM'**
  String get modelFormatTransformers;

  /// No description provided for @modelFormatExllama.
  ///
  /// In en, this message translates to:
  /// **'ExLlama · EXL3'**
  String get modelFormatExllama;

  /// No description provided for @huggingFaceModelSearchHint.
  ///
  /// In en, this message translates to:
  /// **'Search Hugging Face models'**
  String get huggingFaceModelSearchHint;

  /// No description provided for @modelSearchRefresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh model results'**
  String get modelSearchRefresh;

  /// No description provided for @modelSearchEmpty.
  ///
  /// In en, this message translates to:
  /// **'No models matched this search.'**
  String get modelSearchEmpty;

  /// No description provided for @modelSearchFailed.
  ///
  /// In en, this message translates to:
  /// **'Hugging Face models could not be loaded.'**
  String get modelSearchFailed;

  /// No description provided for @modelSearchUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Hugging Face could not be reached. Check your connection and try again.'**
  String get modelSearchUnavailable;

  /// No description provided for @modelSearchRateLimited.
  ///
  /// In en, this message translates to:
  /// **'Hugging Face is receiving too many requests. Wait a moment and try again.'**
  String get modelSearchRateLimited;

  /// No description provided for @modelSearchInvalidResponse.
  ///
  /// In en, this message translates to:
  /// **'Hugging Face returned model data OpenChat could not read. Try again shortly.'**
  String get modelSearchInvalidResponse;

  /// No description provided for @modelSearchTimedOut.
  ///
  /// In en, this message translates to:
  /// **'Hugging Face took too long to respond. Try again.'**
  String get modelSearchTimedOut;

  /// No description provided for @modelChooseForDetails.
  ///
  /// In en, this message translates to:
  /// **'Choose a model to inspect its files.'**
  String get modelChooseForDetails;

  /// No description provided for @modelDownloadsLabel.
  ///
  /// In en, this message translates to:
  /// **'Downloads'**
  String get modelDownloadsLabel;

  /// No description provided for @modelLikesLabel.
  ///
  /// In en, this message translates to:
  /// **'Likes'**
  String get modelLikesLabel;

  /// No description provided for @modelLicenseLabel.
  ///
  /// In en, this message translates to:
  /// **'License'**
  String get modelLicenseLabel;

  /// No description provided for @modelRevisionLabel.
  ///
  /// In en, this message translates to:
  /// **'Revision'**
  String get modelRevisionLabel;

  /// No description provided for @modelFilesLabel.
  ///
  /// In en, this message translates to:
  /// **'Model files'**
  String get modelFilesLabel;

  /// No description provided for @modelVisionComponentsLabel.
  ///
  /// In en, this message translates to:
  /// **'Vision components'**
  String get modelVisionComponentsLabel;

  /// No description provided for @modelMtpComponentsLabel.
  ///
  /// In en, this message translates to:
  /// **'MTP components'**
  String get modelMtpComponentsLabel;

  /// No description provided for @modelAuxiliaryComponentsLabel.
  ///
  /// In en, this message translates to:
  /// **'Other auxiliary components'**
  String get modelAuxiliaryComponentsLabel;

  /// No description provided for @modelDownloadComponentButton.
  ///
  /// In en, this message translates to:
  /// **'Download this component'**
  String get modelDownloadComponentButton;

  /// No description provided for @modelComponentDownloaded.
  ///
  /// In en, this message translates to:
  /// **'Component downloaded'**
  String get modelComponentDownloaded;

  /// No description provided for @modelShowMoreComponents.
  ///
  /// In en, this message translates to:
  /// **'Show more components'**
  String get modelShowMoreComponents;

  /// No description provided for @modelReadmeLabel.
  ///
  /// In en, this message translates to:
  /// **'Model description'**
  String get modelReadmeLabel;

  /// No description provided for @modelReadmeMissing.
  ///
  /// In en, this message translates to:
  /// **'This model does not have a README.'**
  String get modelReadmeMissing;

  /// No description provided for @modelReadmeAccessDenied.
  ///
  /// In en, this message translates to:
  /// **'Access to this repository is required to view its description.'**
  String get modelReadmeAccessDenied;

  /// No description provided for @modelReadmeTooLarge.
  ///
  /// In en, this message translates to:
  /// **'The README is too large to display.'**
  String get modelReadmeTooLarge;

  /// No description provided for @modelReadmeUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The model description could not be loaded.'**
  String get modelReadmeUnavailable;

  /// No description provided for @modelDownloadOptionsLabel.
  ///
  /// In en, this message translates to:
  /// **'Download options'**
  String get modelDownloadOptionsLabel;

  /// No description provided for @modelDownloadGroupLabel.
  ///
  /// In en, this message translates to:
  /// **'File set'**
  String get modelDownloadGroupLabel;

  /// No description provided for @modelDownloadSizeLabel.
  ///
  /// In en, this message translates to:
  /// **'Size'**
  String get modelDownloadSizeLabel;

  /// No description provided for @modelDownloadButton.
  ///
  /// In en, this message translates to:
  /// **'Download model'**
  String get modelDownloadButton;

  /// No description provided for @modelCancelDownload.
  ///
  /// In en, this message translates to:
  /// **'Cancel download'**
  String get modelCancelDownload;

  /// No description provided for @modelDownloadRunning.
  ///
  /// In en, this message translates to:
  /// **'File {fileIndex} of {fileCount}: {fileName}'**
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount);

  /// No description provided for @modelDownloadComplete.
  ///
  /// In en, this message translates to:
  /// **'Model downloaded and added to Local Models.'**
  String get modelDownloadComplete;

  /// No description provided for @modelDownloadCancelled.
  ///
  /// In en, this message translates to:
  /// **'Model download cancelled. You can resume it later.'**
  String get modelDownloadCancelled;

  /// No description provided for @modelDownloadFailed.
  ///
  /// In en, this message translates to:
  /// **'The model could not be downloaded.'**
  String get modelDownloadFailed;

  /// No description provided for @modelDownloadProgressUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Download progress could not be read.'**
  String get modelDownloadProgressUnavailable;

  /// No description provided for @modelRevisionChanged.
  ///
  /// In en, this message translates to:
  /// **'This model changed on Hugging Face. Reload its files and try again.'**
  String get modelRevisionChanged;

  /// No description provided for @modelDownloadAccessNeeded.
  ///
  /// In en, this message translates to:
  /// **'This repository is gated or private and needs Hugging Face access.'**
  String get modelDownloadAccessNeeded;

  /// No description provided for @modelNoCompatibleFiles.
  ///
  /// In en, this message translates to:
  /// **'No complete compatible model files were found in this repository.'**
  String get modelNoCompatibleFiles;

  /// No description provided for @modelUnknownDownloadSize.
  ///
  /// In en, this message translates to:
  /// **'The file size is unavailable, so this download cannot start safely.'**
  String get modelUnknownDownloadSize;

  /// No description provided for @modelDetailsLoading.
  ///
  /// In en, this message translates to:
  /// **'Loading model files…'**
  String get modelDetailsLoading;

  /// No description provided for @modelNoFiles.
  ///
  /// In en, this message translates to:
  /// **'No compatible files are available for this format.'**
  String get modelNoFiles;

  /// No description provided for @modelGatedBadge.
  ///
  /// In en, this message translates to:
  /// **'Access required'**
  String get modelGatedBadge;

  /// No description provided for @modelPrivateBadge.
  ///
  /// In en, this message translates to:
  /// **'Private'**
  String get modelPrivateBadge;

  /// No description provided for @modelSavedToFolder.
  ///
  /// In en, this message translates to:
  /// **'Downloads use the folder selected for this engine in Settings.'**
  String get modelSavedToFolder;

  /// No description provided for @userQuestionTitle.
  ///
  /// In en, this message translates to:
  /// **'The assistant needs your input'**
  String get userQuestionTitle;

  /// No description provided for @userQuestionRequiredHint.
  ///
  /// In en, this message translates to:
  /// **'Required questions are marked'**
  String get userQuestionRequiredHint;

  /// No description provided for @userQuestionSubmit.
  ///
  /// In en, this message translates to:
  /// **'Send answer'**
  String get userQuestionSubmit;

  /// No description provided for @userQuestionResuming.
  ///
  /// In en, this message translates to:
  /// **'Resuming the assistant'**
  String get userQuestionResuming;

  /// No description provided for @userQuestionUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This question is no longer available. Reload the conversation.'**
  String get userQuestionUnavailable;

  /// No description provided for @userQuestionRequiredValidation.
  ///
  /// In en, this message translates to:
  /// **'Answer each required question to continue.'**
  String get userQuestionRequiredValidation;

  /// No description provided for @userQuestionSubmitFailed.
  ///
  /// In en, this message translates to:
  /// **'Your answer could not be saved. Try again.'**
  String get userQuestionSubmitFailed;

  /// No description provided for @userQuestionRequiredLabel.
  ///
  /// In en, this message translates to:
  /// **'Required'**
  String get userQuestionRequiredLabel;

  /// No description provided for @userQuestionContinue.
  ///
  /// In en, this message translates to:
  /// **'Continue assistant'**
  String get userQuestionContinue;

  /// No description provided for @userQuestionSaved.
  ///
  /// In en, this message translates to:
  /// **'Your answer is saved. Continue when ready.'**
  String get userQuestionSaved;

  /// No description provided for @userQuestionLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'The pending question could not be loaded. Try again.'**
  String get userQuestionLoadFailed;

  /// No description provided for @userQuestionResumeFailed.
  ///
  /// In en, this message translates to:
  /// **'The saved answer is ready, but the assistant could not continue. Try again.'**
  String get userQuestionResumeFailed;

  /// No description provided for @userQuestionNotificationTitle.
  ///
  /// In en, this message translates to:
  /// **'OpenChat is waiting for you'**
  String get userQuestionNotificationTitle;

  /// No description provided for @userQuestionNotificationBody.
  ///
  /// In en, this message translates to:
  /// **'The AI is waiting for your response.'**
  String get userQuestionNotificationBody;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['de', 'en', 'es', 'fr', 'tr'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'es':
      return AppLocalizationsEs();
    case 'fr':
      return AppLocalizationsFr();
    case 'tr':
      return AppLocalizationsTr();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
