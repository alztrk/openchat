import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
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
    Locale('en'),
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
  /// **'Enter a non-empty, single-line API key up to 4096 characters.'**
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
  /// **'Enter a non-empty, single-line API key (up to 4096 characters).'**
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
  /// **'Local chat history could not be opened. Restart the app and try again.'**
  String get historyStorageUnavailableDescription;

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

  /// No description provided for @reasoningUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Reasoning is unavailable until a model is connected.'**
  String get reasoningUnavailable;

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
      <String>['en', 'tr'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
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
