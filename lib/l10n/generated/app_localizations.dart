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
  /// **'Zihora'**
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

  /// No description provided for @collapseSidebar.
  ///
  /// In en, this message translates to:
  /// **'Collapse sidebar'**
  String get collapseSidebar;

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
  /// **'The account connected, but an older saved credential could not be removed. Restart Zihora and try again.'**
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
  /// **'When the conversation account has no different model available, Zihora can use the account you choose here. It skips title generation when ordinary usage is unavailable.'**
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

  /// No description provided for @chatGptProvider.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT'**
  String get chatGptProvider;

  /// No description provided for @noModelConnected.
  ///
  /// In en, this message translates to:
  /// **'No model is connected yet'**
  String get noModelConnected;

  /// No description provided for @noModelConnectedBody.
  ///
  /// In en, this message translates to:
  /// **'Connect a ChatGPT account and choose a model to start a conversation.'**
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

  /// No description provided for @chatGptDataUnavailable.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT returned information Zihora could not read. Refresh the connection and try again.'**
  String get chatGptDataUnavailable;

  /// No description provided for @modelCatalogUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The model list is unavailable. Refresh the account connection and try again.'**
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
  /// **'ChatGPT could not complete the response. Your saved messages are still available.'**
  String get chatRequestFailed;

  /// No description provided for @chatGptRateLimited.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT reported a usage limit. Check this account’s usage details.'**
  String get chatGptRateLimited;

  /// No description provided for @chatGptReauthenticationRequired.
  ///
  /// In en, this message translates to:
  /// **'This ChatGPT connection needs a new sign-in. Reconnect the account in Settings.'**
  String get chatGptReauthenticationRequired;

  /// No description provided for @chatGptProviderChanged.
  ///
  /// In en, this message translates to:
  /// **'ChatGPT’s response format changed. Update Zihora and try again.'**
  String get chatGptProviderChanged;

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
