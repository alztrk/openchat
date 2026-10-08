// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'OpenChat';

  @override
  String get newChat => 'New chat';

  @override
  String get chats => 'Chats';

  @override
  String get collapseSidebars => 'Collapse sidebars';

  @override
  String get showSidebars => 'Show sidebars';

  @override
  String get home => 'Home';

  @override
  String get extensions => 'Extensions';

  @override
  String get scheduled => 'Scheduled';

  @override
  String get design => 'Design';

  @override
  String get security => 'Security';

  @override
  String get sectionUnavailable => 'This section is not available yet.';

  @override
  String get settings => 'Settings';

  @override
  String get settingsDescription => 'Connections, appearance, and local data';

  @override
  String get connections => 'Connections';

  @override
  String get models => 'Models';

  @override
  String get modelsDescription =>
      'Manage models from connected providers, set a default model, and hide models you don\'t need.';

  @override
  String get localEngines => 'Local engines';

  @override
  String get localEnginesDescription =>
      'Install verified local runtimes, register your own model files, and check whether a runtime is healthy. Move or copy models into an engine folder, or keep them where they are.';

  @override
  String get localEnginesUnavailable =>
      'The local engine service is not available yet.';

  @override
  String get localEnginesLoadFailed =>
      'Local engine information could not be loaded.';

  @override
  String get localEnginesEmpty => 'No local engine releases are available.';

  @override
  String get localEnginesReload => 'Reload';

  @override
  String localEngineRelease(String tag) {
    return 'Release $tag';
  }

  @override
  String get localEngineVariants => 'Packages';

  @override
  String get localEngineStable => 'Stable';

  @override
  String get localEnginePreview => 'Preview';

  @override
  String get localEngineNightly => 'Nightly';

  @override
  String get localEngineRecommended => 'Recommended';

  @override
  String get localEngineAvailable => 'Available';

  @override
  String get localEngineInstalled => 'Installed';

  @override
  String get localEngineNotInstalled => 'Not installed';

  @override
  String get localEngineBlocked => 'Blocked';

  @override
  String get localEngineDeprecated => 'Deprecated';

  @override
  String get localEngineWindowsDeprecatedReason =>
      'New vLLM and ExLlama installs are disabled on Windows. Existing runtime files and model registrations are kept.';

  @override
  String get localEngineUnsupportedPlatform => 'Unsupported platform';

  @override
  String get localEngineHardwareUnavailable => 'Hardware unavailable';

  @override
  String get localEngineDriverUnsupported => 'Update NVIDIA driver';

  @override
  String get localEngineDriverVersionUnavailable =>
      'NVIDIA driver version could not be verified';

  @override
  String get localEngineVllmBlockedReason =>
      'vLLM requires an NVIDIA driver version 580 or newer and an existing Linux environment. On Windows, it uses an existing WSL2 distribution with GPU access.';

  @override
  String get localEngineExllamaBlockedReason =>
      'The ExLlamaV3 runtime is not ready for installation yet. OpenChat must pin and verify the complete TabbyAPI, PyTorch, Triton, Flash Linear Attention, and Python dependency set before offering it.';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Requirements: $requirements';
  }

  @override
  String get localEngineInstall => 'Install';

  @override
  String get localEngineInstalling => 'Preparing installation...';

  @override
  String get localEngineInstallProgress => 'Local engine installation progress';

  @override
  String get localEngineCancelInstall => 'Cancel installation';

  @override
  String get localEngineCancellingInstall => 'Cancelling...';

  @override
  String get localEngineInstallFailed =>
      'The local engine could not be installed. The catalog was refreshed.';

  @override
  String get localEngineHealth => 'Runtime status';

  @override
  String get localEngineExecutable => 'llama-server executable';

  @override
  String get localEngineExecutableDescription =>
      'Choose a llama-server binary for OpenChat to launch registered GGUF models. This is separate from connecting to a server you started yourself.';

  @override
  String get localEngineExecutableChoose => 'Choose executable';

  @override
  String get localEngineExecutableClear => 'Clear selection';

  @override
  String get localEngineExecutableNotConfigured =>
      'No executable selected. OpenChat can use an installed package.';

  @override
  String get localEngineExecutableMissing =>
      'The saved executable was not found. Choose it again.';

  @override
  String get localEngineExecutableInvalid =>
      'Choose an existing llama-server executable.';

  @override
  String get localEngineSettingsFailed =>
      'The llama-server executable setting could not be saved.';

  @override
  String get localEngineExternalServerCheck => 'Check for running llama-server';

  @override
  String get localEngineExternalServerNotConnected =>
      'No user-started server is connected. OpenChat will not start or stop this server.';

  @override
  String get localEngineExternalServerConnecting =>
      'Checking the selected local server...';

  @override
  String get localEngineExternalServerFoundTitle =>
      'Running llama-server found';

  @override
  String localEngineExternalServerFoundDescription(int port) {
    return 'A llama.cpp server is listening on port $port. OpenChat will connect to it and list its models. Disconnecting in OpenChat will not stop the server.';
  }

  @override
  String get localEngineExternalServerNotNow => 'Not now';

  @override
  String get localEngineExternalServerConnect => 'Connect';

  @override
  String localEngineExternalServerConnected(int port, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count models available',
      one: '1 model available',
    );
    return 'Connected on port $port. $_temp0.';
  }

  @override
  String get localEngineManagedModelSection => 'OpenChat-managed models';

  @override
  String localEngineManagedServerModelSection(int port) {
    return 'OpenChat server · 127.0.0.1:$port';
  }

  @override
  String localEngineExternalModelSection(int port) {
    return 'User server · 127.0.0.1:$port';
  }

  @override
  String get localEngineExternalServerDisconnect => 'Disconnect';

  @override
  String get localEngineExternalServerNotFound =>
      'No running llama-server was found.';

  @override
  String get localEngineExternalServerScanFailed =>
      'Running llama-server processes could not be checked.';

  @override
  String get localEngineExternalServerConnectFailed =>
      'Could not connect. Check that llama-server is ready and exposes its local model endpoint.';

  @override
  String get localEngineExternalServerAuthRequired =>
      'This server requires authentication. OpenChat does not read or reuse credentials from other processes.';

  @override
  String get localEngineRunning => 'Running';

  @override
  String get localEngineStopped => 'Stopped';

  @override
  String get localEngineUnhealthy => 'Not responding';

  @override
  String get localEngineUnavailable => 'This engine cannot be started yet.';

  @override
  String get localEngineStartModel => 'Start model';

  @override
  String get localEngineStopModel => 'Stop engine';

  @override
  String get localModels => 'Registered models';

  @override
  String get localModelsEmpty => 'No models are registered for this engine.';

  @override
  String get localModelsPageTitle => 'Local models';

  @override
  String get localModelsPageDescription =>
      'View downloaded and registered local models.';

  @override
  String get localModelsPageEmpty => 'No local models are registered yet.';

  @override
  String get localModelsLoadFailed => 'Local models could not be loaded.';

  @override
  String get localModelsRefresh => 'Refresh';

  @override
  String get localModelsDiscover => 'Discover models';

  @override
  String get localModelAddFile => 'Add model file';

  @override
  String get localModelAddFolder => 'Add model folder';

  @override
  String get localModelStorageChoiceTitle => 'Choose where to keep the model';

  @override
  String localModelStorageChoiceTarget(String folder) {
    return 'Selected model folder: $folder';
  }

  @override
  String get localModelDirectoryTitle => 'Model folder';

  @override
  String get localModelDirectoryDescription =>
      'Choose where models for this engine are stored. Changing this folder does not move models already registered.';

  @override
  String get localModelChooseDirectory => 'Choose folder';

  @override
  String get localModelUseDefaultDirectory => 'Use default';

  @override
  String get localModelScanDirectory => 'Scan folder';

  @override
  String get localModelScanningDirectory => 'Scanning folder...';

  @override
  String get localModelDiscoveryTitle => 'Unregistered models found';

  @override
  String localModelDiscoveryPrompt(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'OpenChat found $count supported models in this folder that are not registered. Register them?',
      one: 'OpenChat found 1 supported model in this folder that is not registered. Register it?',
    );
    return '$_temp0';
  }

  @override
  String get localModelDiscoveryTruncated =>
      'The scan reached its safe limit. Choose a smaller folder to find more models.';

  @override
  String get localModelDiscoveryEmpty =>
      'No new supported models were found in this folder.';

  @override
  String get localModelDiscoveryRegisterAll => 'Register found models';

  @override
  String localModelDiscoveryRegistered(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Registered $count models.',
      one: 'Registered 1 model.',
    );
    return '$_temp0';
  }

  @override
  String localModelDiscoveryPartial(int registered, int total) {
    return 'Registered $registered of $total models. Some models could not be registered.';
  }

  @override
  String get localModelDirectoryUnavailable =>
      'This model folder is unavailable. Choose a folder that exists and that OpenChat can access.';

  @override
  String get localModelDiscoveryFailed =>
      'The model folder could not be scanned. Check access permissions and try again.';

  @override
  String get localModelMoveToFolder => 'Move model to this folder';

  @override
  String get localModelCopyToFolder => 'Copy model to this folder';

  @override
  String get localModelKeepInPlace => 'Keep the model where it is';

  @override
  String get localModelSaving => 'Saving model...';

  @override
  String get localModelTransferError =>
      'The model could not be copied or moved. The original model was left in place.';

  @override
  String get localModelTransferRecoveryError =>
      'The model could not be registered or restored. A complete model copy remains in the OpenChat model folder; select it there to register it.';

  @override
  String get localModelRemove => 'Remove registration';

  @override
  String get localModelRemoveConfirmation =>
      'Only the OpenChat registration will be removed. The model file will stay on disk. Continue?';

  @override
  String get localModelCancelStart => 'Cancel startup';

  @override
  String get localModelStopping => 'Stopping runtime...';

  @override
  String get localModelActionError =>
      'The local model action could not be completed.';

  @override
  String get localModelPathMissing =>
      'The model path could not be found. Move it back or remove its registration.';

  @override
  String get localModelEngineNotReady =>
      'Available after this engine is installed and can run models.';

  @override
  String get localModelInvalid =>
      'The selected file or folder is not a valid model for this engine.';

  @override
  String get localModelPathError =>
      'The selected model file or folder could not be accessed.';

  @override
  String get localModelStoragePathError =>
      'OpenChat could not create its model folders. Check available storage and permissions, then try again.';

  @override
  String get localModelStorageError =>
      'The model registration could not be written to the database.';

  @override
  String get localModelStartError =>
      'The model could not be started. Check the runtime installation and model file.';

  @override
  String get localModelStartTimeout =>
      'The model did not become ready in time. Try a smaller model or check your hardware.';

  @override
  String get localModelRuntimeUnavailable =>
      'The local model stopped responding. Restart it from Settings > Local engines and try again.';

  @override
  String get localModelContextUnavailable =>
      'The local runtime did not report its active context window. Update or reinstall llama.cpp, then try again.';

  @override
  String get localModelInferenceFailed =>
      'The local model could not handle this request. Check its chat template and available memory.';

  @override
  String get localModelSaved => 'Local model registered.';

  @override
  String get localModelRemoved => 'Local model registration removed.';

  @override
  String get localEngineStageDownloading => 'Downloading';

  @override
  String get localEngineStageVerifying => 'Verifying';

  @override
  String get localEngineStageExtracting => 'Extracting';

  @override
  String get localEngineStageRuntimeSetup => 'Setting up the Python runtime';

  @override
  String get localEngineStagePublishing => 'Finishing installation';

  @override
  String get localEngineStageReady => 'Ready';

  @override
  String get defaultModel => 'Default';

  @override
  String get setDefaultModel => 'Set as default';

  @override
  String get clearDefaultModel => 'Remove default';

  @override
  String defaultModelUpdated(String model) {
    return 'Default model updated: $model';
  }

  @override
  String get defaultModelCleared => 'Default model removed.';

  @override
  String get hideModel => 'Hide';

  @override
  String get showModel => 'Show';

  @override
  String get hiddenModel => 'Hidden';

  @override
  String modelHidden(String model) {
    return 'Model hidden: $model';
  }

  @override
  String modelUnhidden(String model) {
    return 'Model shown: $model';
  }

  @override
  String get noModelsFound => 'No models found.';

  @override
  String get refreshModels => 'Refresh models';

  @override
  String get connectedProvidersModels => 'Connected provider models';

  @override
  String get noConnectedProviders =>
      'No providers connected yet. Connect accounts or add API keys in the Connections tab.';

  @override
  String get sharedInstructions => 'Shared instructions';

  @override
  String get sharedInstructionsDescription =>
      'These instructions are sent with every connected provider. File tool access follows the Tool access setting.';

  @override
  String get sharedInstructionsHint =>
      'Describe how you want responses to be written...';

  @override
  String get sharedInstructionsLoadFailed =>
      'Shared instructions could not be loaded. Try again.';

  @override
  String get sharedInstructionsSaveFailed =>
      'Shared instructions could not be saved.';

  @override
  String get sharedInstructionsSaved => 'Shared instructions saved.';

  @override
  String get sharedInstructionsTooLong =>
      'Shared instructions cannot exceed 4096 characters.';

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
      'Add a Google AI Studio API key to use the models available to your account. Free access depends on the model and your quota.';

  @override
  String get groqApiDescription =>
      'Use the models enabled for your account with a Groq API key. Pricing and usage limits vary by plan and model.';

  @override
  String get cerebrasApiDescription =>
      'Use the tool-capable models available to your account with a Cerebras API key. Access, pricing, and limits vary by model and account.';

  @override
  String get openRouterApiDescription =>
      'Only models listed at \$0 for input and output with text and tool support appear here. Actual access and limits can change.';

  @override
  String get mistralApiDescription =>
      'Add a Mistral API key to use the chat models available to your account. Free access, pricing, and usage limits depend on your Mistral plan.';

  @override
  String get geminiUnpaidDataNotice =>
      'On Gemini API\'s free tier, Google may use submitted content to improve its products. Review Google\'s data-use terms before sending sensitive information.';

  @override
  String get providerApiKey => 'API key';

  @override
  String get providerKeySaved => 'API key is saved securely on this device.';

  @override
  String providerKeySavedSuffix(Object suffix) {
    return 'API key ending in ••••$suffix is saved securely on this device.';
  }

  @override
  String get providerNoKey => 'No API key is connected.';

  @override
  String get providerKeyInvalid =>
      'Enter a valid API key for this provider. Do not include spaces; the key can be up to 4096 characters.';

  @override
  String get providerKeyStorageFailed =>
      'The API key could not be read or saved securely.';

  @override
  String get favoriteModels => 'Favorites';

  @override
  String get favoriteModelsEmpty => 'No favorite models yet.';

  @override
  String get modelSearchHint => 'Search models...';

  @override
  String get chatGptFastModeEnabledTooltip =>
      'Fast mode is requested. It can use more subscription credits or cost more per API token.';

  @override
  String get chatGptFastModeDisabledTooltip =>
      'Request Fast mode. It can use more subscription credits or cost more per API token; availability depends on the model.';

  @override
  String get chatGptFastModeUnavailableTooltip =>
      'The selected model does not advertise Fast mode support.';

  @override
  String get chatGptFastModeLoadingTooltip => 'Loading Fast mode preference…';

  @override
  String get chatGptFastModeSettingsLoadFailed =>
      'Fast mode preference could not be loaded.';

  @override
  String get chatGptFastModeSettingsSaveFailed =>
      'Fast mode preference could not be saved.';

  @override
  String get modelSearchNoResults => 'No models match your search.';

  @override
  String get addModelFavorite => 'Add to favorite models';

  @override
  String get removeModelFavorite => 'Remove from favorites';

  @override
  String get openCodeConsole => 'OpenCode Console';

  @override
  String get openCodeConsoleDescription =>
      'Free models work without a key. Add a Console API key for paid models; each request is charged to your Console balance.';

  @override
  String openCodeKeySaved(Object suffix) {
    return 'Console API key ending in ••••$suffix is saved securely on this device.';
  }

  @override
  String get openCodeNoKey => 'No Console API key. Free models are available.';

  @override
  String get openCodeApiKey => 'OpenCode Console API key';

  @override
  String get openCodeKeyInvalid =>
      'Enter a non-empty API key without spaces or line breaks (up to 4096 characters).';

  @override
  String get openCodeKeyStorageFailed =>
      'The Console API key could not be read or saved securely.';

  @override
  String get openCodePaidModel => 'Paid';

  @override
  String get openCodeFreeModel => 'Free';

  @override
  String get modelSourceApi => 'API';

  @override
  String get modelSourceOAuth => 'OAuth';

  @override
  String get openCodeFreeModels => 'Free models';

  @override
  String get openCodeApiModels => 'API models';

  @override
  String modelContextWindow(String value) {
    return 'Context window · $value tokens';
  }

  @override
  String openCodeModelContextWindow(String value) {
    return 'OpenCode catalog (Models.dev) · context: $value tokens';
  }

  @override
  String get add => 'Add';

  @override
  String get edit => 'Edit';

  @override
  String get exportConversation => 'Export conversation';

  @override
  String get deleteConversation => 'Delete conversation';

  @override
  String get archiveConversation => 'Archive conversation';

  @override
  String get restoreConversation => 'Restore conversation';

  @override
  String get archivedChats => 'Archived';

  @override
  String get noArchivedChats => 'No archived conversations.';

  @override
  String get stopResponseBeforeArchive =>
      'Stop the active response before archiving this conversation.';

  @override
  String get conversationArchived => 'Conversation archived.';

  @override
  String get conversationRestored => 'Conversation restored.';

  @override
  String get conversationArchiveFailed =>
      'The conversation could not be archived. Try again.';

  @override
  String get conversationRestoreFailed =>
      'The conversation could not be restored. Try again.';

  @override
  String get confirmDeleteConversationTitle => 'Delete this conversation?';

  @override
  String confirmDeleteConversation(String title) {
    return 'This will permanently delete “$title” and all its messages from this device.';
  }

  @override
  String get stopResponseBeforeDelete =>
      'Stop the active response before deleting this conversation.';

  @override
  String get conversationDeleted => 'Conversation deleted.';

  @override
  String get conversationDeletedFileChangesCleanupFailed =>
      'The conversation was deleted, but its file-change backups could not be removed.';

  @override
  String get conversationHistoryClearedFileChangesCleanupFailed =>
      'Conversation history was cleared, but file-change backups could not be removed.';

  @override
  String get conversationDeleteFailed =>
      'The conversation could not be deleted. Try again.';

  @override
  String get conversationExported =>
      'Conversation exported as a Markdown file.';

  @override
  String get conversationExportFailed =>
      'The conversation could not be exported. Try again.';

  @override
  String get conversationExportProvider => 'Provider';

  @override
  String get conversationExportModel => 'Model';

  @override
  String get conversationExportCreated => 'Created';

  @override
  String get conversationExportStatus => 'Status';

  @override
  String get toolPermissions => 'Tool access';

  @override
  String get toolPermissionsDescription =>
      'Choose where AI file tools can operate and whether each call requires your approval.';

  @override
  String get toolPermissionRequireApproval => 'Ask for approval';

  @override
  String get selectedModelDoesNotSupportToolCalls =>
      'This model does not support tool calls. Tool access settings do not apply to it.';

  @override
  String get selectedModelToolSupportUnknown =>
      'This model does not report tool-call support. OpenChat will try sending tools; the provider may reject the request.';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Ask before each file, web, or terminal call. File access stays in project and OpenChat folders.';

  @override
  String get toolPermissionApproveSafeOperations => 'Approve safe operations';

  @override
  String get toolPermissionApproveSafeOperationsDescription =>
      'Auto-run read-only file calls in project and OpenChat data. Ask before changes, web, and terminal.';

  @override
  String get toolPermissionFullAccess => 'Full access';

  @override
  String get toolPermissionFullAccessDescription =>
      'No prompts. File tools can access any folder; web and terminal tools run without approval.';

  @override
  String get toolPermissionSettingsLoadFailed =>
      'Tool access settings could not be loaded.';

  @override
  String get toolPermissionSettingsSaveFailed =>
      'Tool access settings could not be saved. Try again.';

  @override
  String get toolPermissionRequestTitle => 'Tool permission';

  @override
  String get toolPermissionRequestDescription =>
      'The AI wants to use this tool at the selected location. Permission applies to this call only.';

  @override
  String get toolPermissionRequestExpired =>
      'This tool permission request is no longer active.';

  @override
  String get toolPermissionResponseFailed =>
      'Your choice could not be sent. You can try again.';

  @override
  String get toolPermissionContent => 'Content to write';

  @override
  String get toolPermissionOldText => 'Text to find';

  @override
  String get toolPermissionNewText => 'Replacement text';

  @override
  String get toolPermissionQuery => 'Search text';

  @override
  String get toolPermissionOffset => 'Starting position';

  @override
  String get toolPermissionLimit => 'Maximum results';

  @override
  String get toolPermissionStartLine => 'Starting line';

  @override
  String get toolPermissionLineCount => 'Number of lines';

  @override
  String get toolPermissionIncludeHidden => 'Include hidden files';

  @override
  String get commonYes => 'Yes';

  @override
  String get commonNo => 'No';

  @override
  String get toolPermissionTarget => 'Location to access';

  @override
  String get toolPermissionTool => 'Tool';

  @override
  String get toolPermissionArguments => 'Request details';

  @override
  String get toolPermissionDeny => 'Deny';

  @override
  String get toolPermissionStopResponse => 'Stop response';

  @override
  String get toolPermissionAllowOnce => 'Allow this call';

  @override
  String get toolDenied => 'Denied';

  @override
  String get toolCancelled => 'Cancelled';

  @override
  String get toolAwaitingApproval => 'Waiting for approval';

  @override
  String get conversationExportToolActivity => 'Tool activity';

  @override
  String get responseReplaceFailed =>
      'The new response was saved, but the earlier response could not be replaced.';

  @override
  String get responseRetryNotCompleted =>
      'The new response did not finish. The earlier response was kept.';

  @override
  String get responseRetryCleanupFailed =>
      'The retry could not be cleaned up. Refresh the conversation history.';

  @override
  String get responseInProgress => 'Response in progress';

  @override
  String get responseRetryUnavailable =>
      'This response cannot be retried. Start a new message instead.';

  @override
  String responseVersionCount(int current, int total) {
    return 'Response $current of $total';
  }

  @override
  String get previousResponseVersion => 'Previous response';

  @override
  String get nextResponseVersion => 'Next response';

  @override
  String get providerRateLimited =>
      'The provider reported a usage limit. Try again later.';

  @override
  String get providerAuthenticationRequired =>
      'The provider rejected the request. Check the connection and model access.';

  @override
  String get providerRequestFailed =>
      'The provider could not complete the response. Your saved messages are still available.';

  @override
  String get providerToolRequestRejected =>
      'The provider rejected a request containing tools. Check the model\'s tool support or choose a model that advertises tool use.';

  @override
  String get providerNetworkUnavailable =>
      'The provider could not be reached. Check your connection and try again.';

  @override
  String get contextWindowExceeded =>
      'The conversation is too large for this model\'s context window. Shorten the latest message or choose a model with a larger context window. Your chat history is saved.';

  @override
  String get openCodeFreeTierRestricted =>
      'OpenCode free models are only available within the OpenCode app.';

  @override
  String get apiKey => 'API key';

  @override
  String get apiKeyInputLabel => 'API key';

  @override
  String get apiKeyRequired => 'Enter an API key.';

  @override
  String get apiKeyInvalidFormat =>
      'Enter a valid OpenAI API key. It must start with sk-.';

  @override
  String get apiKeyAlreadySaved => 'This API key is already saved.';

  @override
  String get apiKeySaved => 'API key saved securely on this device.';

  @override
  String get apiKeySaveFailed =>
      'The API key could not be saved securely. Try again.';

  @override
  String get apiKeyLoadFailed => 'Saved API keys could not be loaded.';

  @override
  String get savedApiKey => 'Saved key';

  @override
  String savedApiKeyWithSuffix(String suffix) {
    return 'Key ending in ••••$suffix';
  }

  @override
  String get showApiKey => 'Show API key';

  @override
  String get hideApiKey => 'Hide API key';

  @override
  String get retry => 'Retry';

  @override
  String get save => 'Save';

  @override
  String get saving => 'Saving...';

  @override
  String get oauth => 'OAuth';

  @override
  String get oauthSigningIn => 'Signing in...';

  @override
  String get oauthBrowserWaiting => 'Complete sign-in in the browser.';

  @override
  String get oauthConnectionsLoadFailed =>
      'ChatGPT OAuth connections could not be loaded.';

  @override
  String get oauthSignInFailed =>
      'ChatGPT sign-in could not be completed. Check the browser and try again.';

  @override
  String get oauthConnectionAdded => 'ChatGPT account connected.';

  @override
  String get oldCredentialCleanupFailed =>
      'The account connected, but an older saved credential could not be removed. Restart OpenChat and try again.';

  @override
  String get connectionSelectionFailed =>
      'The ChatGPT account could not be selected.';

  @override
  String get removeChatGptConnection => 'Remove ChatGPT connection';

  @override
  String get removeConnectionAction => 'Remove';

  @override
  String confirmRemoveConnection(String name) {
    return 'Remove $name and its saved credentials? Existing chats and messages will remain on this device.';
  }

  @override
  String get connectionRemoveSucceeded =>
      'Connection removed. Existing chats and messages are still available.';

  @override
  String get connectionRemoveFailed =>
      'The connection could not be removed. Try again.';

  @override
  String get workspaceSelectionFailed =>
      'The ChatGPT workspace could not be selected.';

  @override
  String get chatGptAccount => 'ChatGPT account';

  @override
  String get planUnavailable => 'Plan unavailable';

  @override
  String accountPlan(String plan) {
    return 'Plan: $plan';
  }

  @override
  String get connectionNeedsSignIn => 'Sign in again to use this account.';

  @override
  String get connectionSelected => 'Selected';

  @override
  String get useConnection => 'Use account';

  @override
  String get selectWorkspace => 'Choose a workspace';

  @override
  String get workspaceWithoutName => 'Workspace';

  @override
  String get workspace => 'Workspace';

  @override
  String get workspaceUnavailable => 'No workspace information is available.';

  @override
  String get selectAccountForWorkspace =>
      'Select this account to choose its workspace.';

  @override
  String get accountEmailUnavailable => 'Email address unavailable';

  @override
  String accountUsage(String plan) {
    return 'Usage · $plan';
  }

  @override
  String get refreshUsage => 'Refresh usage';

  @override
  String get ordinaryUsageAvailable => 'Ordinary usage is available.';

  @override
  String get ordinaryUsageUnavailable =>
      'Ordinary usage is currently unavailable.';

  @override
  String get ordinaryUsageUnknown =>
      'Usage availability could not be determined.';

  @override
  String usageUpdatedAt(String time) {
    return 'Updated $time';
  }

  @override
  String get usageLoadFailed => 'Usage information could not be loaded.';

  @override
  String get usageFiveHour => '5-hour';

  @override
  String get usageWeekly => 'Weekly';

  @override
  String get usageMonthly => 'Monthly';

  @override
  String workspaceNumbered(int number) {
    return 'Workspace $number';
  }

  @override
  String quotaResetsAt(String time) {
    return 'Resets $time';
  }

  @override
  String creditExpiresAt(String time) {
    return 'Expires $time';
  }

  @override
  String creditGrantedAt(String time) {
    return 'Granted $time';
  }

  @override
  String get noResetCredits => 'No reset credits are available.';

  @override
  String usageUsedPercent(String percent) {
    return '$percent% used';
  }

  @override
  String get resetCreditCountUnavailable =>
      'Reset credit count is unavailable.';

  @override
  String resetCreditsAvailable(int count) {
    return 'Available reset credits: $count';
  }

  @override
  String get resetCredit => 'Reset credit';

  @override
  String get statusUnavailable => 'Status unavailable';

  @override
  String get resetCreditDetailsUnavailable =>
      'Reset credit details were not returned.';

  @override
  String get resetCreditAvailableStatus => 'Available';

  @override
  String get useResetCredit => 'Use credit';

  @override
  String get resetCreditRedeeming => 'Using…';

  @override
  String get confirmResetCreditTitle => 'Use this reset credit?';

  @override
  String get confirmResetCreditMessage =>
      'A reset credit will be submitted. This action cannot be undone. Continue?';

  @override
  String get confirmResetCreditAction => 'Use credit';

  @override
  String get resetCreditApplied => 'The usage limit was reset.';

  @override
  String get resetCreditAlreadyUsed =>
      'This reset credit has already been used.';

  @override
  String get resetCreditNothingToReset =>
      'There is no usage limit to reset right now.';

  @override
  String get resetCreditNoLongerAvailable =>
      'This reset credit is no longer available. Refresh usage information.';

  @override
  String get resetCreditOutcomeUnknown =>
      'The result could not be confirmed. Refresh usage information before using this credit again.';

  @override
  String get resetCreditRefreshRequired =>
      'The previous result could not be confirmed. Refresh usage information before trying again.';

  @override
  String get resetCreditRejected =>
      'ChatGPT did not accept the reset request for this account.';

  @override
  String get resetCreditSignInRequired =>
      'Sign in to your ChatGPT account again, then retry.';

  @override
  String get noChatGptConnections => 'No ChatGPT connections yet.';

  @override
  String get apiKeyConnectionUnavailable =>
      'API key connection has not been added yet.';

  @override
  String get oauthConnectionUnavailable =>
      'OAuth connection has not been added yet.';

  @override
  String get titleGenerationTarget => 'Automatic chat titles';

  @override
  String get titleGenerationTargetDescription =>
      'When the conversation account has no different model available, OpenChat can use the account you choose here. It skips title generation when ordinary usage is unavailable.';

  @override
  String get titleUseConversationAccount => 'Use the conversation account';

  @override
  String get titleAccountUnavailable => 'Selected title account is unavailable';

  @override
  String get titleWorkspaceHint => 'Choose a workspace for titles';

  @override
  String get titleWorkspaceRequired =>
      'Choose a workspace before this account can generate titles.';

  @override
  String get titlePreferenceLoadFailed =>
      'The title account preference could not be loaded.';

  @override
  String get titlePreferenceSaveFailed =>
      'The title account preference could not be saved.';

  @override
  String get appearance => 'Appearance';

  @override
  String get themeSettingDescription => 'Choose how the app looks.';

  @override
  String get conversationWidth => 'Response width';

  @override
  String get conversationWidthDescription =>
      'Choose the line width used for chat responses.';

  @override
  String get widthNarrow => 'Narrow';

  @override
  String get widthNormal => 'Normal';

  @override
  String get widthWide => 'Wide';

  @override
  String get conversationTextSize => 'Text size';

  @override
  String get conversationTextSizeDescription =>
      'Choose the text size used across the app.';

  @override
  String get textSizeSmall => 'Small';

  @override
  String get textSizeNormal => 'Normal';

  @override
  String get textSizeLarge => 'Large';

  @override
  String get appFont => 'App font';

  @override
  String get appFontDescription =>
      'Choose the typeface used throughout OpenChat.';

  @override
  String get appearancePreferenceSaveFailed =>
      'The appearance preference could not be saved. Try again.';

  @override
  String get language => 'App language';

  @override
  String get languageSettingDescription =>
      'Choose the language used by the app.';

  @override
  String get systemLanguage => 'Device language';

  @override
  String get englishLanguage => 'English';

  @override
  String get turkishLanguage => 'Turkish';

  @override
  String get spanishLanguage => 'Spanish';

  @override
  String get germanLanguage => 'German';

  @override
  String get frenchLanguage => 'French';

  @override
  String get languageSaveFailed =>
      'The language preference could not be saved. Try again.';

  @override
  String get systemTheme => 'System';

  @override
  String get lightTheme => 'Light';

  @override
  String get darkTheme => 'Dark';

  @override
  String get localData => 'Local data';

  @override
  String get conversationArchiveTitle => 'Conversation archives';

  @override
  String get conversationArchiveDescription =>
      'Create or restore an encrypted archive of selected conversations on this device.';

  @override
  String get conversationArchiveIncludesNotice =>
      'Archives preserve selected messages, tool inputs and outputs, reasoning summaries, per-chat memory settings and attached files. Chat text may contain sensitive information or local paths. Provider credentials, linked accounts, project links and global settings are excluded.';

  @override
  String get conversationArchiveUnavailable =>
      'The local archive service or chat history is unavailable.';

  @override
  String get exportConversations => 'Export conversations';

  @override
  String get importConversations => 'Import archive';

  @override
  String get conversationArchiveNoConversations =>
      'There are no conversations to export.';

  @override
  String get conversationArchiveSelectTitle => 'Choose conversations to export';

  @override
  String get conversationArchiveSearch => 'Search conversations';

  @override
  String conversationArchiveSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversations selected',
      one: '# conversation selected',
      zero: 'No conversations selected',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchiveSelectAll => 'Select all shown';

  @override
  String get conversationArchiveDeselectAll => 'Deselect all shown';

  @override
  String get conversationArchiveNoMatches => 'No matching conversations.';

  @override
  String get conversationArchivePassphrase => 'Archive passphrase';

  @override
  String get conversationArchiveConfirmPassphrase => 'Confirm passphrase';

  @override
  String get conversationArchivePassphraseHint => 'At least 12 characters';

  @override
  String get conversationArchivePassphraseTooShort =>
      'Use at least 12 characters.';

  @override
  String get conversationArchivePassphraseTooLong =>
      'The passphrase must be no more than 512 bytes.';

  @override
  String get conversationArchivePassphraseMismatch =>
      'The passphrases do not match.';

  @override
  String get conversationArchivePassphraseRecovery =>
      'Keep this passphrase somewhere safe. OpenChat cannot recover it.';

  @override
  String get conversationArchiveChooseFolder =>
      'Choose where to save the encrypted archive';

  @override
  String get conversationArchiveChooseFile =>
      'Choose an OpenChat conversation archive';

  @override
  String conversationArchiveExportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Encrypted archive created for # conversations.',
      one: 'Encrypted archive created for # conversation.',
    );
    return '$_temp0';
  }

  @override
  String conversationArchiveImportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversations imported.',
      one: '# conversation imported.',
      zero: 'No conversations were imported.',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchivePickerFailed =>
      'The file or folder picker could not be opened.';

  @override
  String get conversationArchiveInvalidFile =>
      'The selected archive has no usable file path.';

  @override
  String get conversationArchiveInvalidResponse =>
      'The archive service returned invalid data.';

  @override
  String get conversationArchiveExportFailed =>
      'The conversation archive could not be created.';

  @override
  String get conversationArchiveImportFailed =>
      'The conversation archive could not be imported.';

  @override
  String get profileArchiveTitle => 'Application data backup';

  @override
  String get profileArchiveDescription =>
      'Create or restore an encrypted copy of the conversation database and its referenced attachments.';

  @override
  String get profileArchiveIncludesNotice =>
      'Provider credentials, global preferences and model or runtime files are not included. The current database and attachments are preserved in a recovery folder during restore.';

  @override
  String get profileArchiveUnavailable =>
      'The local profile backup service is unavailable.';

  @override
  String get profileArchiveExport => 'Back up application data';

  @override
  String get profileArchiveRestore => 'Restore application data';

  @override
  String get profileArchiveChooseFolder =>
      'Choose where to save the encrypted profile backup';

  @override
  String get profileArchiveChooseFile => 'Choose an OpenChat profile backup';

  @override
  String profileArchiveExportSuccess(
    String conversationCount,
    String messageCount,
    String attachmentCount,
  ) {
    return 'Encrypted backup created: $conversationCount conversations, $messageCount messages, $attachmentCount attachments.';
  }

  @override
  String get profileArchiveRestoreConfirmTitle => 'Replace application data?';

  @override
  String get profileArchiveRestoreConfirmBody =>
      'The selected backup will replace the conversation database and referenced attachments the next time OpenChat starts. The current database and attachments will be kept in a recovery folder. Provider credentials, global preferences and model or runtime files are not part of the backup.';

  @override
  String get profileArchiveRestoreConfirmButton => 'Restore and close OpenChat';

  @override
  String get profileArchiveRestartTitle => 'Close OpenChat to apply the backup';

  @override
  String profileArchiveRestoreReady(
    int conversationCount,
    int messageCount,
    int attachmentCount,
  ) {
    return 'The backup contains $conversationCount conversations, $messageCount messages and $attachmentCount attachments. Close OpenChat now. The restored data will be checked during startup, and your current profile will remain available for recovery.';
  }

  @override
  String get profileArchiveCloseApp => 'Close OpenChat';

  @override
  String get profileArchiveCloseFailed =>
      'OpenChat could not be closed. Close the window to apply the backup.';

  @override
  String get profileArchiveProcessing =>
      'Encrypting or validating the profile backup. Large backups can take several minutes.';

  @override
  String get profileArchivePickerFailed =>
      'The file or folder picker could not be opened.';

  @override
  String get profileArchiveInvalidFile =>
      'The selected backup has no usable file path.';

  @override
  String get profileArchiveInvalidResponse =>
      'The profile backup service returned invalid data.';

  @override
  String get profileArchiveExportFailed =>
      'The encrypted profile backup could not be created.';

  @override
  String get profileArchiveRestoreFailed =>
      'The profile backup could not be prepared for restore.';

  @override
  String get profileArchiveInvalidArchive =>
      'The backup is invalid or the passphrase is incorrect.';

  @override
  String get profileArchivePassphraseInvalid =>
      'Use a passphrase with at least 12 characters and no more than 512 bytes.';

  @override
  String get profileArchiveNotFound =>
      'The selected profile backup could not be found.';

  @override
  String get profileArchiveConflict =>
      'A profile restore is already pending, or the selected output file already exists.';

  @override
  String get profileArchiveBusy =>
      'Another profile backup operation is in progress.';

  @override
  String get profileArchiveStorageFailed =>
      'The profile backup could not be read, written or safely restored.';

  @override
  String get profileArchiveLimitExceeded =>
      'The profile backup exceeds a supported size or item limit.';

  @override
  String get profileArchiveSchemaUnsupported =>
      'This backup was created by a newer OpenChat database schema.';

  @override
  String get profileArchiveTakingLong =>
      'The profile backup is taking longer than expected. Wait for the operation to finish before trying again.';

  @override
  String get profileArchiveOperationFailed =>
      'The profile backup operation could not be completed.';

  @override
  String get conversationArchivePassphraseTitle => 'Unlock archive';

  @override
  String get conversationArchivePreviewTitle => 'Review archive contents';

  @override
  String get conversationArchiveCreatedAt => 'Created';

  @override
  String get conversationArchiveConversationCount => 'Conversations';

  @override
  String get conversationArchiveMessageCount => 'Messages';

  @override
  String get conversationArchiveAttachmentCount => 'Attachments';

  @override
  String get conversationArchiveDuplicateCount =>
      'Conversations already on this device';

  @override
  String get conversationArchiveSkipDuplicates =>
      'Skip conversations already on this device';

  @override
  String get conversationArchiveImportCopies =>
      'Import duplicates as separate copies';

  @override
  String get conversationArchiveRestoreNotice =>
      'Restored conversations are disconnected from provider accounts and projects. Select a model again before continuing those chats.';

  @override
  String get conversationArchiveInvalidPassphraseOrFile =>
      'The passphrase is incorrect or the archive is invalid.';

  @override
  String get conversationArchivePassphraseInvalid =>
      'The passphrase length is not supported.';

  @override
  String get conversationArchiveNotFound =>
      'The selected archive or conversation could not be found.';

  @override
  String get conversationArchiveConflict =>
      'A destination file already exists. Choose another folder or resolve the existing file first.';

  @override
  String get conversationArchiveBusy =>
      'Stop active assistant runs in the selected chats before exporting them.';

  @override
  String get conversationArchiveStorageFailed =>
      'The archive could not be read, written or restored safely.';

  @override
  String get conversationArchiveLimitExceeded =>
      'The archive is larger than the supported limit.';

  @override
  String get conversationArchiveTakingLong =>
      'The archive operation is taking longer than expected. It may still be running; check the chat list before retrying.';

  @override
  String get conversationArchiveOperationFailed =>
      'The archive operation failed. Check the selected file and available disk space, then try again.';

  @override
  String get conversationArchiveProcessing =>
      'Encrypting or verifying the archive. Larger archives can take several minutes.';

  @override
  String get conversationHistory => 'Chat history';

  @override
  String get historyDeviceDescription =>
      'Conversations are stored on this device.';

  @override
  String get historyDeviceStatus => 'On this device';

  @override
  String get historyCheckingDescription => 'Preparing local chat history.';

  @override
  String get historyCheckingStatus => 'Preparing';

  @override
  String get historyStorageUnavailableDescription =>
      'Local chat history could not be opened.';

  @override
  String get historyStorageCorruptDescription =>
      'The local database is damaged. No schema updates were applied. Restore a verified backup to continue.';

  @override
  String get historyStorageBackupFailedDescription =>
      'OpenChat could not verify a pre-update database backup, so it stopped the update. Check available disk space and retry.';

  @override
  String get historyStorageUnavailableStatus => 'Unavailable';

  @override
  String get historyLoading => 'Loading conversations...';

  @override
  String get historyLoadFailed =>
      'Chat history could not be loaded. Restart the app.';

  @override
  String get messageHistoryLoadFailed =>
      'Messages for this conversation could not be loaded.';

  @override
  String get messageHistoryLoading => 'Loading conversation messages.';

  @override
  String get clearConversationHistory => 'Clear all history';

  @override
  String get clearConversationHistoryDescription =>
      'Permanently delete chats and messages stored on this device.';

  @override
  String get confirmClearHistoryTitle => 'Clear all chat history?';

  @override
  String get confirmClearHistoryBody =>
      'This permanently deletes all chats and messages stored on this device. This action cannot be undone.';

  @override
  String get cancel => 'Cancel';

  @override
  String get continueLabel => 'Continue';

  @override
  String get deleteAll => 'Delete all';

  @override
  String get clearingHistory => 'Deleting...';

  @override
  String get clearHistorySucceeded => 'Chat history was deleted.';

  @override
  String get clearHistoryFailed =>
      'Chat history could not be deleted. Try again.';

  @override
  String get themeSaveFailed =>
      'The theme preference could not be saved. Try again.';

  @override
  String get searchChats => 'Search chats';

  @override
  String get searchChatsHint => 'Search chats and messages';

  @override
  String get searchMessagesTooltip => 'Search messages';

  @override
  String get historySearchDateFilter => 'Filter by date';

  @override
  String get historySearchDateFilterApplied => 'Date filter is active';

  @override
  String get historySearchFiltersTitle => 'Search filters';

  @override
  String get historySearchRouteFilterNote =>
      'Provider and model refer to the route saved with each assistant answer.';

  @override
  String get historySearchProviderFilter => 'Answer provider';

  @override
  String get historySearchModelFilter => 'Answer model';

  @override
  String get historySearchProjectFilter => 'Project';

  @override
  String get historySearchArchiveFilter => 'Archive status';

  @override
  String get historySearchAllProviders => 'All providers';

  @override
  String get historySearchAllModels => 'All models';

  @override
  String get historySearchAllProjects => 'All projects';

  @override
  String get historySearchTagFilter => 'Tag';

  @override
  String get selectConversations => 'Select chats';

  @override
  String get cancelSelection => 'Cancel selection';

  @override
  String get archiveSelectedConversations => 'Archive selected chats';

  @override
  String get moveSelectedChats => 'Move selected chats';

  @override
  String get moveToChats => 'Move to chats';

  @override
  String get bookmarkConversation => 'Bookmark conversation';

  @override
  String get removeConversationBookmark => 'Remove bookmark';

  @override
  String get conversationBookmarkFailed =>
      'The conversation bookmark could not be updated.';

  @override
  String get conversationBookmarked => 'Bookmarked';

  @override
  String selectConversation(String title) {
    return 'Select $title';
  }

  @override
  String conversationsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# chats selected',
      one: '# chat selected',
      zero: 'No chats selected',
    );
    return '$_temp0';
  }

  @override
  String get historySearchAllTags => 'All tags';

  @override
  String get historySearchAllStatuses => 'All conversations';

  @override
  String get historySearchActiveConversations => 'Active conversations';

  @override
  String get historySearchArchivedConversations => 'Archived conversations';

  @override
  String get historySearchClearFilters => 'Clear filters';

  @override
  String get historySearchApplyFilters => 'Apply filters';

  @override
  String get historySearchOpenFilters => 'Open search filters';

  @override
  String get historySearchFiltersActive => 'Search filters are active';

  @override
  String get historySearchClearDateFilter => 'Clear date filter';

  @override
  String get searchMessagesHeader => 'Message matches';

  @override
  String get searchMessagesLoading => 'Searching messages…';

  @override
  String get searchMessagesNoResults => 'No message matches';

  @override
  String get searchMessagesTooShort =>
      'Enter at least two characters to search messages.';

  @override
  String get searchMessagesTooLong =>
      'Search text must be 512 characters or fewer.';

  @override
  String get searchMessagesFailed =>
      'Messages could not be searched. Try again.';

  @override
  String get searchMessageUnavailable => 'This message is no longer available.';

  @override
  String get projects => 'Projects';

  @override
  String get noProjects => 'No projects yet';

  @override
  String get createProject => 'Create project';

  @override
  String get projectName => 'Project name';

  @override
  String get projectNameRequired => 'Enter a project name.';

  @override
  String get projectFolder => 'Project folder';

  @override
  String get chooseProjectFolder => 'Choose folder';

  @override
  String get projectFolderNotSelected => 'Choose a folder to continue.';

  @override
  String get projectFolderSelectionFailed =>
      'The folder could not be selected.';

  @override
  String get projectCreateFailed => 'The project could not be created.';

  @override
  String get projectCreated => 'Project created.';

  @override
  String get projectLoadFailed => 'Projects could not be loaded.';

  @override
  String get projectMoveFailed => 'The chat could not be moved to the project.';

  @override
  String get pinnedChats => 'Pinned';

  @override
  String get noPinnedChats => 'No pinned chats yet';

  @override
  String get noChatsTitle => 'No chats yet';

  @override
  String get noChatsSearchTitle => 'No chats to search';

  @override
  String get showMore => 'Show more';

  @override
  String get projectOptions => 'Project options';

  @override
  String get projectToolRulesTitle => 'Project tool permissions';

  @override
  String get projectToolRulesDescription =>
      'Choose a rule for each tool. Unset tools use the global permission mode. Allow still follows file scope and process sandbox limits.';

  @override
  String get projectToolRuleInherit => 'Use global setting';

  @override
  String get projectToolRuleAsk => 'Ask for approval';

  @override
  String get projectToolRuleAllow => 'Allow';

  @override
  String get projectToolRuleDeny => 'Deny';

  @override
  String get projectToolRulesLoadFailed =>
      'Project tool permissions could not be loaded. Check the saved settings before sending another request.';

  @override
  String get projectToolRulesSaveFailed =>
      'Project tool permissions could not be saved. Your previous rules are still active.';

  @override
  String get projectOptionsTitle => 'Project options';

  @override
  String get projectOptionsToolPermissions => 'Tool permissions';

  @override
  String get projectOptionsWorktrees => 'Git worktrees';

  @override
  String get projectWorktreesTitle => 'Isolated worktrees';

  @override
  String get projectWorktreesDescription =>
      'Create a branch from the current commit in a separate folder. Uncommitted changes are not copied.';

  @override
  String get projectWorktreesLoading => 'Loading worktrees…';

  @override
  String get projectWorktreesEmpty =>
      'No worktrees have been created for this project.';

  @override
  String get projectWorktreeCreate => 'Create worktree';

  @override
  String get projectWorktreeCreateFailed =>
      'A worktree could not be created. Check that this folder is a Git repository.';

  @override
  String get projectWorktreeLoadFailed =>
      'The project worktrees could not be loaded.';

  @override
  String get projectWorktreeOperationFailed =>
      'The Git operation failed. Check the repository state and try again.';

  @override
  String get projectWorktreeNotRepository =>
      'This project folder is not inside a Git repository.';

  @override
  String projectWorktreeBranch(String branch) {
    return 'Branch: $branch';
  }

  @override
  String projectWorktreePath(String path) {
    return 'Folder: $path';
  }

  @override
  String get projectWorktreeStatusClean => 'No uncommitted changes';

  @override
  String projectWorktreeStatusChanges(int count) {
    return 'Changed files: $count';
  }

  @override
  String get projectWorktreeReview => 'Review changes';

  @override
  String get projectWorktreeUse => 'Use as project';

  @override
  String get projectWorktreeRemove => 'Remove worktree';

  @override
  String get projectWorktreeRemoveTitle => 'Remove worktree?';

  @override
  String projectWorktreeRemoveDescription(String branch) {
    return 'This discards uncommitted and untracked files in $branch. The branch and its commits are kept.';
  }

  @override
  String get projectWorktreeReviewTitle => 'Worktree changes';

  @override
  String get projectWorktreeNoChanges =>
      'There are no uncommitted changes. Committed changes are on this branch.';

  @override
  String get projectWorktreeStagedDiff => 'Staged changes';

  @override
  String get projectWorktreeUnstagedDiff => 'Unstaged changes';

  @override
  String get projectWorktreeListTruncated =>
      'Only the first 20 worktrees are shown.';

  @override
  String get projectWorktreeCheckFailed =>
      'The worktree could not be inspected.';

  @override
  String get projectWorktreeRunCheck => 'Run check';

  @override
  String get projectWorktreeTaskPickerTitle => 'Choose a named check';

  @override
  String get projectWorktreeTaskEmpty =>
      'This worktree has no named checks. Add tasks to `.openchat/tasks.json` in the project.';

  @override
  String get projectWorktreeTaskLoadFailed =>
      'Named checks could not be loaded from this worktree.';

  @override
  String get projectWorktreeTaskConfirmationTitle => 'Run this check?';

  @override
  String get projectWorktreeTaskCommand => 'Command';

  @override
  String projectWorktreeTaskTimeout(int seconds) {
    return 'Time limit: $seconds seconds';
  }

  @override
  String get projectWorktreeTaskRun => 'Run check';

  @override
  String projectWorktreeTaskRunning(String task) {
    return 'Check running: $task';
  }

  @override
  String get projectWorktreeTaskStopping => 'Stopping check…';

  @override
  String get projectWorktreeTaskStop => 'Stop check';

  @override
  String get projectWorktreeTaskCancelled => 'The check was cancelled.';

  @override
  String get projectWorktreeTaskTimedOut => 'The check reached its time limit.';

  @override
  String projectWorktreeTaskExitCode(int code) {
    return 'Check finished with exit code $code.';
  }

  @override
  String get projectWorktreeTaskExitCodeUnavailable =>
      'The check finished without an exit code.';

  @override
  String get projectWorktreeTaskOutputTruncated =>
      'The output is truncated to the first 128 KiB.';

  @override
  String get projectWorktreeTaskRunFailed =>
      'The check could not run in the worktree sandbox.';

  @override
  String projectWorktreeTaskResultTitle(String task) {
    return 'Check result: $task';
  }

  @override
  String get projectWorktreeTaskOutput => 'Output';

  @override
  String get projectWorktreeTaskNoOutput => 'The check did not produce output.';

  @override
  String get projectWorktreeTaskDenied =>
      'Project permissions deny running named checks.';

  @override
  String get agentRunManagerTitle => 'Runs';

  @override
  String get agentRunManagerDescription =>
      'Review active, paused, or interrupted work across conversations.';

  @override
  String get agentRunLoading => 'Loading runs';

  @override
  String get agentRunLoadFailed => 'Run status could not be loaded.';

  @override
  String get agentRunEmpty =>
      'There are no active, paused, or interrupted runs.';

  @override
  String get agentRunRefresh => 'Refresh run list';

  @override
  String get agentRunOpenConversation => 'Open chat';

  @override
  String get agentRunStatusRunning => 'Running';

  @override
  String get agentRunStatusPaused => 'Paused';

  @override
  String get agentRunStatusInterrupted => 'Interrupted';

  @override
  String get agentRunStatusUnavailable => 'Status unavailable';

  @override
  String get agentRunStatusCompleted => 'Completed';

  @override
  String get agentRunStatusFailed => 'Failed';

  @override
  String get agentRunStatusCancelled => 'Cancelled';

  @override
  String get agentRunSubagent => 'Child run';

  @override
  String agentRunSubagentTask(String objective) {
    return 'Child task: $objective';
  }

  @override
  String get agentRunLiveStarting => 'Starting delegated analysis…';

  @override
  String get agentRunLiveThinking => 'Analyzing delegated task…';

  @override
  String agentRunLiveUsingTool(String tool) {
    return 'Using $tool';
  }

  @override
  String get agentRunEndedInAnotherChat => 'A run in another chat ended.';

  @override
  String get newProjectConversation => 'Start a new project chat';

  @override
  String get newConversation => 'Start a new conversation';

  @override
  String get conversationTitle => 'New chat';

  @override
  String get conversationMemory => 'Conversation memory';

  @override
  String get conversationMemoryDescription =>
      'Review this conversation\'s compacted context and search its older messages.';

  @override
  String conversationMemoryCurrentConversation(String title) {
    return 'Selected conversation: $title';
  }

  @override
  String get conversationMemoryNoConversation =>
      'Open a conversation to inspect its memory.';

  @override
  String get contextUsageTitle => 'Context usage';

  @override
  String contextUsageUsed(String count) {
    return 'Usage: about $count tokens';
  }

  @override
  String contextUsageSummary(String used, String limit, String percent) {
    return '~$used / $limit tokens ($percent)';
  }

  @override
  String contextUsageModelLimit(String count) {
    return '$count tokens';
  }

  @override
  String get contextUsageNoModelLimit => 'Model limit unknown.';

  @override
  String contextUsageProviderMeasurement(String count) {
    return 'Last provider measurement: $count tokens';
  }

  @override
  String contextUsageInstructionsEstimate(String count, String percent) {
    return 'Instructions: $count tokens · $percent';
  }

  @override
  String contextUsageToolDefinitionsEstimate(String count, String percent) {
    return 'Tool definitions: $count tokens · $percent';
  }

  @override
  String contextUsageMessagesEstimate(String count, String percent) {
    return 'Messages: $count tokens · $percent';
  }

  @override
  String contextUsageAttachmentsEstimate(String count, String percent) {
    return 'Attachments: $count tokens · $percent';
  }

  @override
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count tokens · $percent';
  }

  @override
  String contextUsageDraftAttachment(String name) {
    return 'Draft · $name';
  }

  @override
  String contextUsageUserMessagesEstimate(String count, String percent) {
    return 'User: $count tokens · $percent';
  }

  @override
  String contextUsageAssistantMessagesEstimate(String count, String percent) {
    return 'Assistant: $count tokens · $percent';
  }

  @override
  String contextUsageToolsEstimate(String count, String percent) {
    return 'Tool use: $count tokens · $percent';
  }

  @override
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count tokens · $percent';
  }

  @override
  String contextUsageMemoryEstimate(String count, String percent) {
    return 'Compacted memory: $count tokens · $percent';
  }

  @override
  String contextUsageDraftEstimate(String count, String percent) {
    return 'Draft: $count tokens · $percent';
  }

  @override
  String contextUsageFreeSpaceEstimate(String count, String percent) {
    return 'Free space: $count tokens · $percent';
  }

  @override
  String get contextUsageOverLimit => 'Model limit exceeded.';

  @override
  String get contextUsageMeasurementUnavailable =>
      'Memory details could not be loaded.';

  @override
  String get contextUsageInstructionUnavailable =>
      'Instruction details could not be loaded.';

  @override
  String get contextUsageConfigurationUnavailable =>
      'Instruction and tool definition estimates could not be loaded.';

  @override
  String get contextUsageConfigurationLoading =>
      'Preparing instruction and tool definition estimates…';

  @override
  String get conversationMemorySemanticTitle => 'Semantic search';

  @override
  String get conversationMemorySemanticDescription =>
      'Download a multilingual model of about 136 MB to find older messages phrased differently.';

  @override
  String get conversationMemorySemanticPrepare => 'Prepare';

  @override
  String get conversationMemorySemanticChecking =>
      'Checking local semantic search…';

  @override
  String get conversationMemorySemanticPreparing =>
      'Downloading and verifying the model…';

  @override
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  ) {
    return '$percent% downloaded · $downloaded / $total MB';
  }

  @override
  String get conversationMemorySemanticIndexing =>
      'Building the local archive index…';

  @override
  String get conversationMemorySemanticCancelling => 'Cancelling the download…';

  @override
  String get conversationMemorySemanticDownloadCancelled =>
      'Download cancelled. Keyword search remains available.';

  @override
  String get conversationMemorySemanticPrepareFailed =>
      'Could not prepare the model. Try again; valid downloaded parts will be reused.';

  @override
  String get conversationMemorySemanticKeywordSearchFallback =>
      'Keyword search remains available before preparation.';

  @override
  String get conversationMemorySemanticReady =>
      'Local semantic search is ready';

  @override
  String get conversationMemorySemanticIndexNotice =>
      'The model runs on this device. The first search may index older messages and saved tool details locally and take longer.';

  @override
  String get conversationMemorySummaryTitle => 'Compacted context';

  @override
  String get conversationMemoryNoSummary =>
      'There is no compacted summary yet.';

  @override
  String get conversationMemoryCheckpointDescription =>
      'The provider stores context as a reusable checkpoint instead of readable summary text. The full message history remains in the archive.';

  @override
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  ) {
    return 'Last request · $provider · $model · $count input tokens';
  }

  @override
  String get conversationMemorySearchTitle => 'Search the archive';

  @override
  String get conversationMemorySearchHint =>
      'Enter an older topic or phrase...';

  @override
  String get conversationMemorySearchAction => 'Search';

  @override
  String get conversationMemorySearchQueryTooShort =>
      'Enter at least two characters to search.';

  @override
  String get conversationMemorySearchInstruction =>
      'Completed messages and tool results are searched in this conversation only.';

  @override
  String get conversationMemoryArchiveSettingsTitle => 'Archive indexing';

  @override
  String get conversationMemoryArchiveSettingsDescription =>
      'Choose which saved conversation text can be added to local archive search. Turning an item off deletes its derived search and semantic index data; the original conversation stays saved.';

  @override
  String get conversationMemoryArchiveIncludeConversation =>
      'Include this conversation';

  @override
  String get conversationMemoryArchiveConversationIncluded =>
      'Messages and included tool results can appear in archive search.';

  @override
  String get conversationMemoryArchiveConversationExcluded =>
      'This conversation\'s derived archive indexes are removed and it is excluded from future indexing.';

  @override
  String get conversationMemoryArchiveToolsTitle => 'Tool results';

  @override
  String get conversationMemoryArchiveToolIncluded =>
      'Saved details from this tool can appear in archive search.';

  @override
  String get conversationMemoryArchiveToolExcluded =>
      'This tool\'s saved details are removed from archive indexes.';

  @override
  String get conversationMemoryArchiveNoTools =>
      'No completed tool results are saved in this conversation.';

  @override
  String get conversationMemoryArchiveSettingsSaveFailed =>
      'Archive indexing settings could not be saved. Try again.';

  @override
  String get conversationMemorySearchNoResults =>
      'No matching archive entries.';

  @override
  String get conversationMemorySearchFailed =>
      'The conversation archive could not be searched. Try again.';

  @override
  String get conversationMemoryLoadFailed =>
      'Compacted context could not be loaded. Try again.';

  @override
  String get conversationMemoryResetAction => 'Reset context';

  @override
  String get conversationMemoryResetTitle => 'Reset compacted context?';

  @override
  String get conversationMemoryResetConfirmation =>
      'The saved compacted context and last request measurement will be removed. The full message history and archive will remain; compacted context can be created again during a later request if needed.';

  @override
  String get conversationMemoryResetConfirm => 'Reset';

  @override
  String get conversationMemoryResetFailed =>
      'The compacted context could not be reset.';

  @override
  String get conversationMemoryUserMessage => 'User message';

  @override
  String get conversationMemoryAssistantMessage => 'Assistant response';

  @override
  String get renameConversation => 'Edit chat title';

  @override
  String get editConversationTags => 'Edit tags';

  @override
  String get conversationTagsDialogTitle => 'Conversation tags';

  @override
  String get conversationTagsFieldLabel => 'Tags';

  @override
  String get conversationTagsFieldHint => 'Separate tags with commas';

  @override
  String get conversationTagsHelp =>
      'Use up to 12 tags, with 32 characters per tag.';

  @override
  String get conversationTagsSaveFailed => 'Tags could not be saved.';

  @override
  String get saveHistorySearchTitle => 'Save history search';

  @override
  String get savedHistorySearchName => 'Search name';

  @override
  String get savedHistorySearchesTitle => 'Saved searches';

  @override
  String get savedHistorySearchesEmpty => 'No saved searches yet.';

  @override
  String get deleteSavedHistorySearch => 'Delete saved search';

  @override
  String get saveCurrentHistorySearch => 'Save current search';

  @override
  String get savedHistorySearchesLoadFailed =>
      'Saved searches could not be loaded.';

  @override
  String get savedHistorySearchSaveFailed =>
      'Saved searches could not be updated.';

  @override
  String get savedHistorySearchLimitReached =>
      'You can save up to 20 searches.';

  @override
  String get pinConversation => 'Pin chat';

  @override
  String get unpinConversation => 'Unpin chat';

  @override
  String get conversationTitleRequired => 'Chat title cannot be empty.';

  @override
  String get conversationBranchEditTitle => 'Edit message and start a branch';

  @override
  String get conversationBranchEditLabel => 'Message';

  @override
  String get conversationBranchStart => 'Start branch';

  @override
  String conversationBranchTitle(String title) {
    return '$title (branch)';
  }

  @override
  String get conversationBranchCreateFailed =>
      'The message branch could not be created.';

  @override
  String get conversationTitleSaveFailed => 'Chat title could not be saved.';

  @override
  String get conversationModelSaveFailed =>
      'Chat model could not be saved. The previous model is still selected.';

  @override
  String get moreOptions => 'More options';

  @override
  String get emptyChatWelcomeTitle => 'How can I help?';

  @override
  String get emptyChatWelcomeBody => 'Ask a question to start chatting.';

  @override
  String get switchToDarkMode => 'Switch to dark theme';

  @override
  String get switchToLightMode => 'Switch to light theme';

  @override
  String get theme => 'Theme';

  @override
  String get keyboardHint => 'Enter to send · Shift+Enter for a new line';

  @override
  String get minimizeWindow => 'Minimize window';

  @override
  String get maximizeWindow => 'Maximize window';

  @override
  String get restoreWindow => 'Restore window';

  @override
  String get modelSelection => 'Choose model';

  @override
  String get noModelConnected => 'No model is connected yet';

  @override
  String get noModelConnectedBody =>
      'Connect a provider and choose one of its models to start a conversation.';

  @override
  String get close => 'Close';

  @override
  String get reasoning => 'Reasoning';

  @override
  String get reasoningMedium => 'Medium';

  @override
  String get reasoningMinimal => 'Minimal';

  @override
  String get reasoningLow => 'Low';

  @override
  String get reasoningHigh => 'High';

  @override
  String get reasoningExtraHigh => 'Extra high';

  @override
  String get reasoningMax => 'Maximum';

  @override
  String get reasoningUltra => 'Ultra';

  @override
  String get reasoningDefault => 'Default';

  @override
  String get reasoningDefaultHint =>
      'No custom reasoning level is sent. The provider\'s default behavior is used; the level is not adjusted to task difficulty.';

  @override
  String get stop => 'Stop';

  @override
  String get modelsLoading => 'Loading models...';

  @override
  String get modelsUnavailable => 'Models unavailable';

  @override
  String get noModelsAvailable =>
      'No models are currently available for this account.';

  @override
  String get providerDataUnavailable =>
      'The provider returned data that OpenChat could not read. Refresh the connection and try again.';

  @override
  String get oauthResponseInvalid =>
      'The sign-in result could not be read. Try connecting again.';

  @override
  String get modelCatalogUnavailable =>
      'The model list is unavailable. Refresh the provider connection and try again.';

  @override
  String get messageSaveFailed =>
      'Your message could not be saved to local chat history.';

  @override
  String get chatHistoryUnavailable =>
      'Chat history could not be updated. Try again.';

  @override
  String get chatRequestFailed =>
      'The response could not be completed. Your saved messages are still available.';

  @override
  String get goalAlreadyActive =>
      'Resume or stop the active goal before starting another one in this chat.';

  @override
  String get goalStateUnavailable =>
      'OpenChat could not check whether this chat already has an active goal. Try again.';

  @override
  String get cachedCatalog => 'cached models';

  @override
  String get attachFile => 'Attach file';

  @override
  String get attachmentsUnavailable =>
      'File attachments are not available yet.';

  @override
  String get removeAttachment => 'Remove attachment';

  @override
  String get previewImage => 'Zoom image';

  @override
  String get attachmentUnavailable => 'This attachment is unavailable.';

  @override
  String get attachmentCountExceeded =>
      'You can attach up to 10 files and 3 images.';

  @override
  String get attachmentFileTooLarge =>
      'The file exceeds the allowed size limit.';

  @override
  String get attachmentTotalTooLarge =>
      'Attachments cannot exceed 14 MB in total.';

  @override
  String get unsupportedAttachmentFile => 'This file type is not supported.';

  @override
  String get attachmentReadFailed =>
      'The file could not be read. Select it again and retry.';

  @override
  String get attachmentSaveFailed =>
      'The attachment could not be saved. Select it again and retry.';

  @override
  String get attachmentMustBeUtf8 =>
      'Text attachments must use UTF-8 encoding.';

  @override
  String get attachmentInvalidImage =>
      'The image file format could not be verified.';

  @override
  String get modelDoesNotSupportImages =>
      'The selected model does not support image attachments.';

  @override
  String get messageHint => 'Write a message...';

  @override
  String get sendMessage => 'Send message';

  @override
  String get send => 'Send';

  @override
  String get welcomeTitle => 'Start a conversation';

  @override
  String get welcomeBody =>
      'Connect an AI model before starting a conversation.';

  @override
  String get historyOpen => 'Open chat history';

  @override
  String get assistantDisclaimer =>
      'AI responses can be inaccurate. Check important information.';

  @override
  String get modelRequired => 'Connect a model before sending a message.';

  @override
  String get selectedModelUnavailable =>
      'This model is no longer available. Choose another model.';

  @override
  String get messageModelUnavailable => 'Model unavailable';

  @override
  String get userMessage => 'User';

  @override
  String get copyMessage => 'Copy message';

  @override
  String get messageCopied => 'Message copied to clipboard.';

  @override
  String get messageCopyFailed => 'Message could not be copied to clipboard.';

  @override
  String get today => 'Today';

  @override
  String get unavailableTime => '—:—';

  @override
  String get unavailableValue => '—';

  @override
  String responseTokenRate(String rate) {
    return '$rate tok/s';
  }

  @override
  String responseTokenCount(String count) {
    return '$count tokens';
  }

  @override
  String get responseCompleted => 'Response complete';

  @override
  String responseCompletedWithDuration(String duration) {
    return 'Response complete · $duration';
  }

  @override
  String get reasoningSummary => 'Reasoning summary';

  @override
  String reasoningSummaryWithDuration(String duration) {
    return 'Reasoning summary · $duration';
  }

  @override
  String get reasoningSummaryTooltip =>
      'A summary provided by the model. The duration is how long it took to arrive in the stream; it is not hidden chain-of-thought data.';

  @override
  String get toolRunning => 'Running';

  @override
  String get toolWaitingForUser => 'Waiting for your answer';

  @override
  String get toolCompleted => 'Completed';

  @override
  String get toolFailed => 'Failed';

  @override
  String get toolInput => 'Input';

  @override
  String get toolOutput => 'Output';

  @override
  String get toolListFiles => 'List files';

  @override
  String get toolSearchFiles => 'Search files';

  @override
  String get toolReadFile => 'Read file';

  @override
  String get toolGetFileInfo => 'Get file information';

  @override
  String get toolWriteFile => 'Write to file';

  @override
  String get toolEditFile => 'Edit file';

  @override
  String get toolExecuteCommand => 'Execute command';

  @override
  String get toolRunProjectTask => 'Run project task';

  @override
  String get toolDelegateTask => 'Delegate analysis';

  @override
  String get toolPermissionTask => 'Task';

  @override
  String get toolPermissionTimeout => 'Timeout (seconds)';

  @override
  String get toolSendTerminalInput => 'Send terminal input';

  @override
  String get toolGitStatus => 'Git status';

  @override
  String get toolGitDiff => 'Git diff';

  @override
  String get toolGitHistory => 'Git history';

  @override
  String get toolGitBranch => 'Branch';

  @override
  String get toolGitUpstream => 'Upstream';

  @override
  String get toolGitAhead => 'Ahead';

  @override
  String get toolGitBehind => 'Behind';

  @override
  String get toolGitStaged => 'Staged';

  @override
  String get toolGitUnstaged => 'Unstaged';

  @override
  String get toolGitNoChanges => 'The working tree is clean.';

  @override
  String get toolGitNoDiff => 'There is no diff to show.';

  @override
  String get toolGitNoHistory => 'No commits were found.';

  @override
  String get toolWebSearch => 'Web search';

  @override
  String get toolReadUrlContent => 'Read webpage';

  @override
  String get toolSearchQuery => 'Search query';

  @override
  String get toolLocalWebSource => 'Local web search';

  @override
  String get toolLocalPageSource => 'Local page read';

  @override
  String get toolProviderSource => 'Provider source';

  @override
  String toolSourceRetrievedAt(String time) {
    return 'Retrieved $time';
  }

  @override
  String get toolSourceDetails => 'Source details';

  @override
  String toolCitationSource(String sourceId) {
    return 'Source $sourceId';
  }

  @override
  String get toolWebSearchNoResults => 'No web results found.';

  @override
  String toolWebSearchResultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count results',
      one: '1 result',
      zero: '0 results',
    );
    return '$_temp0';
  }

  @override
  String get toolUrl => 'URL';

  @override
  String toolReadUrlLength(int count) {
    return '$count characters';
  }

  @override
  String get toolOpenUrl => 'Open in browser';

  @override
  String get toolCopyUrl => 'Copy URL';

  @override
  String get toolCopyContent => 'Copy content';

  @override
  String get toolCopyFailed => 'The content could not be copied.';

  @override
  String get toolOperationWorking => 'This operation is in progress.';

  @override
  String get toolOperationFailed => 'The operation could not be completed.';

  @override
  String get toolOperationUnavailable => 'The result could not be displayed.';

  @override
  String get toolOperationTruncated => 'Only part of the result is available.';

  @override
  String get toolSearchNoMatches => 'No matches found.';

  @override
  String get toolSearchMoreResults => 'More matches are available.';

  @override
  String toolSearchMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count matches',
      one: '1 match',
      zero: '0 matches',
    );
    return '$_temp0';
  }

  @override
  String get toolReadNoLines => 'There are no lines in this range.';

  @override
  String get toolReadMoreLines => 'More lines are available.';

  @override
  String toolReadLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count lines',
      one: '1 line',
      zero: '0 lines',
    );
    return '$_temp0';
  }

  @override
  String get toolFileTypeFile => 'File';

  @override
  String get toolFileTypeDirectory => 'Folder';

  @override
  String toolWriteSuccess(String size) {
    return '$size written';
  }

  @override
  String get toolFilePreview => 'Written content preview';

  @override
  String toolEditSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# changes applied',
      one: '# change applied',
    );
    return '$_temp0';
  }

  @override
  String get toolEditBefore => 'Before';

  @override
  String get toolEditAfter => 'After';

  @override
  String get toolPermissionCommand => 'Command';

  @override
  String get toolPermissionTerminalId => 'Terminal ID';

  @override
  String get toolPermissionInput => 'Terminal input';

  @override
  String get toolTerminalNoOutput => 'No output produced.';

  @override
  String get toolTerminalWaitingOutput => 'Waiting for output or input...';

  @override
  String get toolTerminalRunning => 'Running...';

  @override
  String get toolTerminalTerminated => 'Terminated';

  @override
  String get toolTerminalWaitingForInput => 'Waiting for input';

  @override
  String toolTerminalExitCode(int code) {
    return 'Exit code: $code';
  }

  @override
  String get toolTerminalCopied => 'Copied to clipboard';

  @override
  String get toolTechnicalDetails => 'Details';

  @override
  String toolFileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items',
      one: '1 item',
      zero: 'No items',
    );
    return '$_temp0';
  }

  @override
  String get toolEmptyListing => 'There are no items to show in this folder.';

  @override
  String get toolListingUnavailable => 'This file list could not be displayed.';

  @override
  String get toolListingIncomplete => 'Some items could not be listed.';

  @override
  String get toolMoreFilesAvailable => 'More items are available.';

  @override
  String get toolDesktopLocation => 'Desktop';

  @override
  String get toolProjectLocation => 'Project folder';

  @override
  String get toolOpenChatLocation => 'Application data folder';

  @override
  String get responseFailed => 'Response failed';

  @override
  String get responseStopped => 'Response stopped';

  @override
  String secondsShort(int count) {
    return '$count sec';
  }

  @override
  String get usageQuotas => 'Usage Quotas';

  @override
  String get usageQuotasDescription =>
      'View remaining quotas, usage limits, and reset times for all your connected ChatGPT accounts.';

  @override
  String get statistics => 'Statistics';

  @override
  String get statisticsDescription =>
      'Explore token usage, requests, and ChatGPT quota history by provider, model, and conversation.';

  @override
  String get statisticsLoadFailed => 'Usage statistics could not be loaded.';

  @override
  String get statisticsUnavailable =>
      'Local usage statistics are not available right now.';

  @override
  String get statisticsRetry => 'Reload';

  @override
  String get statisticsDateRange => 'Date range';

  @override
  String get statisticsProvider => 'Provider';

  @override
  String get statisticsRunId => 'Run ID';

  @override
  String statisticsRunIdValue(String runId) {
    return 'Run ID: $runId';
  }

  @override
  String get statisticsModel => 'Model';

  @override
  String get statisticsOperation => 'Operation type';

  @override
  String get statisticsReasoningEffort => 'Reasoning effort';

  @override
  String get statisticsFastModeFilter => 'Fast mode';

  @override
  String get statisticsAll => 'All';

  @override
  String get statisticsUnspecified => 'Unspecified';

  @override
  String get statisticsClearFilters => 'Clear filters';

  @override
  String get statisticsTotalTokens => 'Total tokens';

  @override
  String get statisticsInputTokens => 'Input tokens';

  @override
  String get statisticsOutputTokens => 'Output tokens';

  @override
  String get statisticsReasoningTokens => 'Reasoning tokens';

  @override
  String get statisticsCachedInputTokens => 'Cached input tokens';

  @override
  String get statisticsCacheWriteTokens => 'Cache write tokens';

  @override
  String get statisticsRequests => 'Requests';

  @override
  String get statisticsSuccessfulRequests => 'Successful';

  @override
  String get statisticsFailedRequests => 'Failed';

  @override
  String get statisticsCancelledRequests => 'Stopped';

  @override
  String get statisticsInterruptedRequests => 'Interrupted';

  @override
  String get statisticsPendingRequests => 'In progress';

  @override
  String get statisticsConversations => 'Conversations';

  @override
  String get statisticsCoverage => 'Usage data coverage';

  @override
  String get statisticsInputCoverage => 'Requests with reported input tokens';

  @override
  String get statisticsOutputCoverage => 'Requests with reported output tokens';

  @override
  String get statisticsReasoningCoverage =>
      'Requests with reported reasoning tokens';

  @override
  String statisticsCoverageText(int total, int reported) {
    return 'The provider returned token usage for $reported of $total requests.';
  }

  @override
  String get statisticsProviderReportedCost => 'Provider reported cost';

  @override
  String statisticsCostCoverage(int reported) {
    return 'Cost information is available for $reported requests.';
  }

  @override
  String get statisticsModelsDevCatalogCost => 'Cost at models.dev list prices';

  @override
  String statisticsModelsDevCatalogCostCoverage(int priced) {
    return 'List-price equivalent calculated for $priced requests.';
  }

  @override
  String statisticsModelsDevPricingCurrent(String date) {
    return 'models.dev pricing catalog fetched $date.';
  }

  @override
  String statisticsModelsDevPricingStale(String date) {
    return 'models.dev is unreachable; cached prices from $date are being used.';
  }

  @override
  String get statisticsModelsDevPricingUnavailable =>
      'models.dev pricing is unavailable. Costs are not calculated without an exact provider and model match.';

  @override
  String get statisticsUsageTrend => 'Usage over time';

  @override
  String get statisticsDaily => 'Daily';

  @override
  String get statisticsMonthly => 'Monthly';

  @override
  String get statisticsProviders => 'Provider usage';

  @override
  String get statisticsModels => 'Model usage';

  @override
  String get statisticsReasoningLevels => 'Selected reasoning effort';

  @override
  String get statisticsOperations => 'Operation types';

  @override
  String get statisticsFastModeUsage => 'Fast mode usage';

  @override
  String get statisticsRequested => 'Requested';

  @override
  String get statisticsNotRequested => 'Not requested';

  @override
  String get statisticsServiceTiers => 'Returned service tiers';

  @override
  String get statisticsServiceTier => 'Service tier';

  @override
  String get statisticsNoBreakdownData => 'No data to show for this period.';

  @override
  String get statisticsNoConversationData =>
      'No conversation usage for this period.';

  @override
  String get statisticsOpenConversation => 'Open conversation';

  @override
  String get statisticsRequestDetails => 'Request history';

  @override
  String get statisticsRequestPayload => 'Sent request data';

  @override
  String get statisticsRequestContextUnavailable =>
      'The sent request summary is unavailable.';

  @override
  String statisticsRequestSourceMessages(String ids) {
    return 'Included message IDs: $ids';
  }

  @override
  String statisticsRequestArchivedMessages(String ids) {
    return 'Retrieved message IDs: $ids';
  }

  @override
  String statisticsRequestSummaryBoundary(String id) {
    return 'Summary includes messages through: $id';
  }

  @override
  String statisticsRequestSourceAttachments(String files) {
    return 'Attachments sent: $files';
  }

  @override
  String get statisticsRequestSourcesTruncated =>
      'Some source details are omitted from this record.';

  @override
  String statisticsRequestMessageCount(int count) {
    return 'Messages sent: $count';
  }

  @override
  String statisticsRequestImageCount(int count) {
    return 'Images sent: $count';
  }

  @override
  String statisticsRequestToolResultCount(int count) {
    return 'Tool results sent: $count';
  }

  @override
  String statisticsRequestInstructionBytes(int count) {
    return 'Instruction bytes: $count';
  }

  @override
  String statisticsRequestRoles(String roles) {
    return 'Message roles: $roles';
  }

  @override
  String statisticsRequestTools(String names) {
    return 'Tool definitions: $names';
  }

  @override
  String statisticsRequestCacheControls(String names) {
    return 'Cache controls: $names';
  }

  @override
  String get statisticsNone => 'None';

  @override
  String get statisticsRequestTime => 'Request time';

  @override
  String get statisticsConversationTitle => 'Conversation title';

  @override
  String get statisticsStatus => 'Status';

  @override
  String get statisticsUsageSource => 'Usage data source';

  @override
  String get statisticsNoRequestData => 'No requests match these filters.';

  @override
  String statisticsShowingRows(int start, int end, int total) {
    return '$start - $end of $total';
  }

  @override
  String get statisticsExportCsv => 'Export CSV';

  @override
  String get statisticsExporting => 'Exporting';

  @override
  String get statisticsExported => 'Statistics were exported to a CSV file.';

  @override
  String get statisticsExportFailed => 'Statistics could not be exported.';

  @override
  String get statisticsQuotaHistory => 'ChatGPT quota history';

  @override
  String get statisticsQuotaSnapshot => 'Quota snapshot';

  @override
  String get statisticsNoQuotaHistory =>
      'No ChatGPT quota snapshots were saved in this date range.';

  @override
  String get statisticsUsed => 'Used';

  @override
  String get statisticsResetAt => 'Resets';

  @override
  String get statisticsQuotaAllowed => 'Requests allowed';

  @override
  String get statisticsQuotaBlocked => 'Requests blocked';

  @override
  String get statisticsQuotaUnknown => 'Usage status unknown';

  @override
  String get statisticsQuotaFreshnessCurrent => 'Current data';

  @override
  String get statisticsQuotaFreshnessStale => 'Stale data';

  @override
  String get statisticsQuotaFreshnessUnknown => 'Data status unknown';

  @override
  String get statisticsNotReported => 'Not reported';

  @override
  String get statisticsLegacyDataNote =>
      'Older messages may contain only output tokens; missing model and input token details are not inferred.';

  @override
  String get statisticsModelsDevPricingNote =>
      'Catalog-price equivalents use current models.dev prices and are not provider invoices. ChatGPT OAuth subscription usage, Fast requests, and unsupported service tiers are excluded.';

  @override
  String get statisticsLegacyOutput => 'Output tokens from older messages';

  @override
  String get statisticsChatGptOAuth => 'ChatGPT OAuth';

  @override
  String get statisticsChatGptApi => 'ChatGPT API';

  @override
  String get statisticsOperationChat => 'Chat';

  @override
  String get statisticsOperationToolFollowUp => 'Tool follow-up';

  @override
  String get statisticsOperationCompaction => 'Context compaction';

  @override
  String get statisticsOperationTitleGeneration =>
      'Conversation title generation';

  @override
  String get statisticsOperationLegacy => 'Older message';

  @override
  String get statisticsStatusCompleted => 'Completed';

  @override
  String get statisticsStatusFailed => 'Failed';

  @override
  String get statisticsStatusCancelled => 'Stopped';

  @override
  String get statisticsStatusInterrupted => 'Interrupted';

  @override
  String get statisticsStatusPending => 'In progress';

  @override
  String get statisticsStatusLegacy => 'Historical data';

  @override
  String get refreshAll => 'Refresh all';

  @override
  String get noChatGptAccountsForQuota =>
      'No connected ChatGPT accounts found.';

  @override
  String get noChatGptAccountsForQuotaDescription =>
      'Connect your ChatGPT account in the Connections tab to view your usage limits and quotas.';

  @override
  String get goToConnections => 'Go to Connections';

  @override
  String get activeAccountBadge => 'Active';

  @override
  String workspaceQuotaLabel(String name) {
    return 'Workspace: $name';
  }

  @override
  String get modelsPageDescription =>
      'Find and download models hosted on Hugging Face.';

  @override
  String get modelSortDownloads => 'Most downloaded';

  @override
  String get modelSortLikes => 'Most liked';

  @override
  String get modelSortRecentlyUpdated => 'Recently updated';

  @override
  String get modelPreviousPage => 'Previous';

  @override
  String get modelNextPage => 'Next';

  @override
  String modelPageLabel(int page) {
    return 'Page $page';
  }

  @override
  String get modelFormatGguf => 'GGUF · llama.cpp';

  @override
  String get modelFormatTransformers => 'Transformers · vLLM';

  @override
  String get modelFormatExllama => 'ExLlama · EXL3';

  @override
  String get huggingFaceModelSearchHint => 'Search Hugging Face models';

  @override
  String get modelSearchRefresh => 'Refresh model results';

  @override
  String get modelSearchEmpty => 'No models matched this search.';

  @override
  String get modelSearchFailed => 'Hugging Face models could not be loaded.';

  @override
  String get modelSearchUnavailable =>
      'Hugging Face could not be reached. Check your connection and try again.';

  @override
  String get modelSearchRateLimited =>
      'Hugging Face is receiving too many requests. Wait a moment and try again.';

  @override
  String get modelSearchInvalidResponse =>
      'Hugging Face returned model data OpenChat could not read. Try again shortly.';

  @override
  String get modelSearchTimedOut =>
      'Hugging Face took too long to respond. Try again.';

  @override
  String get modelChooseForDetails => 'Choose a model to inspect its files.';

  @override
  String get modelDownloadsLabel => 'Downloads';

  @override
  String get modelLikesLabel => 'Likes';

  @override
  String get modelLicenseLabel => 'License';

  @override
  String get modelRevisionLabel => 'Revision';

  @override
  String get modelFilesLabel => 'Model files';

  @override
  String get modelVisionComponentsLabel => 'Vision components';

  @override
  String get modelMtpComponentsLabel => 'MTP components';

  @override
  String get modelAuxiliaryComponentsLabel => 'Other auxiliary components';

  @override
  String get modelDownloadComponentButton => 'Download this component';

  @override
  String get modelComponentDownloaded => 'Component downloaded';

  @override
  String get modelShowMoreComponents => 'Show more components';

  @override
  String get modelReadmeLabel => 'Model description';

  @override
  String get modelReadmeMissing => 'This model does not have a README.';

  @override
  String get modelReadmeAccessDenied =>
      'Access to this repository is required to view its description.';

  @override
  String get modelReadmeTooLarge => 'The README is too large to display.';

  @override
  String get modelReadmeUnavailable =>
      'The model description could not be loaded.';

  @override
  String get modelDownloadOptionsLabel => 'Download options';

  @override
  String get modelDownloadGroupLabel => 'File set';

  @override
  String get modelDownloadSizeLabel => 'Size';

  @override
  String get modelDownloadButton => 'Download model';

  @override
  String get modelCancelDownload => 'Cancel download';

  @override
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount) {
    return 'File $fileIndex of $fileCount: $fileName';
  }

  @override
  String get modelDownloadComplete =>
      'Model downloaded and added to Local Models.';

  @override
  String get modelDownloadCancelled =>
      'Model download cancelled. You can resume it later.';

  @override
  String get modelDownloadFailed => 'The model could not be downloaded.';

  @override
  String get modelDownloadProgressUnavailable =>
      'Download progress could not be read.';

  @override
  String get modelRevisionChanged =>
      'This model changed on Hugging Face. Reload its files and try again.';

  @override
  String get modelDownloadAccessNeeded =>
      'This repository is gated or private and needs Hugging Face access.';

  @override
  String get modelNoCompatibleFiles =>
      'No complete compatible model files were found in this repository.';

  @override
  String get modelUnknownDownloadSize =>
      'The file size is unavailable, so this download cannot start safely.';

  @override
  String get modelDetailsLoading => 'Loading model files…';

  @override
  String get modelNoFiles =>
      'No compatible files are available for this format.';

  @override
  String get modelGatedBadge => 'Access required';

  @override
  String get modelPrivateBadge => 'Private';

  @override
  String get modelSavedToFolder =>
      'Downloads use the folder selected for this engine in Settings.';

  @override
  String get userQuestionTitle => 'The assistant needs your input';

  @override
  String get userQuestionRequiredHint => 'Required questions are marked';

  @override
  String get userQuestionSubmit => 'Send answer';

  @override
  String get userQuestionResuming => 'Resuming the assistant';

  @override
  String get userQuestionUnavailable =>
      'This question is no longer available. Reload the conversation.';

  @override
  String get userQuestionRequiredValidation =>
      'Answer each required question to continue.';

  @override
  String get userQuestionSubmitFailed =>
      'Your answer could not be saved. Try again.';

  @override
  String get userQuestionRequiredLabel => 'Required';

  @override
  String get userQuestionContinue => 'Continue assistant';

  @override
  String get userQuestionSaved => 'Your answer is saved. Continue when ready.';

  @override
  String get userQuestionLoadFailed =>
      'The pending question could not be loaded. Try again.';

  @override
  String get userQuestionResumeFailed =>
      'The saved answer is ready, but the assistant could not continue. Try again.';

  @override
  String get userQuestionNotificationTitle => 'OpenChat is waiting for you';

  @override
  String get userQuestionNotificationBody =>
      'The AI is waiting for your response.';

  @override
  String get assistantResponseNotificationReplyEmpty =>
      'Enter a reply before sending.';

  @override
  String get assistantResponseNotificationReplyTooLong =>
      'This reply is too long to send from a notification. Use the chat composer to send it.';

  @override
  String get assistantResponseNotificationReplyUnavailable =>
      'This notification is out of date or the chat is unavailable. Open the chat and send a new message.';

  @override
  String get assistantResponseNotificationReplyNotSent =>
      'The quick reply could not be sent right now. Review the chat and try again.';

  @override
  String fileChangesSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# files changed',
      one: '# file changed',
    );
    return '$_temp0';
  }

  @override
  String fileChangesLineCounts(int added, int removed) {
    return '+$added / -$removed';
  }

  @override
  String get fileChangesSomeCountsUnavailable => 'Line counts unavailable';

  @override
  String get fileChangesView => 'View changes';

  @override
  String get fileChangesTrackingFailed =>
      'Some file changes could not be tracked. The list may be incomplete.';

  @override
  String get fileChangesTitle => 'Changes in this conversation';

  @override
  String get fileChangesOpenButton => 'Changes';

  @override
  String get fileChangesLoadFailed =>
      'Conversation changes could not be loaded.';

  @override
  String get fileChangesDiffFailed => 'The file diff could not be loaded.';

  @override
  String get fileChangesConflict =>
      'This file changed after the AI edit. It was left untouched.';

  @override
  String get fileChangesRevertFailed => 'The change could not be reverted.';

  @override
  String get fileChangesEmpty =>
      'No file changes were captured for this conversation.';

  @override
  String get fileChangesDiffTitle => 'Select a file to inspect its diff';

  @override
  String get fileChangesBinary =>
      'Binary file changes cannot be shown as text.';

  @override
  String get fileChangesDiffUnavailable =>
      'A text diff is unavailable for this file.';

  @override
  String get fileChangesDiffTruncated =>
      'The diff is long. Only its first part is shown.';

  @override
  String get fileChangesActive => 'Changed';

  @override
  String get fileChangesReverted => 'Reverted';

  @override
  String get fileChangesRevert => 'Revert';

  @override
  String fileChangesMoreFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# more files',
      one: '# more file',
    );
    return '$_temp0';
  }

  @override
  String get fileChangesUnavailableTitle => 'File changes unavailable';

  @override
  String get goalSlashCommand => '/goal';

  @override
  String get goalCommandDescription =>
      'Start this message as a goal and keep working until it is complete or needs your input.';

  @override
  String get goalObjectiveRequired => 'Write a goal after /goal.';

  @override
  String get goalWorking => 'Working on goal';

  @override
  String get goalPaused => 'Goal paused';

  @override
  String get goalInterrupted => 'Goal interrupted';

  @override
  String get goalCompleted => 'Goal completed';

  @override
  String get goalStopped => 'Goal stopped';

  @override
  String get goalFailed => 'Goal failed';

  @override
  String get goalPausedForQuota =>
      'Paused because the model quota or rate limit was reached.';

  @override
  String get goalPausedForBlocker => 'Paused because progress is blocked.';

  @override
  String get goalPausedForUserInput => 'Paused while waiting for your input.';

  @override
  String get goalPausedByUser => 'Paused by you.';

  @override
  String get goalPausedAfterError => 'Paused after a request error.';

  @override
  String get goalPauseAction => 'Pause goal';

  @override
  String get goalResumeAction => 'Resume goal';

  @override
  String get goalStopAction => 'Stop goal';

  @override
  String goalElapsedTime(String time) {
    return 'Elapsed: $time';
  }
}
