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
  String get sharedInstructions => 'Shared instructions';

  @override
  String get sharedInstructionsDescription =>
      'These instructions are sent with every ChatGPT and OpenCode conversation. File tool access follows the Tool access setting.';

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
  String get favoriteModels => 'Favorites';

  @override
  String get favoriteModelsEmpty => 'No favorite models yet.';

  @override
  String get modelSearchHint => 'Search models...';

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
      'Free models work without a key. Paid models are billed per request after adding Console credits. Add a Console service API key to use them.';

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
      'Enter a non-empty, single-line API key (up to 4096 characters).';

  @override
  String get openCodeKeyStorageFailed =>
      'The Console API key could not be read or saved securely.';

  @override
  String get openCodePaidModel => 'Paid';

  @override
  String get openCodeFreeModel => 'Free';

  @override
  String get add => 'Add';

  @override
  String get edit => 'Edit';

  @override
  String get exportConversation => 'Export conversation';

  @override
  String get deleteConversation => 'Delete conversation';

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
  String get toolPermissionRequireApprovalDescription =>
      'Each tool call asks for permission. Access is limited to the project folder and %LOCALAPPDATA%\\Zihora.';

  @override
  String get toolPermissionFullAccess => 'Full access';

  @override
  String get toolPermissionFullAccessDescription =>
      'Current read-only tools can access any folder without asking. Files are not changed and commands are not run.';

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
  String get providerRateLimited =>
      'The provider reported a usage limit. Try again later.';

  @override
  String get providerAuthenticationRequired =>
      'The provider rejected the request. Check the connection and model access.';

  @override
  String get providerRequestFailed =>
      'The provider could not complete the response. Your saved messages are still available.';

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
      'Local chat history could not be opened. Restart the app and try again.';

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
  String get searchChatsHint => 'Search your chats';

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
  String get newProjectConversation => 'Start a new project chat';

  @override
  String get newConversation => 'Start a new conversation';

  @override
  String get conversationTitle => 'New chat';

  @override
  String get renameConversation => 'Edit chat title';

  @override
  String get pinConversation => 'Pin chat';

  @override
  String get unpinConversation => 'Unpin chat';

  @override
  String get conversationTitleRequired => 'Chat title cannot be empty.';

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
      'Connect a ChatGPT account and choose a model to start a conversation.';

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
  String get stop => 'Stop';

  @override
  String get modelsLoading => 'Loading models...';

  @override
  String get modelsUnavailable => 'Models unavailable';

  @override
  String get noModelsAvailable =>
      'No models are currently available for this account.';

  @override
  String get chatGptDataUnavailable =>
      'ChatGPT returned information OpenChat could not read. Refresh the connection and try again.';

  @override
  String get modelCatalogUnavailable =>
      'The model list is unavailable. Refresh the account connection and try again.';

  @override
  String get messageSaveFailed =>
      'Your message could not be saved to local chat history.';

  @override
  String get chatHistoryUnavailable =>
      'Chat history could not be updated. Try again.';

  @override
  String get chatRequestFailed =>
      'ChatGPT could not complete the response. Your saved messages are still available.';

  @override
  String get chatGptRateLimited =>
      'ChatGPT reported a usage limit. Check this account’s usage details.';

  @override
  String get chatGptReauthenticationRequired =>
      'This ChatGPT connection needs a new sign-in. Reconnect the account in Settings.';

  @override
  String get chatGptProviderChanged =>
      'ChatGPT’s response format changed. Update OpenChat and try again.';

  @override
  String get cachedCatalog => 'cached models';

  @override
  String get reasoningUnavailable =>
      'Reasoning is unavailable until a model is connected.';

  @override
  String get attachFile => 'Attach file';

  @override
  String get attachmentsUnavailable =>
      'File attachments are not available yet.';

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
  String responseMetadata(String rate, String tokens, String time) {
    return '$rate tok/s · $tokens tokens · $time';
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
  String get toolZihoraLocation => 'Application data folder';

  @override
  String get responseFailed => 'Response failed';

  @override
  String get responseStopped => 'Response stopped';

  @override
  String secondsShort(int count) {
    return '$count sec';
  }
}
