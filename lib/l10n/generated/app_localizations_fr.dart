// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get appTitle => 'OpenChat';

  @override
  String get newChat => 'Nouvelle discussion';

  @override
  String get temporaryChatStart => 'Discussion temporaire';

  @override
  String get temporaryChatTitle => 'Discussion temporaire';

  @override
  String get temporaryChatEnd => 'Terminer';

  @override
  String temporaryChatNotice(String provider) {
    return 'Cette discussion reste hors de l’historique enregistré sauf si vous la sauvegardez. Les messages et fichiers joints sont envoyés à $provider, qui définit leur conservation. Les outils ne sont pas disponibles ici.';
  }

  @override
  String temporaryChatDisclosureTitle(String provider) {
    return 'Envoyer ce message à $provider ?';
  }

  @override
  String temporaryChatDisclosureBody(String provider) {
    return 'Votre message, les fichiers joints et cet historique temporaire seront envoyés à $provider. Ce fournisseur définit la durée de conservation de ces données. Continuer ?';
  }

  @override
  String get temporaryChatExitTitle => 'Quitter cette discussion temporaire ?';

  @override
  String get temporaryChatExitBody =>
      'Enregistrez les messages envoyés dans l’historique, supprimez-les ou continuez ici. Les pièces jointes non envoyées sont supprimées en quittant.';

  @override
  String get temporaryChatSave => 'Enregistrer dans l’historique';

  @override
  String get temporaryChatDiscard => 'Supprimer la discussion';

  @override
  String get temporaryChatCancel => 'Continuer la discussion';

  @override
  String get temporaryChatSaveFailed =>
      'Impossible d’enregistrer cette discussion temporaire. Elle est toujours disponible ici ; réessayez ou supprimez-la.';

  @override
  String get temporaryChatConnectionRequired =>
      'Connectez et vérifiez un compte ChatGPT avant de démarrer une discussion temporaire.';

  @override
  String get temporaryChatRemoveAttachmentsFirst =>
      'Supprimez les pièces jointes en attente avant de démarrer une discussion temporaire.';

  @override
  String get temporaryChatStopBeforeLeaving =>
      'Arrêtez la réponse avant de quitter ce chat temporaire.';

  @override
  String get temporaryChatMessageLimitReached =>
      'Ce chat temporaire a atteint sa limite de messages ou de taille. Enregistrez-le ou démarrez un autre chat.';

  @override
  String get temporaryChatResponseFailed =>
      'Aucune réponse n’a été reçue. Vous pouvez envoyer un autre message ou réessayer.';

  @override
  String get chats => 'Discussions';

  @override
  String get workspaces => 'Espaces de travail';

  @override
  String get workspacesDescription =>
      'Regroupez les conversations par sujet. Les fichiers et les outils restent dans les projets de code.';

  @override
  String get workspaceCreate => 'Nouvel espace de travail';

  @override
  String get workspaceCreateAction => 'Créer un espace de travail';

  @override
  String get workspaceName => 'Nom de l’espace de travail';

  @override
  String get workspaceNameRequired =>
      'Saisissez un nom pour l’espace de travail.';

  @override
  String get workspaceEmptyTitle => 'Aucun espace de travail pour le moment';

  @override
  String get workspaceEmptyDescription =>
      'Créez un espace de travail pour regrouper les conversations liées.';

  @override
  String get workspaceNoConversations =>
      'Aucune conversation dans cet espace de travail pour le moment.';

  @override
  String get workspaceMoveConversation => 'Déplacer la conversation';

  @override
  String get workspaceUnassignedChats => 'Discussions sans espace de travail';

  @override
  String get workspaceNoWorkspace => 'Aucun espace de travail';

  @override
  String workspaceConversationCount(int count) {
    return 'Conversations : $count';
  }

  @override
  String get workspaceRename => 'Renommer l’espace de travail';

  @override
  String get workspaceDelete => 'Supprimer l’espace de travail';

  @override
  String workspaceDeleteConfirmation(String name) {
    return 'Supprimer « $name » ? Ses conversations resteront dans Discussions.';
  }

  @override
  String get workspaceLoadFailed =>
      'Impossible de charger les espaces de travail.';

  @override
  String get workspacesLoading => 'Chargement des espaces de travail…';

  @override
  String get conversationsLoading => 'Chargement des conversations…';

  @override
  String get workspaceSaveFailed =>
      'Impossible d’enregistrer l’espace de travail.';

  @override
  String get workspaceDeleteFailed =>
      'Impossible de supprimer l’espace de travail.';

  @override
  String get workspaceMoveFailed => 'Impossible de déplacer la conversation.';

  @override
  String get outputs => 'Réponses enregistrées';

  @override
  String get outputsDescription =>
      'Enregistrez les réponses utiles pour les retrouver ici.';

  @override
  String get outputsEmptyTitle => 'Aucune réponse enregistrée pour le moment';

  @override
  String get outputsEmptyDescription =>
      'Utilisez Enregistrer la réponse sous une réponse pour la conserver ici.';

  @override
  String get outputsLoadFailed =>
      'Impossible de charger les réponses enregistrées.';

  @override
  String get outputsLoading => 'Chargement des réponses enregistrées…';

  @override
  String get attachments => 'Pièces jointes';

  @override
  String get attachmentChooseFile => 'Choisir un fichier';

  @override
  String get attachmentChooseFromLibrary => 'Choisir dans la bibliothèque';

  @override
  String get attachmentLibraryTitle => 'Fichiers des conversations';

  @override
  String get attachmentLibraryDescription =>
      'Parcourez les fichiers des conversations et retrouvez leur chat d’origine.';

  @override
  String get attachmentLibraryEmpty =>
      'Aucun fichier de conversation enregistré n’est disponible.';

  @override
  String get attachmentLibraryUnavailable =>
      'Ce fichier n’est plus disponible.';

  @override
  String get attachmentLibraryAllFilter => 'Tous les fichiers';

  @override
  String get attachmentLibraryAvailableFilter => 'Disponible';

  @override
  String get attachmentLibraryMissingFilter => 'Manquant';

  @override
  String get attachmentLibraryOpenConversation =>
      'Ouvrir la conversation source';

  @override
  String get attachmentLibraryLoading =>
      'Chargement des fichiers de conversation';

  @override
  String get attachmentLibrarySearchHint =>
      'Filtrer par nom de fichier ou de conversation';

  @override
  String get attachmentLibraryLoadFailed =>
      'Impossible de charger d’autres fichiers de conversation. Réessayez.';

  @override
  String get saveResponse => 'Enregistrer la réponse';

  @override
  String get removeSavedResponse => 'Supprimer la réponse enregistrée';

  @override
  String get outputSaveFailed => 'Impossible d’enregistrer la réponse.';

  @override
  String get outputRemoveFailed =>
      'Impossible de supprimer la réponse enregistrée.';

  @override
  String get outputOpenConversation => 'Ouvrir la conversation';

  @override
  String get outputConversationUnavailable =>
      'La conversation n’est plus disponible';

  @override
  String outputSavedAt(String date) {
    return 'Enregistrée le $date';
  }

  @override
  String get outputWorkspaceOpen => 'Modifier la réponse';

  @override
  String get outputWorkspaceTitle => 'Espace de travail de réponse';

  @override
  String get outputWorkspaceDescription =>
      'Modifiez une copie de travail. La conversation source reste inchangée.';

  @override
  String get outputWorkspaceContentLabel => 'Contenu de la réponse';

  @override
  String get outputWorkspaceMarkdownSource => 'Source Markdown';

  @override
  String get outputWorkspaceSource => 'Source';

  @override
  String get outputWorkspaceLanguage => 'Langage';

  @override
  String get outputWorkspaceEdit => 'Modifier';

  @override
  String get outputWorkspacePreview => 'Aperçu';

  @override
  String outputWorkspaceVersion(int version) {
    return 'Version $version';
  }

  @override
  String get outputWorkspaceSaveVersion => 'Enregistrer comme nouvelle version';

  @override
  String get outputWorkspaceSaveFailed =>
      'Impossible d’enregistrer la nouvelle version.';

  @override
  String get outputWorkspaceExport => 'Exporter en Markdown';

  @override
  String get outputExportReviewTitle => 'Vérifier avant l’exportation';

  @override
  String outputExportReviewScope(int version) {
    return 'Seule la version $version de la réponse sélectionnée sera exportée. La conversation source et les autres messages ne sont pas inclus.';
  }

  @override
  String get outputExportReviewSensitiveNotice =>
      'La copie Markdown sera enregistrée à l’emplacement choisi. La réponse peut contenir des informations sensibles ; sa suppression ultérieure ne permet pas de rappeler les autres copies.';

  @override
  String get outputWorkspaceExported => 'Version de la réponse exportée.';

  @override
  String get outputWorkspaceExportFailed =>
      'Impossible d’exporter la version de la réponse.';

  @override
  String get outputWorkspaceVersionsFailed =>
      'Impossible de charger l’historique des versions.';

  @override
  String get outputWorkspaceEmpty =>
      'Une version de réponse ne peut pas être vide.';

  @override
  String get outputWorkspaceUnsavedTitle =>
      'Abandonner les modifications non enregistrées ?';

  @override
  String get outputWorkspaceUnsavedBody =>
      'Ces modifications n’ont pas été enregistrées comme version.';

  @override
  String get outputWorkspaceKeepEditing => 'Continuer la modification';

  @override
  String get outputWorkspaceDiscard => 'Abandonner les modifications';

  @override
  String get collapseSidebars => 'Réduire les barres latérales';

  @override
  String get showSidebars => 'Afficher les barres latérales';

  @override
  String get previousPage => 'Page précédente';

  @override
  String get nextPage => 'Page suivante';

  @override
  String get home => 'Accueil';

  @override
  String get extensions => 'Extensions';

  @override
  String get scheduled => 'Planifiées';

  @override
  String get design => 'Design';

  @override
  String get security => 'Sécurité';

  @override
  String get sectionUnavailable =>
      'Cette rubrique n\'est pas encore disponible.';

  @override
  String get settings => 'Paramètres';

  @override
  String get settingsDescription => 'Connexions, apparence et données locales';

  @override
  String get settingsAppearanceAccessibilityGroup =>
      'Apparence et accessibilité';

  @override
  String get settingsChatBehaviorGroup => 'Comportement du chat';

  @override
  String get settingsProvidersModelsGroup => 'Fournisseurs et modèles';

  @override
  String get settingsToolsPermissionsGroup => 'Outils et autorisations';

  @override
  String get settingsNotificationsGroup => 'Notifications et alertes';

  @override
  String get settingsDataPrivacyGroup => 'Données et confidentialité';

  @override
  String get settingsSearchPlaceholder => 'Rechercher dans les paramètres';

  @override
  String get settingsSearchClear => 'Effacer la recherche';

  @override
  String get settingsSearchEmpty =>
      'Aucun paramètre ne correspond à cette recherche.';

  @override
  String get settingsAccountConnectionsHeading => 'Connexions de comptes';

  @override
  String get settingsModelProvidersHeading => 'Fournisseurs de modèles';

  @override
  String get customProvidersHeading => 'Fournisseurs personnalisés';

  @override
  String get customProvidersDescription =>
      'Ajoutez un fournisseur compatible avec le format d’API OpenAI ou Anthropic.';

  @override
  String get customProvidersEmpty =>
      'Aucun fournisseur personnalisé n’a encore été ajouté.';

  @override
  String get addCustomProvider => 'Ajouter un fournisseur personnalisé';

  @override
  String get editCustomProvider => 'Modifier le fournisseur personnalisé';

  @override
  String get customProviderName => 'Nom du fournisseur';

  @override
  String get customProviderProtocol => 'Format d’API';

  @override
  String get customProviderBaseUrl => 'URL de base de l’API';

  @override
  String get customProviderApiKey => 'Clé API';

  @override
  String get customProviderApiKeyOptional =>
      'Laissez vide si le serveur ne nécessite pas de clé';

  @override
  String get customProviderKeyUnchanged =>
      'Laissez vide pour conserver la clé actuelle';

  @override
  String get customProviderInvalid =>
      'Vérifiez les informations du fournisseur.';

  @override
  String get customProviderUrlInvalid =>
      'Saisissez une URL d’API HTTP ou HTTPS valide. Elle ne peut pas contenir d’informations utilisateur, de requête ni de fragment.';

  @override
  String get openAiCompatible => 'Compatible OpenAI';

  @override
  String get anthropicCompatible => 'Compatible Anthropic';

  @override
  String get settingsConnectionsDescription =>
      'Connectez les comptes et les fournisseurs de modèles que vous utilisez.';

  @override
  String get settingsUsageQuotasDescription =>
      'Consultez l’utilisation restante des services connectés.';

  @override
  String get settingsStatisticsDescription =>
      'Suivez l’évolution de l’utilisation des conversations et des modèles.';

  @override
  String get settingsModelPreferencesDescription =>
      'Choisissez les modèles affichés et leur mode de sélection.';

  @override
  String get settingsLocalEnginesDescription =>
      'Gérez les logiciels qui exécutent les modèles sur cet appareil.';

  @override
  String get settingsConversationMemoryDescription =>
      'Gérez la mémoire personnelle et recherchez dans les archives locales des conversations.';

  @override
  String get personalMemoryDescription =>
      'Enregistrez les faits de votre choix pour les ajouter aux prochaines requêtes du modèle lorsque la mémoire personnelle est activée.';

  @override
  String get personalMemoryPrivacyNotice =>
      'Les copies enregistrées restent sur cet appareil. Lorsque cette option est activée, leur texte est envoyé au fournisseur avec votre requête. Vous pouvez consulter ou supprimer les faits ici à tout moment.';

  @override
  String get personalMemoryEnabled => 'Utiliser la mémoire personnelle';

  @override
  String personalMemoryResponseLabel(int count) {
    return 'Mémoire · $count';
  }

  @override
  String get personalMemorySourcesTitle =>
      'Faits enregistrés envoyés avec cette demande';

  @override
  String get personalMemorySourcesDescription =>
      'Ces faits enregistrés ont été inclus dans la demande. Le modèle peut ne pas les avoir tous utilisés.';

  @override
  String get personalMemorySourceUnavailable =>
      'Ce fait a depuis été modifié ou supprimé de la mémoire enregistrée.';

  @override
  String get personalMemoryEnabledDescription =>
      'Inclure les faits enregistrés dans les nouvelles requêtes. Cette option est désactivée par défaut.';

  @override
  String get personalMemoryExcludeHereTooltip =>
      'Exclure la mémoire personnelle de cette conversation';

  @override
  String get personalMemoryUseHereTooltip =>
      'Utiliser la mémoire personnelle dans cette conversation';

  @override
  String get personalMemoryFactsTitle => 'Faits enregistrés';

  @override
  String get personalMemoryEmpty => 'Vous n’avez encore enregistré aucun fait.';

  @override
  String get personalMemoryAdd => 'Ajouter un fait';

  @override
  String get personalMemoryEdit => 'Modifier le fait';

  @override
  String get personalMemoryDelete => 'Supprimer le fait';

  @override
  String get personalMemoryDeleteTitle => 'Supprimer ce fait enregistré ?';

  @override
  String get personalMemoryDeleteConfirmation =>
      'Il sera supprimé de cet appareil et des prochaines requêtes au modèle.';

  @override
  String get personalMemoryDeleteConfirm => 'Supprimer';

  @override
  String get personalMemoryFactLabel => 'Fait à mémoriser';

  @override
  String get personalMemoryFactHint =>
      'Par exemple, une préférence à retenir par OpenChat';

  @override
  String get personalMemoryLimitReached =>
      'Vous avez atteint la limite de faits enregistrés.';

  @override
  String get personalMemoryLoadFailed =>
      'Impossible de charger les faits enregistrés.';

  @override
  String get personalMemorySaveFailed =>
      'Impossible de modifier le fait enregistré.';

  @override
  String get personalMemoryClearAction => 'Effacer les souvenirs enregistrés';

  @override
  String get personalMemoryClearTitle =>
      'Effacer tous les souvenirs enregistrés ?';

  @override
  String get personalMemoryClearConfirmation =>
      'Tous les faits enregistrés seront supprimés de cet appareil et la mémoire personnelle sera désactivée.';

  @override
  String get personalMemoryClearConfirm => 'Effacer les souvenirs';

  @override
  String get personalMemoryClearFailed =>
      'Impossible d’effacer les souvenirs enregistrés.';

  @override
  String get settingsSharedInstructionsDescription =>
      'Définissez des consignes ajoutées aux nouvelles conversations.';

  @override
  String get settingsAssistantProfilesDescription =>
      'Créez des assistants réutilisables avec leurs propres consignes de réponse.';

  @override
  String get settingsNotifications => 'Notifications';

  @override
  String get settingsNotificationsDescription =>
      'Gérez les alertes pour les réponses terminées et les questions en attente.';

  @override
  String get keyboardShortcuts => 'Raccourcis clavier';

  @override
  String get keyboardShortcutsDescription =>
      'Modifiez les touches pour envoyer un message, ajouter une ligne ou arrêter une réponse.';

  @override
  String get keyboardShortcutRecord => 'Appuyez sur une combinaison de touches';

  @override
  String get keyboardShortcutConflict =>
      'Cette combinaison est déjà attribuée à une autre action.';

  @override
  String get keyboardShortcutLoadFailed =>
      'Impossible de charger les raccourcis clavier.';

  @override
  String get keyboardShortcutSaveFailed =>
      'Impossible d’enregistrer les raccourcis clavier.';

  @override
  String get keyboardShortcutsReset => 'Rétablir les valeurs par défaut';

  @override
  String get keyboardSendMessage => 'Envoyer le message';

  @override
  String get keyboardSendMessageDescription =>
      'Envoie le message lorsque le champ est actif.';

  @override
  String get keyboardNewLine => 'Insérer une nouvelle ligne';

  @override
  String get keyboardNewLineDescription =>
      'Ajoute un saut de ligne sans envoyer le message.';

  @override
  String get keyboardStopResponse => 'Arrêter la réponse';

  @override
  String get keyboardStopResponseDescription =>
      'Arrête la réponse lorsque le champ est actif.';

  @override
  String get notificationResponses => 'Notifications de réponse';

  @override
  String get notificationResponsesDescription =>
      'Afficher une alerte lorsqu’une réponse est terminée pendant qu’OpenChat est en arrière-plan.';

  @override
  String get notificationQuestions => 'Questions en attente';

  @override
  String get notificationQuestionsDescription =>
      'Afficher une alerte lorsque l’IA attend votre réponse.';

  @override
  String get notificationResponsePreview => 'Afficher le texte de la réponse';

  @override
  String get notificationResponsePreviewDescription =>
      'Inclure le texte de la réponse dans l’alerte. Si cette option est désactivée, seule la disponibilité d’une réponse est indiquée.';

  @override
  String get notificationQuickReply => 'Répondre depuis la notification';

  @override
  String get notificationQuickReplyDescription =>
      'Ajouter un champ de texte et une action d’envoi aux alertes de réponse.';

  @override
  String get notificationSettingsSaveFailed =>
      'Impossible d’enregistrer la préférence de notification.';

  @override
  String get notificationSettingsUnavailable =>
      'Impossible de lire les préférences de notification.';

  @override
  String get notificationDeliveryChecking =>
      'Vérification de la disponibilité des notifications Windows.';

  @override
  String get notificationDeliveryEnabled =>
      'Les notifications Windows sont disponibles.';

  @override
  String get notificationDeliveryDisabledForApp =>
      'Les notifications OpenChat sont désactivées dans Windows.';

  @override
  String get notificationDeliveryDisabledForUser =>
      'Les notifications sont désactivées dans les paramètres Windows.';

  @override
  String get notificationDeliveryDisabledByPolicy =>
      'Les notifications sont bloquées par la stratégie de votre organisation.';

  @override
  String get notificationDeliveryDisabledByManifest =>
      'OpenChat n’est pas enregistré pour envoyer des notifications Windows.';

  @override
  String get notificationDeliveryUnavailable =>
      'Impossible de vérifier la disponibilité des notifications Windows.';

  @override
  String get notificationDeliveryNotSupported =>
      'L’état des notifications Windows n’est pas disponible sur cette plateforme.';

  @override
  String get sendTestNotification => 'Envoyer une notification de test';

  @override
  String get notificationTestTitle => 'Test des notifications OpenChat';

  @override
  String get notificationTestBody =>
      'Si vous voyez ce message, Windows transmet les notifications.';

  @override
  String get notificationTestRequested => 'Notification de test demandée.';

  @override
  String get notificationTestFailed =>
      'Impossible d’envoyer la notification de test.';

  @override
  String get openNotificationSettings =>
      'Ouvrir les paramètres de notification Windows';

  @override
  String get settingsAppearanceDescription =>
      'Réglez la langue, le thème, la taille du texte et la largeur des conversations.';

  @override
  String get settingsLocalDataDescription =>
      'Consultez l’emplacement de l’historique et gérez les archives locales.';

  @override
  String get connections => 'Connexions';

  @override
  String get providerCatalogVerification => 'Vérifier le fournisseur';

  @override
  String providerConnectionLastChecked(Object dateTime) {
    return 'Dernière vérification : $dateTime';
  }

  @override
  String providerLastRequestOutcome(Object outcome) {
    return 'Dernière requête : $outcome';
  }

  @override
  String providerLastRequestAt(Object dateTime) {
    return 'Heure de la requête : $dateTime';
  }

  @override
  String get providerLastRequestReadFailed =>
      'Le résultat de la dernière requête n’a pas pu être lu.';

  @override
  String get providerLastRequestSaveFailed =>
      'La requête est terminée, mais son résultat n’a pas pu être enregistré.';

  @override
  String get providerRequestCompleted => 'Terminée';

  @override
  String get providerRequestWaitingForInput => 'En attente d’une réponse';

  @override
  String get providerRequestStopped => 'Arrêtée';

  @override
  String get providerRequestAuthenticationFailed =>
      'Échec de l’authentification';

  @override
  String get providerRequestRateLimited => 'Limite de requêtes atteinte';

  @override
  String get providerRequestNetworkFailed => 'Erreur réseau';

  @override
  String get providerRequestFailed =>
      'Le fournisseur n’a pas pu terminer la réponse. Vos messages enregistrés sont toujours disponibles.';

  @override
  String get providerVerificationResultNotSaved =>
      'Le résultat n’a pas pu être enregistré sur cet appareil.';

  @override
  String get providerVerificationRecordUnreadable =>
      'Le résultat enregistré n’a pas pu être lu. Vérifiez à nouveau la connexion.';

  @override
  String get setupStepCompleted => 'Terminé';

  @override
  String get setupStepCurrent => 'Étape actuelle';

  @override
  String get models => 'Modèles';

  @override
  String get modelLibrary => 'Bibliothèque';

  @override
  String get modelPreferences => 'Modèles';

  @override
  String get modelsDescription =>
      'Choisissez le modèle par défaut des nouvelles conversations et masquez ceux que vous n’utilisez pas.';

  @override
  String get defaultModelPreferenceInvalid =>
      'Le choix du modèle par défaut enregistré est illisible. Effacez-le dans Préférences des modèles ou Options du projet.';

  @override
  String get clearDefaultModelPreference =>
      'Effacer le modèle par défaut enregistré';

  @override
  String get defaultModelPreferenceClearFailed =>
      'Le choix du modèle par défaut enregistré n’a pas pu être effacé.';

  @override
  String get hiddenModelPreferencesInvalid =>
      'Les préférences des modèles masqués sont illisibles. Effacez-les ici pour afficher de nouveau tous les modèles.';

  @override
  String get clearHiddenModelPreferences => 'Réinitialiser les modèles masqués';

  @override
  String get hiddenModelPreferencesClearFailed =>
      'Les préférences des modèles masqués n’ont pas pu être effacées.';

  @override
  String get modelContextAutomatic => 'Automatique';

  @override
  String modelContextCustom(String count) {
    return 'Limite : $count';
  }

  @override
  String modelContextTitle(String model) {
    return 'Budget de contexte · $model';
  }

  @override
  String modelContextMaximum(String limit) {
    return 'Maximum du modèle : $limit';
  }

  @override
  String get modelContextCustomLabel => 'Votre limite de contexte en jetons';

  @override
  String get modelContextExplanation =>
      'OpenChat réduit l’historique ancien selon cette limite avant l’envoi. Cela ne modifie pas la capacité réelle du modèle et ne garantit pas une baisse de l’utilisation du quota.';

  @override
  String get modelContextInvalid =>
      'Saisissez une valeur comprise entre 2 048 et 2 000 000 jetons.';

  @override
  String get modelContextAboveMaximum =>
      'La limite ne peut pas dépasser le maximum du modèle.';

  @override
  String get localEngines => 'Moteurs locaux';

  @override
  String get localEnginesDescription =>
      'Installez et gérez les moteurs de modèles locaux.';

  @override
  String get localEnginesUnavailable =>
      'Le service moteur local n\'est pas encore disponible.';

  @override
  String get localEnginesLoadFailed =>
      'Les informations sur le moteur local n\'ont pas pu être chargées.';

  @override
  String get localEnginesEmpty =>
      'Aucune version locale du moteur n’est disponible.';

  @override
  String get localEnginesReload => 'Recharger';

  @override
  String localEngineRelease(String tag) {
    return 'Version $tag';
  }

  @override
  String localEngineNewerReleaseIncompatible(String latest, String compatible) {
    return 'Upstream a publié $latest ; $compatible est la version la plus récente compatible avec cet environnement.';
  }

  @override
  String get localEngineReleaseCheckUnavailable =>
      'Impossible de vérifier les nouvelles versions. Vérifiez la connexion puis actualisez ; l’installation exige une version vérifiée.';

  @override
  String get localEngineVariants => 'Packages';

  @override
  String get localEngineStable => 'Stable';

  @override
  String get localEnginePreview => 'Aperçu';

  @override
  String get localEngineNightly => 'Nightly';

  @override
  String get localEngineRecommended => 'Recommandé';

  @override
  String get localEngineAvailable => 'Disponible';

  @override
  String get localEngineInstalled => 'Installé';

  @override
  String get localEngineUpdateAvailable => 'Mise à jour disponible';

  @override
  String get localEngineNotInstalled => 'Non installé';

  @override
  String get localEngineBlocked => 'Bloqué';

  @override
  String get localEngineDeprecated => 'Obsolète';

  @override
  String get localEngineUnsupportedPlatform => 'Plateforme non prise en charge';

  @override
  String get localEngineHardwareUnavailable => 'Matériel indisponible';

  @override
  String get localEngineDriverUnsupported => 'Mettez à jour le pilote NVIDIA';

  @override
  String get localEngineDriverVersionUnavailable =>
      'Impossible de vérifier la version du pilote NVIDIA';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Configuration requise : $requirements';
  }

  @override
  String get localEngineInstall => 'Installer';

  @override
  String get localEngineUpdate => 'Mettre à jour';

  @override
  String get localEngineInstalling => 'Préparation de l\'installation...';

  @override
  String get localEngineInstallProgress =>
      'Progression de l\'installation du moteur local';

  @override
  String get localEngineCancelInstall => 'Annuler l\'installation';

  @override
  String get localEngineCancellingInstall => 'Annulation...';

  @override
  String get localEngineInstallFailed =>
      'Le moteur local n\'a pas pu être installé. Le catalogue a été rafraîchi.';

  @override
  String get localEngineHealth => 'État du runtime';

  @override
  String get localEngineExecutable => 'Exécutable llama-server';

  @override
  String get localEngineExecutableDescription =>
      'Choisissez un exécutable llama-server pour qu’OpenChat lance les modèles GGUF enregistrés. Cette option est distincte de la connexion à un serveur que vous avez démarré.';

  @override
  String get localEngineExecutableChoose => 'Choisir un exécutable';

  @override
  String get localEngineExecutableClear => 'Effacer la sélection';

  @override
  String get localEngineExecutableNotConfigured =>
      'Aucun exécutable sélectionné. OpenChat peut utiliser un paquet installé.';

  @override
  String get localEngineExecutableMissing =>
      'L’exécutable enregistré est introuvable. Sélectionnez-le à nouveau.';

  @override
  String get localEngineExecutableInvalid =>
      'Choisissez un exécutable llama-server existant.';

  @override
  String get localEngineSettingsFailed =>
      'Impossible d’enregistrer le paramètre llama-server.';

  @override
  String get localEngineExternalServerCheck =>
      'Rechercher un llama-server actif';

  @override
  String get localEngineExternalServerNotConnected =>
      'Aucun serveur démarré par l’utilisateur n’est connecté. OpenChat ne démarrera ni n’arrêtera ce serveur.';

  @override
  String get localEngineExternalServerConnecting =>
      'Vérification du serveur local sélectionné…';

  @override
  String get localEngineExternalServerFoundTitle =>
      'Un llama-server actif a été détecté';

  @override
  String localEngineExternalServerFoundDescription(int port) {
    return 'Un serveur llama.cpp écoute sur le port $port. OpenChat s’y connectera et affichera ses modèles. Se déconnecter d’OpenChat n’arrêtera pas le serveur.';
  }

  @override
  String get localEngineExternalServerNotNow => 'Pas maintenant';

  @override
  String get localEngineExternalServerConnect => 'Se connecter';

  @override
  String localEngineExternalServerConnected(int port, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count modèles disponibles',
      one: '1 modèle disponible',
    );
    return 'Connecté au port $port. $_temp0.';
  }

  @override
  String get localEngineManagedModelSection => 'Modèles gérés par OpenChat';

  @override
  String localEngineManagedServerModelSection(int port) {
    return 'Serveur OpenChat · 127.0.0.1:$port';
  }

  @override
  String localEngineExternalModelSection(int port) {
    return 'Serveur de l’utilisateur · 127.0.0.1:$port';
  }

  @override
  String get localEngineExternalServerDisconnect => 'Déconnecter';

  @override
  String get localEngineExternalServerNotFound =>
      'Aucun llama-server actif n’a été trouvé.';

  @override
  String get localEngineExternalServerScanFailed =>
      'Impossible de vérifier les processus llama-server actifs.';

  @override
  String get localEngineExternalServerConnectFailed =>
      'Connexion impossible. Vérifiez que llama-server est prêt et expose son point de terminaison local de modèles.';

  @override
  String get localEngineExternalServerAuthRequired =>
      'Ce serveur exige une authentification. OpenChat ne lit ni ne réutilise les identifiants d’autres processus.';

  @override
  String get localEngineRunning => 'En cours d\'exécution';

  @override
  String get localEngineStopped => 'Arrêté';

  @override
  String get localEngineUnhealthy => 'Ne répond pas';

  @override
  String get localEngineUnavailable => 'Ce moteur ne peut pas encore démarrer.';

  @override
  String get localEngineStartModel => 'Démarrer le modèle';

  @override
  String get localEngineStopModel => 'Arrêter le moteur';

  @override
  String get localModels => 'Modèles enregistrés';

  @override
  String get localModelsEmpty =>
      'Aucun modèle n\'est enregistré pour ce moteur.';

  @override
  String get localModelsPageTitle => 'Modèles locaux';

  @override
  String get localModelsLoading => 'Chargement des modèles locaux…';

  @override
  String get localModelsPageDescription =>
      'Affichez les modèles locaux téléchargés et enregistrés.';

  @override
  String get localModelsPageEmpty =>
      'Aucun modèle local n\'est encore enregistré.';

  @override
  String get localModelsLoadFailed =>
      'Les modèles locaux n\'ont pas pu être chargés.';

  @override
  String get localModelsRefresh => 'Rafraîchir';

  @override
  String get localModelsDiscover => 'Découvrir des modèles';

  @override
  String get localModelAddFile => 'Ajouter un fichier modèle';

  @override
  String get localModelAddFolder => 'Ajouter un dossier modèle';

  @override
  String get localModelStorageChoiceTitle => 'Choisir l’emplacement du modèle';

  @override
  String localModelStorageChoiceTarget(String folder) {
    return 'Dossier modèle sélectionné : $folder';
  }

  @override
  String get localModelDirectoryTitle => 'Dossier des modèles';

  @override
  String get localModelDirectoryDescription =>
      'Choisissez où enregistrer les modèles de ce moteur. Changer de dossier ne déplace pas les modèles déjà enregistrés.';

  @override
  String get localModelChooseDirectory => 'Choisir un dossier';

  @override
  String get localModelUseDefaultDirectory => 'Utiliser le dossier par défaut';

  @override
  String get localModelScanDirectory => 'Analyser le dossier';

  @override
  String get localModelScanningDirectory => 'Analyse du dossier…';

  @override
  String get localModelDiscoveryTitle => 'Modèles non enregistrés trouvés';

  @override
  String localModelDiscoveryPrompt(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'OpenChat a trouvé $count modèles compatibles non enregistrés dans ce dossier. Voulez-vous les enregistrer ?',
      one: 'OpenChat a trouvé 1 modèle compatible non enregistré dans ce dossier. Voulez-vous l’enregistrer ?',
    );
    return '$_temp0';
  }

  @override
  String get localModelDiscoveryTruncated =>
      'La limite de sécurité de l’analyse est atteinte. Choisissez un dossier plus petit pour trouver d’autres modèles.';

  @override
  String get localModelDiscoveryEmpty =>
      'Aucun nouveau modèle compatible n’a été trouvé dans ce dossier.';

  @override
  String get localModelDiscoveryRegisterAll =>
      'Enregistrer les modèles trouvés';

  @override
  String localModelDiscoveryRegistered(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count modèles enregistrés.',
      one: '1 modèle enregistré.',
    );
    return '$_temp0';
  }

  @override
  String localModelDiscoveryPartial(int registered, int total) {
    return '$registered modèles sur $total ont été enregistrés. Certains modèles n’ont pas pu l’être.';
  }

  @override
  String get localModelDirectoryUnavailable =>
      'Ce dossier de modèles n’est pas disponible. Choisissez un dossier existant auquel OpenChat peut accéder.';

  @override
  String get localModelDiscoveryFailed =>
      'Le dossier de modèles n’a pas pu être analysé. Vérifiez les droits d’accès puis réessayez.';

  @override
  String get localModelMoveToFolder => 'Déplacer le modèle vers ce dossier';

  @override
  String get localModelCopyToFolder => 'Copier le modèle dans ce dossier';

  @override
  String get localModelKeepInPlace => 'Conserver le modèle à son emplacement';

  @override
  String get localModelSaving => 'Enregistrement du modèle…';

  @override
  String get localModelTransferError =>
      'Le modèle n\'a pas pu être copié ou déplacé. Le modèle original a été laissé en place.';

  @override
  String get localModelTransferRecoveryError =>
      'Le modèle n\'a pas pu être enregistré ou restauré. Une copie complète du modèle reste dans le dossier du modèle OpenChat ; sélectionnez-le ici pour l\'enregistrer.';

  @override
  String get localModelRemove => 'Supprimer l’enregistrement';

  @override
  String get localModelRemoveConfirmation =>
      'Seul l’enregistrement OpenChat sera supprimé. Le fichier du modèle restera sur le disque. Continuer ?';

  @override
  String get localModelCancelStart => 'Annuler le démarrage';

  @override
  String get localModelStopping => 'Arrêt du runtime…';

  @override
  String get localModelActionError =>
      'L\'action de modèle local n\'a pas pu être réalisée.';

  @override
  String get localModelPathMissing =>
      'Chemin du modèle introuvable. Restaurez le fichier à son emplacement ou supprimez son enregistrement.';

  @override
  String get localModelEngineNotReady =>
      'Disponible une fois le moteur installé et capable d’exécuter des modèles.';

  @override
  String get localModelInvalid =>
      'Le fichier ou dossier sélectionné n\'est pas un modèle valide pour ce moteur.';

  @override
  String get localModelPathError =>
      'Le fichier ou dossier de modèle sélectionné n\'est pas accessible.';

  @override
  String get localModelStoragePathError =>
      'OpenChat n\'a pas pu créer ses dossiers de modèles. Vérifiez l\'espace de stockage disponible et les autorisations, puis réessayez.';

  @override
  String get localModelStorageError =>
      'L\'enregistrement du modèle n\'a pas pu être écrit dans la base de données.';

  @override
  String get localModelStartError =>
      'Le modèle n\'a pas pu être démarré. Vérifiez l\'installation du runtime et le fichier de modèle.';

  @override
  String get localModelStartTimeout =>
      'Le modèle n’était pas prêt à temps. Essayez un modèle plus petit ou vérifiez votre matériel.';

  @override
  String get localModelRuntimeUnavailable =>
      'Le modèle local ne répond plus. Redémarrez-le dans Paramètres > Moteurs locaux, puis réessayez.';

  @override
  String get localModelContextUnavailable =>
      'Le moteur local n\'a pas indiqué sa fenêtre de contexte active. Mettez à jour ou réinstallez llama.cpp, puis réessayez.';

  @override
  String get localModelInferenceFailed =>
      'Le modèle local n’a pas pu traiter cette demande. Vérifiez son modèle de conversation et la mémoire disponible.';

  @override
  String get localModelSaved => 'Modèle local enregistré.';

  @override
  String get localModelRemoved => 'Enregistrement du modèle local supprimé.';

  @override
  String get localEngineStageDownloading => 'Téléchargement';

  @override
  String get localEngineStageVerifying => 'Vérification';

  @override
  String get localEngineStageExtracting => 'Extraction';

  @override
  String get localEngineStagePublishing => 'Finalisation de l’installation';

  @override
  String get localEngineStageReady => 'Prêt';

  @override
  String get defaultModel => 'Par défaut';

  @override
  String get setDefaultModel => 'Définir comme modèle par défaut';

  @override
  String get clearDefaultModel => 'Supprimer le modèle par défaut';

  @override
  String defaultModelUpdated(String model) {
    return 'Modèle par défaut mis à jour : $model';
  }

  @override
  String get defaultModelCleared => 'Modèle par défaut supprimé.';

  @override
  String get hideModel => 'Cacher';

  @override
  String get showModel => 'Afficher';

  @override
  String get hiddenModel => 'Caché';

  @override
  String modelHidden(String model) {
    return 'Modèle masqué : $model';
  }

  @override
  String modelUnhidden(String model) {
    return 'Modèle affiché : $model';
  }

  @override
  String get noModelsFound => 'Aucun modèle trouvé.';

  @override
  String get refreshModels => 'Actualiser les modèles';

  @override
  String get connectedProvidersModels => 'Modèles de fournisseurs connectés';

  @override
  String get noConnectedProviders =>
      'Aucun fournisseur connecté pour l\'instant. Connectez des comptes ou ajoutez des clés API dans l\'onglet Connexions.';

  @override
  String get sharedInstructions => 'Instructions partagées';

  @override
  String get assistantProfiles => 'Profils d’assistant';

  @override
  String get assistantProfilesDescription =>
      'Choisissez un profil pour appliquer ses consignes et son style de réponse aux nouvelles requêtes. Les profils sont enregistrés sur cet appareil.';

  @override
  String get assistantProfilesEmpty =>
      'Aucun profil d’assistant pour le moment.';

  @override
  String get assistantProfileCreate => 'Créer un profil';

  @override
  String get assistantProfileDeactivate => 'Ne pas utiliser de profil';

  @override
  String get assistantProfileEdit => 'Modifier le profil';

  @override
  String get assistantProfileRemoveTitle => 'Supprimer le profil d’assistant ?';

  @override
  String get assistantProfileRemove => 'Supprimer';

  @override
  String assistantProfileRemoveConfirmation(String profileName) {
    return 'Supprimer « $profileName » ? Il ne sera plus utilisé pour les nouvelles requêtes.';
  }

  @override
  String get assistantProfileSaveFailed =>
      'Impossible d’enregistrer le profil d’assistant.';

  @override
  String get assistantProfilesLoadFailed =>
      'Impossible de charger les profils d’assistant.';

  @override
  String get assistantProfileLoadFailed =>
      'Impossible de charger le profil d’assistant actif.';

  @override
  String get assistantProfilesLimitReached =>
      'Vous pouvez enregistrer jusqu’à 20 profils d’assistant.';

  @override
  String get assistantProfileName => 'Nom';

  @override
  String get assistantProfileNameRequired => 'Saisissez un nom pour ce profil.';

  @override
  String get assistantProfileNameTooLong => 'Utilisez 64 caractères maximum.';

  @override
  String get assistantProfileNameExists =>
      'Un profil portant ce nom existe déjà.';

  @override
  String get assistantProfileInstructions => 'Consignes';

  @override
  String get assistantProfileInstructionsHelp =>
      'Lorsque le profil est actif, ces consignes sont envoyées avec les nouvelles requêtes de chat standard. Les chats temporaires n’utilisent pas de profil d’assistant.';

  @override
  String get assistantProfilePreferredModel => 'Modèle préféré';

  @override
  String get assistantProfilePreferredModelUnavailable =>
      'Sélectionnez d’abord un modèle disponible dans le chat.';

  @override
  String get assistantProfilePreferredModelHelp =>
      'Utilisé dans les nouveaux chats. Le modèle propre au projet est prioritaire.';

  @override
  String get assistantProfileRestrictTools => 'Limiter les outils de ce profil';

  @override
  String get assistantProfileRestrictToolsHelp =>
      'Seuls les outils sélectionnés sont proposés. Les autorisations de l’application et du projet peuvent les restreindre davantage.';

  @override
  String get assistantProfileProjectTools => 'Disponibles dans le projet actif';

  @override
  String get assistantProfileProjectTasks =>
      'Exécuter une tâche nommée du projet';

  @override
  String get assistantProfileMcpUnavailable =>
      'Certains serveurs MCP activés sont indisponibles ; leurs outils n’ont pas pu être chargés.';

  @override
  String get assistantProfileLoadMcpTools => 'Charger les outils MCP';

  @override
  String get assistantProfileLoadingMcpTools => 'Connexion aux serveurs MCP…';

  @override
  String assistantProfileAllowedToolCount(int count) {
    return 'Limite d’outils : $count';
  }

  @override
  String get assistantProfileToolAccessUsesAppPermissions =>
      'Autorisations de l’application et du projet';

  @override
  String get assistantProfileToolAccessInheritsAppPermissions =>
      'Ce profil ne limite pas les outils. Les autorisations de l’application et du projet déterminent ceux qui sont disponibles.';

  @override
  String get assistantProfileToolAccessDisabled => 'Outils désactivés';

  @override
  String get assistantProfileToolAccessDisabledHelp =>
      'Ce profil désactive tous les outils.';

  @override
  String get assistantToolGroupWebResearch =>
      'Rechercher sur le Web et lire des pages';

  @override
  String get assistantToolGroupWorkspaceRead =>
      'Lire et rechercher dans les fichiers de l’espace de travail';

  @override
  String get assistantToolGroupWorkspaceWrite =>
      'Créer et modifier des fichiers de l’espace de travail';

  @override
  String get assistantToolGroupTerminal => 'Exécuter des commandes de terminal';

  @override
  String get assistantToolGroupGitRead =>
      'Consulter l’état et l’historique Git';

  @override
  String get assistantToolGroupImageGeneration => 'Générer des images';

  @override
  String get assistantToolGroupAskUser => 'Poser une question à l’utilisateur';

  @override
  String get assistantToolGroupDelegateTask => 'Déléguer une tâche limitée';

  @override
  String get assistantToolGroupGoalControls =>
      'Gérer les objectifs persistants';

  @override
  String get assistantResponseStyle => 'Style de réponse';

  @override
  String get assistantResponseStyleConcise => 'Concis';

  @override
  String get assistantResponseStyleBalanced => 'Équilibré';

  @override
  String get assistantResponseStyleDetailed => 'Détaillé';

  @override
  String assistantProfileStyle(String style) {
    return 'Style : $style';
  }

  @override
  String assistantProfileActive(String profileName) {
    return 'Profil d’assistant : $profileName';
  }

  @override
  String get sharedInstructionsDescription =>
      'Ces instructions sont envoyées avec chaque fournisseur connecté. L\'accès aux outils de fichiers suit le paramètre d\'accès aux outils.';

  @override
  String get restoreDefaults => 'Restaurer les valeurs par défaut';

  @override
  String get sharedInstructionsRestored =>
      'Les instructions par défaut ont été restaurées.';

  @override
  String get defaultInstructionsLoadFailed =>
      'Impossible de charger les instructions par défaut. Réessayez.';

  @override
  String get sharedInstructionsHint =>
      'Décrivez comment vous souhaitez que les réponses soient rédigées...';

  @override
  String get sharedInstructionsLoadFailed =>
      'Impossible de charger les instructions partagées. Réessayez.';

  @override
  String get sharedInstructionsSaveFailed =>
      'Impossible d’enregistrer les instructions partagées.';

  @override
  String get sharedInstructionsSaved => 'Instructions partagées enregistrées.';

  @override
  String get sharedInstructionsTooLong =>
      'Les instructions partagées ne peuvent pas dépasser 4 096 caractères.';

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
  String get deepSeekProvider => 'DeepSeek';

  @override
  String get xAiProvider => 'xAI';

  @override
  String get perplexityProvider => 'Perplexity';

  @override
  String get togetherProvider => 'Together AI';

  @override
  String get sambaNovaProvider => 'SambaNova';

  @override
  String get anthropicProvider => 'Anthropic';

  @override
  String get fireworksProvider => 'Fireworks AI';

  @override
  String get deepInfraProvider => 'DeepInfra';

  @override
  String get dashScopeProvider => 'DashScope (Qwen)';

  @override
  String get cohereProvider => 'Cohere';

  @override
  String get geminiApiDescription =>
      'Traitez texte, images, audio et vidéo avec les modèles compatibles.';

  @override
  String get groqApiDescription =>
      'Exécutez des modèles ouverts avec une faible latence sur l’infrastructure Groq.';

  @override
  String get cerebrasApiDescription =>
      'Exécutez des grands modèles de langage sur l’infrastructure Cerebras Inference.';

  @override
  String get openRouterApiDescription =>
      'Accédez aux modèles de différents fournisseurs avec une seule API.';

  @override
  String get mistralApiDescription =>
      'Accédez aux services de chat, d’agents et de traitement documentaire via l’API Mistral.';

  @override
  String get deepSeekApiDescription =>
      'Accédez aux modèles de chat et de raisonnement de DeepSeek.';

  @override
  String get xAiApiDescription =>
      'Accédez aux modèles Grok via l’API de chat de xAI.';

  @override
  String get perplexityApiDescription =>
      'Obtenez des réponses appuyées sur le Web avec les modèles Perplexity.';

  @override
  String get togetherApiDescription =>
      'Accédez à un catalogue de modèles ouverts via l’API Together AI.';

  @override
  String get sambaNovaApiDescription =>
      'Exécutez des modèles hébergés sur l’infrastructure SambaNova Cloud.';

  @override
  String get anthropicApiDescription =>
      'Accédez aux modèles Claude via l’API Messages d’Anthropic.';

  @override
  String get fireworksApiDescription =>
      'Exécutez des modèles ouverts via l’API d’inférence Fireworks AI.';

  @override
  String get deepInfraApiDescription =>
      'Utilisez les modèles ouverts hébergés par DeepInfra via son API de chat.';

  @override
  String get dashScopeApiDescription =>
      'Utilisez Qwen et d’autres modèles compatibles via Alibaba Cloud Model Studio.';

  @override
  String get cohereApiDescription =>
      'Utilisez les modèles Cohere avec les API de chat et d’appel d’outils.';

  @override
  String get geminiUnpaidDataNotice =>
      'Avec le niveau gratuit de l’API Gemini, Google peut utiliser le contenu envoyé pour améliorer ses produits. Consultez les conditions d’utilisation des données de Google avant d’envoyer des informations sensibles.';

  @override
  String get providerApiKey => 'Clé API';

  @override
  String get providerKeySaved =>
      'La clé API est enregistrée en toute sécurité sur cet appareil.';

  @override
  String providerKeySavedSuffix(Object suffix) {
    return 'La clé API se terminant par ••••$suffix est enregistrée en toute sécurité sur cet appareil.';
  }

  @override
  String get providerNoKey => 'Aucune clé API n\'est connectée.';

  @override
  String get providerKeyInvalid =>
      'Entrez une clé API valide pour ce fournisseur. N’incluez pas d’espaces ; la clé peut comporter jusqu’à 4 096 caractères.';

  @override
  String get providerKeyStorageFailed =>
      'La clé API n\'a pas pu être lue ou enregistrée en toute sécurité.';

  @override
  String get verifyProviderConnection => 'Vérifier la connexion';

  @override
  String get providerConnectionNotVerified =>
      'La connexion n’a pas encore été vérifiée.';

  @override
  String get verifyingProviderConnection =>
      'Vérification de la connexion au fournisseur…';

  @override
  String get providerConnectionVerified =>
      'Connexion vérifiée ; le catalogue de modèles est disponible.';

  @override
  String get providerVerificationStaleCatalog =>
      'Seuls des modèles en cache ont été renvoyés ; la connexion au fournisseur n\'a pas pu être vérifiée. Réessayez.';

  @override
  String get providerModelCatalogEmpty =>
      'Le fournisseur a accepté la requête, mais n\'a renvoyé aucun modèle.';

  @override
  String get providerVerificationAuthenticationFailed =>
      'Le fournisseur a refusé cette clé. Vérifiez-la puis réessayez.';

  @override
  String get providerVerificationRateLimited =>
      'La limite du fournisseur a empêché la vérification. Réessayez plus tard.';

  @override
  String get providerVerificationNetworkFailed =>
      'Impossible de joindre le fournisseur. Vérifiez le réseau puis réessayez.';

  @override
  String get providerConnectionVerificationFailed =>
      'La vérification a échoué. Réessayez ou vérifiez les paramètres du fournisseur.';

  @override
  String get providerQuotaUnavailable =>
      'OpenChat n\'affiche pas actuellement les quotas de ce fournisseur.';

  @override
  String get favoriteModels => 'Favoris';

  @override
  String get favoriteModelsEmpty => 'Pas encore de modèles favoris.';

  @override
  String get modelSearchHint => 'Rechercher des modèles...';

  @override
  String get searchModels => 'Rechercher des modèles';

  @override
  String get chatGptFastModeEnabledTooltip =>
      'Le mode Fast est demandé. Il peut consommer plus rapidement les crédits de l’abonnement ou augmenter le coût par jeton API.';

  @override
  String get chatGptFastModeDisabledTooltip =>
      'Demander le mode Fast. Il peut consommer plus rapidement les crédits de l’abonnement ou augmenter le coût par jeton API ; sa disponibilité dépend du modèle.';

  @override
  String get chatGptFastModeUnavailableTooltip =>
      'Le modèle sélectionné ne déclare pas la prise en charge du mode Fast.';

  @override
  String get chatGptFastModeLoadingTooltip =>
      'Chargement de la préférence du mode Fast…';

  @override
  String get chatGptFastModeSettingsLoadFailed =>
      'Impossible de charger la préférence du mode Fast.';

  @override
  String get chatGptFastModeSettingsSaveFailed =>
      'Impossible d’enregistrer la préférence du mode Fast.';

  @override
  String get modelSearchNoResults =>
      'Aucun modèle ne correspond à votre recherche.';

  @override
  String get addModelFavorite => 'Ajouter aux modèles favoris';

  @override
  String get removeModelFavorite => 'Supprimer des favoris';

  @override
  String get openCodeConsole => 'OpenCode Console';

  @override
  String get openCodeConsoleDescription =>
      'Utilisez OpenCode Zen et ses modèles testés pour les agents de programmation.';

  @override
  String openCodeKeySaved(Object suffix) {
    return 'La clé API Console se terminant par ••••$suffix est enregistrée en toute sécurité sur cet appareil.';
  }

  @override
  String get openCodeNoKey =>
      'Aucune clé API Console. Les modèles gratuits sont disponibles.';

  @override
  String get openCodeApiKey => 'Clé API Console OpenCode';

  @override
  String get openCodeKeyInvalid =>
      'Saisissez une clé API non vide, sans espaces ni sauts de ligne (jusqu\'à 4 096 caractères).';

  @override
  String get openCodeKeyStorageFailed =>
      'La clé Console API n’a pas pu être lue ou enregistrée en toute sécurité.';

  @override
  String get openCodePaidModel => 'Payant';

  @override
  String get openCodeFreeModel => 'Gratuit';

  @override
  String get modelSourceApi => 'API';

  @override
  String get modelSourceOAuth => 'OAuth';

  @override
  String get openCodeFreeModels => 'Modèles gratuits';

  @override
  String get openCodeApiModels => 'Modèles API';

  @override
  String modelContextWindow(String value) {
    return 'Fenêtre de contexte · $value jetons';
  }

  @override
  String get modelDetailsProvider => 'Fournisseur';

  @override
  String get modelDetailsContext => 'Fenêtre de contexte';

  @override
  String get modelDetailsMaximumOutput => 'Sortie maximale';

  @override
  String get modelDetailsInputModalities => 'Types d’entrée';

  @override
  String get modelDetailsOutputModalities => 'Types de sortie';

  @override
  String get modelDetailsImages => 'Entrée d’images';

  @override
  String get modelDetailsToolCalling => 'Appels d’outils';

  @override
  String get modelDetailsSupported => 'Pris en charge';

  @override
  String get modelDetailsUnsupported => 'Non pris en charge';

  @override
  String get modelDetailsInputPrice => 'Entrée';

  @override
  String get modelDetailsOutputPrice => 'Sortie';

  @override
  String get modelDetailsReasoningPrice => 'Raisonnement';

  @override
  String get modelDetailsCacheReadPrice => 'Lecture du cache';

  @override
  String get modelDetailsCacheWritePrice => 'Écriture du cache';

  @override
  String get modelDetailsPricing => 'Tarifs catalogue';

  @override
  String get modelDetailsPricingLoading => 'Chargement des tarifs models.dev…';

  @override
  String get modelDetailsPricingLoadFailed =>
      'Les tarifs du modèle n’ont pas pu être chargés.';

  @override
  String get modelDetailsPricingStale =>
      'Tarifs en cache affichés ; models.dev est indisponible.';

  @override
  String get modelDetailsPricingUnavailable =>
      'Aucun tarif exact n’est répertorié pour ce modèle.';

  @override
  String modelDetailsPerMillion(String value) {
    return '$value / 1 million de jetons';
  }

  @override
  String modelDetailsTier(String context) {
    return 'Palier de contexte · $context';
  }

  @override
  String modelDetailsInputOutputPrices(String input, String output) {
    return 'Entrée $input · sortie $output';
  }

  @override
  String get modelDetailsPricingSource =>
      'Tarifs catalogue USD de models.dev, pas une facture du fournisseur.';

  @override
  String openCodeModelContextWindow(String value) {
    return 'Catalogue OpenCode (Models.dev) · contexte : $value jetons';
  }

  @override
  String get add => 'Ajouter';

  @override
  String get edit => 'Modifier';

  @override
  String get exportConversation => 'Exporter la conversation';

  @override
  String get deleteConversation => 'Supprimer la conversation';

  @override
  String get archiveConversation => 'Archiver la conversation';

  @override
  String get restoreConversation => 'Restaurer la conversation';

  @override
  String get archivedChats => 'Archivées';

  @override
  String get noArchivedChats => 'Aucune conversation archivée.';

  @override
  String get stopResponseBeforeArchive =>
      'Arrêtez la réponse en cours avant d’archiver cette conversation.';

  @override
  String get conversationArchived => 'Conversation archivée.';

  @override
  String get conversationRestored => 'Conversation restaurée.';

  @override
  String get conversationArchiveFailed =>
      'La conversation n’a pas pu être archivée. Réessayez.';

  @override
  String get conversationRestoreFailed =>
      'La conversation n’a pas pu être restaurée. Réessayez.';

  @override
  String get confirmDeleteConversationTitle => 'Supprimer cette conversation ?';

  @override
  String confirmDeleteConversation(String title) {
    return 'Cela supprimera définitivement « $title » et tous ses messages de cet appareil.';
  }

  @override
  String get stopResponseBeforeDelete =>
      'Arrêtez la réponse en cours avant de supprimer cette conversation.';

  @override
  String get conversationDeleted => 'Conversation supprimée.';

  @override
  String get conversationDeletedFileChangesCleanupFailed =>
      'La conversation a été supprimée, mais les sauvegardes des modifications de fichiers n’ont pas pu être effacées.';

  @override
  String get conversationHistoryClearedFileChangesCleanupFailed =>
      'L’historique des conversations a été supprimé, mais les sauvegardes des modifications de fichiers n’ont pas pu être effacées.';

  @override
  String get conversationDeleteFailed =>
      'Impossible de supprimer la conversation. Réessayez.';

  @override
  String get conversationExported =>
      'Conversation exportée au format Markdown.';

  @override
  String get conversationExportFailed =>
      'Impossible d’exporter la conversation. Réessayez.';

  @override
  String get conversationExportProvider => 'Fournisseur';

  @override
  String get conversationExportModel => 'Modèle';

  @override
  String get conversationExportCreated => 'Créé';

  @override
  String get conversationExportStatus => 'Statut';

  @override
  String get toolPermissions => 'Accès aux outils';

  @override
  String get toolPermissionsDescription =>
      'Choisissez où les outils de fichiers de l’IA peuvent agir et si chaque appel nécessite votre approbation.';

  @override
  String get toolPermissionPlan => 'Mode planification';

  @override
  String get toolPermissionPlanDescription =>
      'Aucun outil n’est disponible en mode planification. L’IA prépare un plan à partir de la conversation et attend votre approbation.';

  @override
  String get toolPermissionPlanOff => 'Désactiver le mode planification';

  @override
  String get composerGoalModeLabel => 'Mode objectif';

  @override
  String get composerGoalModeOff => 'Désactiver le mode objectif';

  @override
  String get toolPermissionRequireApproval => 'Demander une autorisation';

  @override
  String get selectedModelDoesNotSupportToolCalls =>
      'Ce modèle ne prend pas en charge les appels d’outils. Les paramètres d’accès aux outils ne s’appliquent pas.';

  @override
  String get selectedModelToolSupportUnknown =>
      'Ce modèle ne précise pas s’il prend en charge les appels d’outils. OpenChat va essayer de les envoyer ; le fournisseur peut rejeter la requête.';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Demande avant chaque appel de fichier, du Web ou du terminal. Les fichiers restent limités au projet et à OpenChat.';

  @override
  String get toolPermissionApproveSafeOperations =>
      'Approuver les opérations sûres';

  @override
  String get toolPermissionApproveSafeOperationsDescription =>
      'Lit les fichiers du projet et d’OpenChat sans demander. Demande avant changements, Web et terminal.';

  @override
  String get toolPermissionFullAccess => 'Accès complet';

  @override
  String get toolPermissionFullAccessDescription =>
      'Aucune demande : fichiers accessibles partout, Web et terminal sans autorisation.';

  @override
  String get toolPermissionSettingsLoadFailed =>
      'Impossible de charger les paramètres d’accès aux outils.';

  @override
  String get toolPermissionSettingsSaveFailed =>
      'Impossible d’enregistrer les paramètres d’accès aux outils. Réessayez.';

  @override
  String get toolPermissionRequestTitle => 'Autorisation de l\'outil';

  @override
  String get toolPermissionRequestDescription =>
      'L\'IA souhaite utiliser cet outil à l\'emplacement sélectionné. L\'autorisation s\'applique uniquement à cet appel.';

  @override
  String get toolPermissionRequestExpired =>
      'Cette demande d\'autorisation d\'outil n\'est plus active.';

  @override
  String get toolPermissionResponseFailed =>
      'Votre choix n’a pas pu être envoyé. Réessayez.';

  @override
  String get toolPermissionContent => 'Contenu à écrire';

  @override
  String get toolPermissionOldText => 'Texte à trouver';

  @override
  String get toolPermissionNewText => 'Texte de remplacement';

  @override
  String get toolPermissionQuery => 'Rechercher du texte';

  @override
  String get toolPermissionOffset => 'Position de départ';

  @override
  String get toolPermissionLimit => 'Résultats maximaux';

  @override
  String get toolPermissionStartLine => 'Ligne de départ';

  @override
  String get toolPermissionLineCount => 'Nombre de lignes';

  @override
  String get toolPermissionIncludeHidden => 'Inclure les fichiers cachés';

  @override
  String get commonYes => 'Oui';

  @override
  String get commonNo => 'Non';

  @override
  String get toolPermissionTarget => 'Emplacement à accéder';

  @override
  String get toolPermissionTool => 'Outil';

  @override
  String get toolPermissionArguments => 'Détails de la demande';

  @override
  String get toolPermissionDeny => 'Refuser';

  @override
  String get toolPermissionStopResponse => 'Arrêter la réponse';

  @override
  String get toolPermissionAllowOnce => 'Autoriser cet appel';

  @override
  String get toolDenied => 'Refusé';

  @override
  String get toolCancelled => 'Annulé';

  @override
  String get toolAwaitingApproval => 'En attente d\'approbation';

  @override
  String get conversationExportToolActivity => 'Activité de l\'outil';

  @override
  String get responseReplaceFailed =>
      'La nouvelle réponse a été enregistrée, mais la réponse précédente n\'a pas pu être remplacée.';

  @override
  String get responseRetryNotCompleted =>
      'La nouvelle réponse n’est pas terminée. La réponse précédente a été conservée.';

  @override
  String get responseRetryCleanupFailed =>
      'Impossible de nettoyer la nouvelle tentative. Actualisez l’historique des conversations.';

  @override
  String get responseInProgress => 'Réponse en cours';

  @override
  String get responseRetryUnavailable =>
      'Cette réponse ne peut pas être réessayée. Commencez plutôt un nouveau message.';

  @override
  String responseVersionCount(int current, int total) {
    return 'Réponse $current sur $total';
  }

  @override
  String get previousResponseVersion => 'Réponse précédente';

  @override
  String get nextResponseVersion => 'Réponse suivante';

  @override
  String get providerRateLimited =>
      'Le fournisseur a signalé une limite d’utilisation. Réessayez plus tard.';

  @override
  String get providerAuthenticationRequired =>
      'Le fournisseur a rejeté la demande. Vérifiez la connexion et l\'accès au modèle.';

  @override
  String get providerToolRequestRejected =>
      'Le fournisseur a rejeté une requête contenant des outils. Vérifiez la prise en charge des outils par le modèle ou choisissez un modèle qui l’indique.';

  @override
  String get providerNetworkUnavailable =>
      'Le fournisseur est injoignable. Vérifiez votre connexion et réessayez.';

  @override
  String get contextWindowExceeded =>
      'La conversation est trop volumineuse pour la fenêtre contextuelle de ce modèle. Raccourcissez le dernier message ou choisissez un modèle avec une fenêtre contextuelle plus grande. Votre historique de discussion est enregistré.';

  @override
  String get openCodeFreeTierRestricted =>
      'Les modèles gratuits OpenCode ne sont disponibles que dans l\'application OpenCode.';

  @override
  String get apiKey => 'Clé API';

  @override
  String get apiKeyInputLabel => 'Clé API';

  @override
  String get apiKeyRequired => 'Entrez une clé API.';

  @override
  String get apiKeyInvalidFormat =>
      'Entrez une clé API OpenAI valide. Elle doit commencer par sk-.';

  @override
  String get apiKeyAlreadySaved => 'Cette clé API est déjà enregistrée.';

  @override
  String get apiKeySaved =>
      'Clé API enregistrée en toute sécurité sur cet appareil.';

  @override
  String get apiKeySaveFailed =>
      'Impossible d’enregistrer la clé API en toute sécurité. Réessayez.';

  @override
  String get apiKeyLoadFailed =>
      'Les clés API enregistrées n\'ont pas pu être chargées.';

  @override
  String get savedApiKey => 'Clé enregistrée';

  @override
  String savedApiKeyWithSuffix(String suffix) {
    return 'Clé se terminant par ••••$suffix';
  }

  @override
  String get showApiKey => 'Afficher la clé API';

  @override
  String get hideApiKey => 'Masquer la clé API';

  @override
  String get retry => 'Réessayer';

  @override
  String get save => 'Enregistrer';

  @override
  String get saving => 'Enregistrement…';

  @override
  String get oauth => 'OAuth';

  @override
  String get oauthSigningIn => 'Connexion…';

  @override
  String get oauthBrowserWaiting => 'Terminez la connexion dans le navigateur.';

  @override
  String get oauthConnectionsLoadFailed =>
      'Les connexions ChatGPT OAuth n\'ont pas pu être chargées.';

  @override
  String get oauthSignInFailed =>
      'La connexion à ChatGPT n’a pas pu être effectuée. Vérifiez le navigateur et réessayez.';

  @override
  String get oauthConnectionAdded => 'Compte ChatGPT connecté.';

  @override
  String get oldCredentialCleanupFailed =>
      'Le compte est connecté, mais un ancien identifiant enregistré n’a pas pu être supprimé. Redémarrez OpenChat et réessayez.';

  @override
  String get connectionSelectionFailed =>
      'Le compte ChatGPT n\'a pas pu être sélectionné.';

  @override
  String get removeChatGptConnection => 'Supprimer la connexion ChatGPT';

  @override
  String get removeConnectionAction => 'Supprimer';

  @override
  String confirmRemoveConnection(String name) {
    return 'Supprimer $name et les identifiants enregistrés associés ? Les conversations et messages existants resteront sur cet appareil.';
  }

  @override
  String get connectionRemoveSucceeded =>
      'Connexion supprimée. Les discussions et messages existants sont toujours disponibles.';

  @override
  String get connectionRemoveFailed =>
      'Impossible de supprimer la connexion. Réessayez.';

  @override
  String get workspaceSelectionFailed =>
      'Impossible de sélectionner l’espace de travail ChatGPT.';

  @override
  String get chatGptAccount => 'Compte ChatGPT';

  @override
  String get planUnavailable => 'Forfait indisponible';

  @override
  String accountPlan(String plan) {
    return 'Plan : $plan';
  }

  @override
  String get connectionNeedsSignIn =>
      'Connectez-vous à nouveau pour utiliser ce compte.';

  @override
  String get connectionSelected => 'Sélectionné';

  @override
  String get useConnection => 'Utiliser ce compte';

  @override
  String get selectWorkspace => 'Choisir un espace de travail';

  @override
  String get workspaceWithoutName => 'Espace de travail du compte ChatGPT';

  @override
  String get workspace => 'Espace de travail du compte ChatGPT';

  @override
  String get workspaceUnavailable =>
      'Aucune information sur l’espace de travail du compte ChatGPT n’est disponible.';

  @override
  String get accountEmailUnavailable => 'Adresse e-mail indisponible';

  @override
  String accountUsage(String plan) {
    return 'Utilisation · $plan';
  }

  @override
  String get refreshUsage => 'Actualiser l\'utilisation';

  @override
  String get ordinaryUsageAvailable => 'L’utilisation normale est disponible.';

  @override
  String get ordinaryUsageUnavailable =>
      'L’utilisation normale n’est actuellement pas disponible.';

  @override
  String get ordinaryUsageUnknown =>
      'La disponibilité de l\'utilisation n\'a pas pu être déterminée.';

  @override
  String usageUpdatedAt(String time) {
    return 'Mis à jour : $time';
  }

  @override
  String get usageLoadFailed =>
      'Les informations d\'utilisation n\'ont pas pu être chargées.';

  @override
  String get usageFiveHour => '5 heures';

  @override
  String get usageWeekly => 'Hebdomadaire';

  @override
  String get usageMonthly => 'Mensuel';

  @override
  String workspaceNumbered(int number) {
    return 'Espace de travail $number du compte ChatGPT';
  }

  @override
  String quotaResetsAt(String time) {
    return 'Réinitialisation : $time';
  }

  @override
  String creditExpiresAt(String time) {
    return 'Expire : $time';
  }

  @override
  String creditGrantedAt(String time) {
    return 'Accordé : $time';
  }

  @override
  String get noResetCredits => 'Aucun crédit de réinitialisation disponible.';

  @override
  String usageUsedPercent(String percent) {
    return '$percent % utilisés';
  }

  @override
  String get resetCreditCountUnavailable =>
      'Nombre de crédits de réinitialisation indisponible.';

  @override
  String resetCreditsAvailable(int count) {
    return 'Crédits de réinitialisation disponibles : $count';
  }

  @override
  String get resetCredit => 'Crédit de réinitialisation';

  @override
  String get statusUnavailable => 'Statut indisponible';

  @override
  String get resetCreditDetailsUnavailable =>
      'Les détails du crédit de réinitialisation n\'ont pas été renvoyés.';

  @override
  String get resetCreditAvailableStatus => 'Disponible';

  @override
  String get useResetCredit => 'Utiliser le crédit';

  @override
  String get resetCreditRedeeming => 'Utilisation…';

  @override
  String get confirmResetCreditTitle =>
      'Utiliser ce crédit de réinitialisation ?';

  @override
  String get confirmResetCreditMessage =>
      'Un crédit de réinitialisation va être utilisé. Cette action est irréversible. Continuer ?';

  @override
  String get confirmResetCreditAction => 'Utiliser le crédit';

  @override
  String get resetCreditApplied =>
      'La limite d\'utilisation a été réinitialisée.';

  @override
  String get resetCreditAlreadyUsed =>
      'Ce crédit de réinitialisation a déjà été utilisé.';

  @override
  String get resetCreditNothingToReset =>
      'Il n’y a aucune limite d’utilisation à réinitialiser pour le moment.';

  @override
  String get resetCreditNoLongerAvailable =>
      'Ce crédit de réinitialisation n\'est plus disponible. Actualisez les informations d’utilisation.';

  @override
  String get resetCreditOutcomeUnknown =>
      'Le résultat n\'a pas pu être confirmé. Actualisez les informations d\'utilisation avant d\'utiliser à nouveau ce crédit.';

  @override
  String get resetCreditRefreshRequired =>
      'Le résultat précédent n\'a pas pu être confirmé. Actualisez les informations d\'utilisation avant de réessayer.';

  @override
  String get resetCreditRejected =>
      'ChatGPT n’a pas accepté la demande de réinitialisation pour ce compte.';

  @override
  String get resetCreditSignInRequired =>
      'Reconnectez-vous à votre compte ChatGPT, puis réessayez.';

  @override
  String get noChatGptConnections => 'Aucune connexion ChatGPT pour l’instant.';

  @override
  String get apiKeyConnectionUnavailable =>
      'La connexion par clé API n’a pas encore été ajoutée.';

  @override
  String get oauthConnectionUnavailable =>
      'La connexion OAuth n’a pas encore été ajoutée.';

  @override
  String get titleGenerationTarget => 'Titres de conversation automatiques';

  @override
  String get titleGenerationTargetDescription =>
      'Les titres sont générés séparément des réponses. Choisissez comment OpenChat doit les créer.';

  @override
  String get titleUseConversationModel =>
      'Utiliser le modèle et le compte de la conversation';

  @override
  String get titleConversationModeDescription =>
      'Utilisez le même modèle et le même compte que la conversation.';

  @override
  String get titleUseSpecificModel => 'Choisir un modèle connecté';

  @override
  String get titleSpecificModelDescription =>
      'Choisissez parmi les fournisseurs connectés et les modèles locaux enregistrés.';

  @override
  String get titleUseLightweightModel =>
      'Laisser OpenChat choisir le plus petit modèle';

  @override
  String get titleLightweightModeDescription =>
      'Choisissez le plus petit modèle dont le nombre de paramètres est connu.';

  @override
  String get titleSelectModelHint =>
      'Choisir un fournisseur connecté ou un modèle local';

  @override
  String get titleLightweightUnavailable =>
      'Aucun modèle disponible n’indique explicitement son nombre de paramètres dans son nom. Vous pouvez choisir un modèle directement.';

  @override
  String titleLightweightModelSelected(String model) {
    return 'OpenChat a choisi $model, le plus petit modèle dont le nombre de paramètres est connu.';
  }

  @override
  String get titleUseConversationAccount =>
      'Utiliser le compte de conversation';

  @override
  String get titleAccountUnavailable =>
      'Le compte de titres sélectionné n’est pas disponible';

  @override
  String get titleWorkspaceHint =>
      'Choisissez un espace de travail pour les titres';

  @override
  String get titleWorkspaceRequired =>
      'Choisissez un espace de travail avant que ce compte puisse générer des titres.';

  @override
  String get titlePreferenceLoadFailed =>
      'La préférence du compte titre n\'a pas pu être chargée.';

  @override
  String get titlePreferenceSaveFailed =>
      'La préférence du compte titre n\'a pas pu être enregistrée.';

  @override
  String get appearance => 'Apparence';

  @override
  String get themeSettingDescription =>
      'Choisissez l\'apparence de l\'application.';

  @override
  String get conversationWidth => 'Largeur des réponses';

  @override
  String get conversationWidthDescription =>
      'Choisissez la largeur de ligne utilisée pour les réponses de conversation.';

  @override
  String get widthNarrow => 'Étroite';

  @override
  String get widthNormal => 'Normale';

  @override
  String get widthWide => 'Large';

  @override
  String get conversationTextSize => 'Taille du texte';

  @override
  String get conversationTextSizeDescription =>
      'Choisissez la taille du texte utilisée dans l\'application.';

  @override
  String get textSizeSmall => 'Petite';

  @override
  String get textSizeNormal => 'Normale';

  @override
  String get textSizeLarge => 'Grande';

  @override
  String get appFont => 'Police des réponses';

  @override
  String get appFontDescription =>
      'Choisissez la police des réponses de l’assistant.';

  @override
  String get appearancePreferenceSaveFailed =>
      'Impossible d’enregistrer la préférence d’apparence. Réessayez.';

  @override
  String get language => 'Langue de l\'application';

  @override
  String get languageSettingDescription =>
      'Choisissez la langue utilisée par l\'application.';

  @override
  String get systemLanguage => 'Langue de l\'appareil';

  @override
  String get englishLanguage => 'Anglais';

  @override
  String get turkishLanguage => 'Turc';

  @override
  String get spanishLanguage => 'Espagnol';

  @override
  String get germanLanguage => 'Allemand';

  @override
  String get frenchLanguage => 'Français';

  @override
  String get languageSaveFailed =>
      'Impossible d’enregistrer la préférence de langue. Réessayez.';

  @override
  String get systemTheme => 'Système';

  @override
  String get lightTheme => 'Clair';

  @override
  String get darkTheme => 'Sombre';

  @override
  String get attachmentIndexTitle => 'Rechercher dans les pièces jointes texte';

  @override
  String get attachmentIndexDescription =>
      'Les fichiers texte pris en charge sont copiés dans un index local, dans la limite de 5 000 fichiers ou 100 Mo. Les fichiers source restent inchangés ; les images ne sont pas indexées.';

  @override
  String get attachmentIndexSupportedTypes =>
      'Types indexés : .txt, .md, .csv, .json, .jsonl, .xml, .html, .css, .js, .jsx, .ts, .tsx, .dart, .rs, .py, .go, .java, .kt, .swift, .c, .h, .cpp, .hpp, .cs, .sh, .ps1, .yaml, .yml, .toml, .ini, .sql, .log';

  @override
  String attachmentIndexCount(int count) {
    return '$count fichiers indexés';
  }

  @override
  String get attachmentIndexRebuild => 'Reconstruire l’index';

  @override
  String get attachmentIndexClear => 'Effacer l’index';

  @override
  String get attachmentIndexFailed =>
      'L’index texte des pièces jointes n’a pas pu être mis à jour.';

  @override
  String get localData => 'Données locales';

  @override
  String get conversationArchiveTitle => 'Archives de conversations';

  @override
  String get conversationArchiveDescription =>
      'Créez ou restaurez une archive chiffrée des conversations sélectionnées sur cet appareil.';

  @override
  String get conversationArchiveIncludesNotice =>
      'L’archive conserve les messages sélectionnés, les entrées et sorties des outils, les résumés de raisonnement, les paramètres mémoire par conversation et les pièces jointes. Le texte des conversations peut contenir des informations sensibles ou des chemins locaux. Les identifiants des fournisseurs, les comptes liés, les liens de projet et les paramètres globaux sont exclus.';

  @override
  String get conversationArchiveUnavailable =>
      'Le service d’archivage local ou l’historique des conversations est indisponible.';

  @override
  String get exportConversations => 'Exporter les conversations';

  @override
  String get importConversations => 'Importer une archive';

  @override
  String get conversationArchiveNoConversations =>
      'Aucune conversation à exporter.';

  @override
  String get conversationArchiveSelectTitle =>
      'Choisir les conversations à exporter';

  @override
  String get conversationArchiveSearch => 'Rechercher des conversations';

  @override
  String conversationArchiveSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversations sélectionnées',
      one: '# conversation sélectionnée',
      zero: 'Aucune conversation sélectionnée',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchiveSelectAll =>
      'Tout sélectionner parmi les résultats';

  @override
  String get conversationArchiveDeselectAll =>
      'Tout désélectionner parmi les résultats';

  @override
  String get conversationArchiveNoMatches =>
      'Aucune conversation correspondante.';

  @override
  String get conversationArchivePassphrase => 'Phrase secrète de l’archive';

  @override
  String get conversationArchiveConfirmPassphrase =>
      'Confirmer la phrase secrète';

  @override
  String get conversationArchivePassphraseHint => '12 caractères minimum';

  @override
  String get conversationArchivePassphraseTooShort =>
      'Utilisez au moins 12 caractères.';

  @override
  String get conversationArchivePassphraseTooLong =>
      'La phrase secrète ne doit pas dépasser 512 octets.';

  @override
  String get conversationArchivePassphraseMismatch =>
      'Les phrases secrètes ne correspondent pas.';

  @override
  String get conversationArchivePassphraseRecovery =>
      'Conservez cette phrase en lieu sûr. OpenChat ne peut pas la récupérer.';

  @override
  String get conversationArchiveChooseFolder =>
      'Choisir le dossier où enregistrer l’archive chiffrée';

  @override
  String get conversationArchiveChooseFile =>
      'Choisir une archive de conversations OpenChat';

  @override
  String conversationArchiveExportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Archive chiffrée créée pour # conversations.',
      one: 'Archive chiffrée créée pour # conversation.',
    );
    return '$_temp0';
  }

  @override
  String conversationArchiveImportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversations importées.',
      one: '# conversation importée.',
      zero: 'Aucune conversation importée.',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchivePickerFailed =>
      'Le sélecteur de fichiers ou de dossiers n’a pas pu s’ouvrir.';

  @override
  String get conversationArchiveInvalidFile =>
      'Le fichier sélectionné ne fournit pas de chemin exploitable.';

  @override
  String get conversationArchiveInvalidResponse =>
      'Le service d’archive a renvoyé des données invalides.';

  @override
  String get conversationArchiveExportFailed =>
      'L’archive des conversations n’a pas pu être créée.';

  @override
  String get conversationArchiveImportFailed =>
      'L’archive des conversations n’a pas pu être importée.';

  @override
  String get profileArchiveTitle => 'Sauvegarde des données de l’application';

  @override
  String get profileArchiveDescription =>
      'Créez ou restaurez une copie chiffrée de la base de données des conversations et des pièces jointes associées.';

  @override
  String get profileArchiveIncludesNotice =>
      'Les identifiants des fournisseurs, les préférences globales et les fichiers des modèles ou des moteurs ne sont pas inclus. Lors d’une restauration, la base de données et les pièces jointes actuelles sont conservées dans un dossier de récupération.';

  @override
  String get profileArchiveUnavailable =>
      'Le service local de sauvegarde du profil n’est pas disponible.';

  @override
  String get profileArchiveExport => 'Sauvegarder les données';

  @override
  String get profileArchiveRestore => 'Restaurer les données';

  @override
  String get profileArchiveChooseFolder =>
      'Choisissez le dossier où enregistrer la sauvegarde chiffrée';

  @override
  String get profileArchiveChooseFile =>
      'Choisissez une sauvegarde de profil OpenChat';

  @override
  String profileArchiveExportSuccess(
    String conversationCount,
    String messageCount,
    String attachmentCount,
  ) {
    return 'Sauvegarde chiffrée créée : $conversationCount conversations, $messageCount messages et $attachmentCount pièces jointes.';
  }

  @override
  String get profileArchiveRestoreConfirmTitle =>
      'Remplacer les données de l’application ?';

  @override
  String get profileArchiveRestoreConfirmBody =>
      'La sauvegarde choisie remplacera la base de données des conversations et les pièces jointes associées au prochain démarrage d’OpenChat. La base de données et les pièces jointes actuelles seront conservées dans un dossier de récupération. Les identifiants, préférences globales et fichiers de modèles ou de moteurs ne sont pas inclus.';

  @override
  String get profileArchiveRestoreConfirmButton =>
      'Restaurer et fermer OpenChat';

  @override
  String get profileArchiveRestartTitle =>
      'Fermez OpenChat pour appliquer la sauvegarde';

  @override
  String profileArchiveRestoreReady(
    int conversationCount,
    int messageCount,
    int attachmentCount,
  ) {
    return 'La sauvegarde contient $conversationCount conversations, $messageCount messages et $attachmentCount pièces jointes. Fermez OpenChat maintenant. Les données restaurées seront vérifiées au démarrage et le profil actuel sera conservé pour récupération.';
  }

  @override
  String get profileArchiveCloseApp => 'Fermer OpenChat';

  @override
  String get profileArchiveCloseFailed =>
      'OpenChat n’a pas pu être fermé. Fermez la fenêtre pour appliquer la sauvegarde.';

  @override
  String get profileArchiveProcessing =>
      'La sauvegarde du profil est chiffrée ou vérifiée. Les sauvegardes volumineuses peuvent prendre plusieurs minutes.';

  @override
  String get profileArchivePickerFailed =>
      'Le sélecteur de fichiers ou de dossiers n’a pas pu s’ouvrir.';

  @override
  String get profileArchiveInvalidFile =>
      'Le fichier sélectionné ne fournit pas de chemin exploitable.';

  @override
  String get profileArchiveInvalidResponse =>
      'Le service de sauvegarde du profil a renvoyé des données invalides.';

  @override
  String get profileArchiveExportFailed =>
      'La sauvegarde chiffrée du profil n’a pas pu être créée.';

  @override
  String get profileArchiveRestoreFailed =>
      'La sauvegarde du profil n’a pas pu être préparée pour restauration.';

  @override
  String get profileArchiveInvalidArchive =>
      'La sauvegarde est invalide ou le mot de passe est incorrect.';

  @override
  String get profileArchivePassphraseInvalid =>
      'Utilisez un mot de passe d’au moins 12 caractères et de 512 octets maximum.';

  @override
  String get profileArchiveNotFound =>
      'La sauvegarde de profil sélectionnée est introuvable.';

  @override
  String get profileArchiveConflict =>
      'Une restauration est déjà en attente ou le fichier de destination existe déjà.';

  @override
  String get profileArchiveBusy =>
      'Une autre opération de sauvegarde du profil est en cours.';

  @override
  String get profileArchiveStorageFailed =>
      'La sauvegarde du profil n’a pas pu être lue, écrite ou restaurée en toute sécurité.';

  @override
  String get profileArchiveLimitExceeded =>
      'La sauvegarde du profil dépasse une limite de taille ou de nombre d’éléments.';

  @override
  String get profileArchiveSchemaUnsupported =>
      'Cette sauvegarde a été créée avec une version plus récente du schéma OpenChat.';

  @override
  String get profileArchiveTakingLong =>
      'La sauvegarde du profil prend plus de temps que prévu. Attendez la fin de l’opération avant de réessayer.';

  @override
  String get profileArchiveOperationFailed =>
      'L’opération de sauvegarde du profil n’a pas pu aboutir.';

  @override
  String get conversationArchivePassphraseTitle => 'Déverrouiller l’archive';

  @override
  String get conversationArchivePreviewTitle =>
      'Vérifier le contenu de l’archive';

  @override
  String get conversationArchiveCreatedAt => 'Créée le';

  @override
  String get conversationArchiveConversationCount => 'Conversations';

  @override
  String get conversationArchiveMessageCount => 'Messages';

  @override
  String get conversationArchiveAttachmentCount => 'Pièces jointes';

  @override
  String get conversationArchiveDuplicateCount =>
      'Conversations déjà présentes sur cet appareil';

  @override
  String get conversationArchiveSkipDuplicates =>
      'Ignorer les conversations déjà présentes';

  @override
  String get conversationArchiveImportCopies =>
      'Importer les doublons en tant que copies distinctes';

  @override
  String get conversationArchiveRestoreNotice =>
      'Les conversations restaurées ne seront liées ni aux comptes fournisseurs ni aux projets. Sélectionnez à nouveau un modèle pour les poursuivre.';

  @override
  String get conversationArchiveInvalidPassphraseOrFile =>
      'La phrase secrète est incorrecte ou l’archive est invalide.';

  @override
  String get conversationArchivePassphraseInvalid =>
      'La longueur de la phrase secrète n’est pas prise en charge.';

  @override
  String get conversationArchiveNotFound =>
      'L’archive ou la conversation sélectionnée est introuvable.';

  @override
  String get conversationArchiveConflict =>
      'Un fichier existe déjà à cet emplacement. Choisissez un autre dossier ou résolvez d’abord le conflit.';

  @override
  String get conversationArchiveBusy =>
      'Arrêtez les exécutions actives de l’assistant dans les conversations sélectionnées avant de les exporter.';

  @override
  String get conversationArchiveStorageFailed =>
      'L’archive n’a pas pu être lue, écrite ou restaurée en toute sécurité.';

  @override
  String get conversationArchiveLimitExceeded =>
      'L’archive dépasse la taille maximale prise en charge.';

  @override
  String get conversationArchiveTakingLong =>
      'L’opération prend plus de temps que prévu et est peut-être toujours en cours. Vérifiez la liste des conversations avant de réessayer.';

  @override
  String get conversationArchiveOperationFailed =>
      'L’opération d’archive a échoué. Vérifiez le fichier sélectionné et l’espace disque disponible, puis réessayez.';

  @override
  String get conversationArchiveProcessing =>
      'Chiffrement ou vérification de l’archive. Les grandes archives peuvent prendre plusieurs minutes.';

  @override
  String get conversationHistory => 'Historique des discussions';

  @override
  String get historyDeviceDescription =>
      'Les conversations sont stockées sur cet appareil.';

  @override
  String get historyDeviceStatus => 'Sur cet appareil';

  @override
  String get historyCheckingDescription =>
      'Préparation de l’historique local des conversations.';

  @override
  String get historyCheckingStatus => 'Préparation';

  @override
  String get historyStorageUnavailableDescription =>
      'L\'historique des discussions locales n\'a pas pu être ouvert.';

  @override
  String get historyStorageCorruptDescription =>
      'La base de données locale est endommagée. Aucune mise à jour du schéma n\'a été appliquée. Restaurez une sauvegarde vérifiée pour continuer.';

  @override
  String get historyStorageBackupFailedDescription =>
      'OpenChat n\'a pas pu vérifier une sauvegarde avant la mise à jour et l\'a interrompue. Vérifiez l\'espace disque disponible, puis réessayez.';

  @override
  String get historyStorageUnavailableStatus => 'Indisponible';

  @override
  String get historyLoading => 'Chargement des conversations…';

  @override
  String get historyLoadFailed =>
      'L\'historique des discussions n\'a pas pu être chargé. Redémarrez l\'application.';

  @override
  String get messageHistoryLoadFailed =>
      'Les messages de cette conversation n\'ont pas pu être chargés.';

  @override
  String get messageHistoryLoading =>
      'Chargement des messages de conversation.';

  @override
  String get clearConversationHistory => 'Effacer tout l\'historique';

  @override
  String get clearConversationHistoryDescription =>
      'Supprimez définitivement les conversations et les messages stockés sur cet appareil.';

  @override
  String get confirmClearHistoryTitle =>
      'Effacer tout l\'historique des discussions ?';

  @override
  String get confirmClearHistoryBody =>
      'Cela supprime définitivement toutes les discussions et messages stockés sur cet appareil. Cette action ne peut pas être annulée.';

  @override
  String get cancel => 'Annuler';

  @override
  String get continueLabel => 'Continuer';

  @override
  String get deleteAll => 'Tout supprimer';

  @override
  String get clearingHistory => 'Suppression…';

  @override
  String get clearHistorySucceeded =>
      'L\'historique des discussions a été supprimé.';

  @override
  String get clearHistoryFailed =>
      'Impossible de supprimer l’historique des conversations. Réessayez.';

  @override
  String get themeSaveFailed =>
      'Impossible d’enregistrer la préférence de thème. Réessayez.';

  @override
  String get searchChats => 'Rechercher dans OpenChat';

  @override
  String get searchChatsHint =>
      'Rechercher dans les discussions, projets, espaces de travail, réponses enregistrées et noms de fichiers';

  @override
  String get searchFilterAll => 'Tout';

  @override
  String get searchFilterChats => 'Discussions';

  @override
  String get searchFilterProjects => 'Projets';

  @override
  String get searchFilterWorkspaces => 'Espaces de travail';

  @override
  String get searchFilterSavedResponses => 'Réponses enregistrées';

  @override
  String get searchFilterAttachments => 'Pièces jointes';

  @override
  String get searchMessagesTooltip => 'Rechercher des messages';

  @override
  String get historySearchDateFilter => 'Filtrer par date';

  @override
  String get historySearchDateFilterApplied => 'Le filtre de date est actif';

  @override
  String get historySearchFiltersTitle => 'Filtres de recherche';

  @override
  String get historySearchFiltersLoading =>
      'Chargement des filtres de recherche…';

  @override
  String get historySearchRouteFilterNote =>
      'Le fournisseur et le modèle correspondent à la route enregistrée avec chaque réponse.';

  @override
  String get historySearchResultTypeFilter => 'Type de résultat';

  @override
  String get historySearchAllResultTypes => 'Messages et conversations';

  @override
  String get historySearchMessagesOnly => 'Messages uniquement';

  @override
  String get historySearchConversationsOnly =>
      'Titres des conversations uniquement';

  @override
  String get historySearchSortFilter => 'Trier par';

  @override
  String get historySearchSortRelevance => 'Pertinence';

  @override
  String get historySearchSortNewest => 'Plus récent';

  @override
  String get historySearchSortOldest => 'Plus ancien';

  @override
  String get historySearchProviderFilter => 'Fournisseur de la réponse';

  @override
  String get historySearchModelFilter => 'Modèle de la réponse';

  @override
  String get historySearchProjectFilter => 'Projet';

  @override
  String get historySearchArchiveFilter => 'État de l’archive';

  @override
  String get historySearchAllProviders => 'Tous les fournisseurs';

  @override
  String get historySearchAllModels => 'Tous les modèles';

  @override
  String get historySearchAllProjects => 'Tous les projets';

  @override
  String get historySearchTagFilter => 'Tag';

  @override
  String get selectConversations => 'Sélectionner des conversations';

  @override
  String get cancelSelection => 'Annuler la sélection';

  @override
  String get archiveSelectedConversations =>
      'Archiver les conversations sélectionnées';

  @override
  String get moveSelectedChats => 'Déplacer les conversations sélectionnées';

  @override
  String get moveToChats => 'Déplacer vers les conversations';

  @override
  String get bookmarkConversation => 'Ajouter la conversation aux favoris';

  @override
  String get removeConversationBookmark => 'Retirer des favoris';

  @override
  String get conversationBookmarkFailed =>
      'Le favori de la conversation n’a pas pu être mis à jour.';

  @override
  String get conversationBookmarked => 'Dans les favoris';

  @override
  String selectConversation(String title) {
    return 'Sélectionner $title';
  }

  @override
  String conversationsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversations sélectionnées',
      one: '# conversation sélectionnée',
      zero: 'Aucune conversation sélectionnée',
    );
    return '$_temp0';
  }

  @override
  String get historySearchAllTags => 'Tous les tags';

  @override
  String get historySearchAllStatuses => 'Toutes les conversations';

  @override
  String get historySearchActiveConversations => 'Conversations actives';

  @override
  String get historySearchArchivedConversations => 'Conversations archivées';

  @override
  String get historySearchClearFilters => 'Effacer les filtres';

  @override
  String get historySearchApplyFilters => 'Appliquer les filtres';

  @override
  String get historySearchOpenFilters => 'Ouvrir les filtres de recherche';

  @override
  String get historySearchFiltersActive =>
      'Les filtres de recherche sont actifs';

  @override
  String get historySearchClearDateFilter => 'Effacer le filtre de date';

  @override
  String get searchMessagesHeader => 'Résultats dans les messages';

  @override
  String get searchMessagesLoading =>
      'Recherche dans les messages et les conversations…';

  @override
  String get searchMessagesNoResults => 'Aucun élément correspondant trouvé.';

  @override
  String get searchMessagesTooShort =>
      'Saisissez au moins deux caractères pour rechercher.';

  @override
  String get searchMessagesTooLong =>
      'Le texte de recherche ne peut pas dépasser 512 caractères.';

  @override
  String get searchMessagesFailed => 'La recherche a échoué. Réessayez.';

  @override
  String get searchMessageUnavailable => 'Ce message n’est plus disponible.';

  @override
  String get projects => 'Projets';

  @override
  String get noProjects => 'Aucun projet pour l\'instant';

  @override
  String get createProject => 'Créer un projet de code';

  @override
  String get projectName => 'Nom du projet de code';

  @override
  String get projectNameRequired => 'Entrez un nom de projet.';

  @override
  String get projectFolder => 'Dossier du projet de code';

  @override
  String get projectFolderDescription =>
      'Les projets de code relient les conversations à un dossier de cet appareil pour accéder aux fichiers et aux outils. Utilisez les espaces de travail pour regrouper des conversations sans dossier.';

  @override
  String get chooseProjectFolder => 'Choisir un dossier';

  @override
  String get projectFolderNotSelected =>
      'Choisissez un dossier pour continuer.';

  @override
  String get projectFolderSelectionFailed =>
      'Le dossier n\'a pas pu être sélectionné.';

  @override
  String get projectCreateFailed => 'Le projet n\'a pas pu être créé.';

  @override
  String get projectCreated => 'Projet créé.';

  @override
  String get projectLoadFailed => 'Les projets n\'ont pas pu être chargés.';

  @override
  String get projectLoading => 'Chargement des projets…';

  @override
  String get projectMoveFailed =>
      'Impossible de déplacer la conversation vers le projet.';

  @override
  String get pinnedChats => 'Conversations épinglées';

  @override
  String get noPinnedChats => 'Aucune discussion épinglée pour l\'instant';

  @override
  String get noChatsTitle => 'Aucune conversation';

  @override
  String get noChatsSearchTitle => 'Aucune conversation à rechercher';

  @override
  String get showMore => 'Afficher plus';

  @override
  String get showLess => 'Afficher moins';

  @override
  String get projectOptions => 'Options du projet';

  @override
  String get projectToolRulesTitle => 'Autorisations des outils du projet';

  @override
  String get projectToolRulesDescription =>
      'Choisissez une règle pour chaque outil. Les outils sans règle suivent le mode d’autorisation global. Autoriser respecte toujours les limites de fichiers et l’isolation des processus.';

  @override
  String get projectToolRulesSummaryLoading =>
      'Chargement des règles d’autorisation des outils de ce projet.';

  @override
  String get projectToolRulesSummaryUnavailable =>
      'Impossible de charger le résumé des autorisations des outils de ce projet.';

  @override
  String get projectToolRulesSummaryInherit =>
      'Ce projet n’a aucune dérogation. Tous les outils suivent le mode global.';

  @override
  String projectToolRulesSummary(int allowed, int approval, int denied) {
    return 'Règles du projet : $allowed autorisent, $approval exigent une approbation et $denied bloquent. Les outils sans règle suivent le mode global.';
  }

  @override
  String get projectToolRuleInherit => 'Utiliser le réglage global';

  @override
  String get projectToolRuleAsk => 'Demander une autorisation';

  @override
  String get projectToolRuleAllow => 'Autoriser';

  @override
  String get projectToolRuleDeny => 'Refuser';

  @override
  String get projectToolRulesLoadFailed =>
      'Les autorisations des outils du projet n’ont pas pu être chargées. Vérifiez les réglages enregistrés avant d’envoyer une autre requête.';

  @override
  String get projectToolRulesSaveFailed =>
      'Les autorisations des outils du projet n’ont pas pu être enregistrées. Les règles précédentes restent actives.';

  @override
  String get projectOptionsTitle => 'Options du projet';

  @override
  String get projectDefaultModelSourceLabel => 'Valeur par défaut du projet';

  @override
  String get projectDefaultModelUseCurrent =>
      'Utiliser le modèle actuel pour ce projet';

  @override
  String get projectDefaultModelClear =>
      'Effacer le modèle par défaut du projet';

  @override
  String get projectDefaultModelSelectFirst =>
      'Sélectionnez un modèle avant de définir le modèle par défaut du projet.';

  @override
  String get projectDefaultModelSaveFailed =>
      'Impossible d’enregistrer le modèle par défaut du projet.';

  @override
  String projectDefaultModelSaved(String model) {
    return 'Le modèle par défaut du projet est $model.';
  }

  @override
  String get projectDefaultModelCleared =>
      'Le modèle par défaut du projet a été effacé.';

  @override
  String get projectOptionsToolPermissions => 'Autorisations des outils';

  @override
  String get projectOptionsMcpServers => 'Serveurs MCP';

  @override
  String get projectOptionsWorktrees => 'Worktrees Git';

  @override
  String get projectOptionsInstructions => 'Instructions du projet';

  @override
  String get projectOptionsSkills => 'Skills du projet';

  @override
  String get projectInstructionsTitle => 'Instructions du projet';

  @override
  String get projectInstructionsDescription =>
      'Ces instructions sont envoyées avec les nouvelles réponses de ce projet. Elles sont enregistrées dans .openchat/instructions.md et limitées à 16 Kio.';

  @override
  String get projectInstructionsHint =>
      'Saisissez des consignes propres au projet';

  @override
  String get projectInstructionsLoadFailed =>
      'Impossible de charger les instructions du projet. Vérifiez le fichier et réessayez.';

  @override
  String get projectInstructionsSaveFailed =>
      'Impossible d’enregistrer les instructions du projet. Vérifiez le dossier du projet et réessayez.';

  @override
  String get projectInstructionsSave => 'Enregistrer les instructions';

  @override
  String projectInstructionsSize(int bytes, int maxBytes) {
    return '$bytes / $maxBytes octets';
  }

  @override
  String get projectIndexTitle => 'Index du projet';

  @override
  String get projectIndexDescription =>
      'Indexe localement les fichiers texte du projet. L’index respecte .gitignore et est stocké hors du dossier du projet.';

  @override
  String get projectIndexEnable => 'Utiliser l’index du projet dans le chat';

  @override
  String get projectIndexSync => 'Synchroniser';

  @override
  String get projectIndexClear => 'Effacer l’index';

  @override
  String get projectIndexLoadFailed =>
      'Impossible de charger l’état de l’index du projet.';

  @override
  String get projectIndexActionFailed =>
      'L’opération sur l’index du projet a échoué.';

  @override
  String get projectIndexStale =>
      'L’index du projet est incomplet. Synchronisez-le dans les options du projet ou désactivez-le.';

  @override
  String projectIndexStatus(Object status, Object files, Object bytes) {
    return 'État : $status. Fichiers : $files. Octets indexés : $bytes.';
  }

  @override
  String get projectSkillsTitle => 'Skills du projet';

  @override
  String get projectSkillsDescription =>
      'Choisissez les Skills du projet dans .openchat/skills/<id>/SKILL.md. Le contenu sélectionné est envoyé avec les nouvelles réponses. La sélection est enregistrée localement pour ce projet.';

  @override
  String get projectSkillsEmpty =>
      'Aucun Skill trouvé. Ajoutez un fichier SKILL.md sous .openchat/skills/<id>/.';

  @override
  String get projectSkillsLoadFailed =>
      'Impossible de charger les Skills du projet. Vérifiez les fichiers du projet et réessayez.';

  @override
  String get projectSkillsSaveFailed =>
      'Impossible d’enregistrer la sélection des Skills du projet.';

  @override
  String get projectSkillsClearSelection => 'Effacer la sélection';

  @override
  String get projectSkillsStale =>
      'Ce Skill sélectionné n’est plus disponible. Décochez-le pour effacer la sélection enregistrée.';

  @override
  String get projectMcpTitle => 'Serveurs MCP du projet';

  @override
  String get projectMcpDescription =>
      'Configurez des serveurs MCP locaux sur stdio pour ce projet. Les serveurs activés démarrent uniquement lorsqu’un chat compatible avec les outils en a besoin.';

  @override
  String get projectMcpPermissionsLocal =>
      'Les autorisations sont enregistrées localement pour ce projet et ne sont pas stockées dans le dépôt.';

  @override
  String get projectMcpPermissionScope => 'Autorisation pour ce serveur';

  @override
  String get projectMcpNoCredentials =>
      'N’ajoutez pas d’identifiants aux arguments. Ajoutez des noms de variables d’environnement, puis stockez leurs valeurs de manière sécurisée avec le bouton de clé.';

  @override
  String get projectMcpEmpty =>
      'Aucun serveur MCP n’est configuré pour ce projet.';

  @override
  String get projectMcpAdd => 'Ajouter un serveur';

  @override
  String get projectMcpEdit => 'Modifier le serveur';

  @override
  String get projectMcpRemove => 'Supprimer le serveur';

  @override
  String get projectMcpServerId => 'ID du serveur';

  @override
  String get projectMcpProgram => 'Chemin absolu du programme';

  @override
  String get projectMcpArguments => 'Arguments';

  @override
  String get projectMcpArgumentsHint =>
      'Saisissez un argument par ligne. Les lignes vides sont ignorées.';

  @override
  String get projectMcpEnabled => 'Activé';

  @override
  String get projectMcpCheck => 'Vérifier la connexion';

  @override
  String get projectMcpChecking => 'Vérification de la connexion…';

  @override
  String projectMcpConnected(int count) {
    return 'Connecté ; $count outils détectés';
  }

  @override
  String get projectMcpCheckFailed =>
      'Le serveur n’a pas pu démarrer ou fournir sa liste d’outils.';

  @override
  String get projectMcpLoadFailed =>
      'Le catalogue MCP du projet n’a pas pu être chargé. Vérifiez les entrées invalides ou non sûres dans `.openchat/mcp.json`.';

  @override
  String get projectMcpSaveFailed =>
      'Le catalogue MCP n’a pas pu être enregistré. Vérifiez l’unicité des ID et les chemins absolus des programmes.';

  @override
  String get projectMcpDuplicateId => 'Chaque serveur doit avoir un ID unique.';

  @override
  String get projectMcpInvalidId =>
      'Utilisez 1 à 24 lettres minuscules, chiffres ou traits de soulignement.';

  @override
  String get projectMcpProgramRequired =>
      'Saisissez un chemin absolu vers le programme du serveur.';

  @override
  String get projectMcpTransport => 'Type de connexion';

  @override
  String get projectMcpTransportStdio => 'Processus local (stdio)';

  @override
  String get projectMcpTransportHttp => 'Serveur distant (Streamable HTTP)';

  @override
  String get projectMcpEndpoint => 'Point de terminaison HTTPS';

  @override
  String get projectMcpEndpointHint =>
      'Utilisez le point HTTPS du serveur MCP distant. Les adresses privées et locales sont bloquées.';

  @override
  String get projectMcpEndpointRequired =>
      'Saisissez un point de terminaison HTTPS public valide.';

  @override
  String get projectMcpAuthVariable => 'Nom de l’identifiant sécurisé';

  @override
  String get projectMcpAuthVariableHint =>
      'Facultatif. Enregistrez sa valeur avec le bouton de clé après avoir ajouté le serveur.';

  @override
  String get projectMcpEnvironmentVariables =>
      'Variables d’environnement des identifiants';

  @override
  String get projectMcpEnvironmentVariablesHint =>
      'Saisissez un nom de variable par ligne. Enregistrez le serveur, puis utilisez le bouton avec la clé pour stocker les valeurs de manière sécurisée.';

  @override
  String get projectMcpEnvironmentVariablesInvalid =>
      'Saisissez jusqu’à 32 noms de variable uniques avec des lettres, chiffres et traits de soulignement.';

  @override
  String get projectMcpCredentials => 'Identifiants MCP';

  @override
  String get projectMcpEnvironmentVariablesRequired =>
      'Modifiez le serveur et ajoutez des noms de variables avant de stocker des identifiants.';

  @override
  String get projectMcpCredentialHint =>
      'Les valeurs sont stockées dans le Gestionnaire d’informations d’identification Windows et transmises uniquement à ce processus serveur.';

  @override
  String get projectMcpSecret => 'Valeur secrète';

  @override
  String get projectMcpCredentialStored =>
      'Une valeur est stockée de manière sécurisée.';

  @override
  String get projectMcpCredentialMissing => 'Aucune valeur n’est stockée.';

  @override
  String get projectMcpCredentialSave => 'Stocker de manière sécurisée';

  @override
  String get projectMcpCredentialRemove => 'Supprimer la valeur stockée';

  @override
  String get projectMcpCredentialUnavailable =>
      'Impossible d’accéder à l’identifiant. Vérifiez le Gestionnaire d’informations d’identification Windows et réessayez.';

  @override
  String get projectMcpSecretRequired =>
      'Saisissez une valeur avant de l’enregistrer.';

  @override
  String get projectMcpSecretTooLarge =>
      'La valeur doit faire 2 500 octets ou moins.';

  @override
  String get projectWorktreesTitle => 'Worktrees isolés';

  @override
  String get projectWorktreesDescription =>
      'Créez une branche à partir du commit actuel dans un dossier distinct. Les changements non commités ne sont pas copiés.';

  @override
  String get projectWorktreesLoading => 'Chargement des worktrees…';

  @override
  String get projectWorktreesEmpty =>
      'Aucun worktree n’a été créé pour ce projet.';

  @override
  String get projectWorktreeCreate => 'Créer un worktree';

  @override
  String get projectWorktreeCreateFailed =>
      'Impossible de créer le worktree. Vérifiez que ce dossier est un dépôt Git.';

  @override
  String get projectWorktreeLoadFailed =>
      'Impossible de charger les worktrees du projet.';

  @override
  String get projectWorktreeOperationFailed =>
      'L’opération Git a échoué. Vérifiez l’état du dépôt et réessayez.';

  @override
  String get projectWorktreeNotRepository =>
      'Ce dossier de projet ne se trouve pas dans un dépôt Git.';

  @override
  String projectWorktreeBranch(String branch) {
    return 'Branche : $branch';
  }

  @override
  String projectWorktreePath(String path) {
    return 'Dossier : $path';
  }

  @override
  String get projectWorktreeStatusClean => 'Aucune modification non validée';

  @override
  String projectWorktreeStatusChanges(int count) {
    return 'Fichiers modifiés : $count';
  }

  @override
  String get projectWorktreeReview => 'Examiner les modifications';

  @override
  String get projectWorktreeUse => 'Utiliser comme projet';

  @override
  String get projectWorktreeRemove => 'Supprimer le worktree';

  @override
  String get projectWorktreeRemoveTitle => 'Supprimer le worktree ?';

  @override
  String projectWorktreeRemoveDescription(String branch) {
    return 'Les fichiers non validés et non suivis de $branch seront supprimés. La branche et ses commits sont conservés.';
  }

  @override
  String get projectWorktreeReviewTitle => 'Modifications du worktree';

  @override
  String get projectWorktreeNoChanges =>
      'Aucune modification non validée. Les commits restent sur cette branche.';

  @override
  String get projectWorktreeStagedDiff => 'Modifications indexées';

  @override
  String get projectWorktreeUnstagedDiff => 'Modifications non indexées';

  @override
  String get projectWorktreeListTruncated =>
      'Seuls les 20 premiers worktrees sont affichés.';

  @override
  String get projectWorktreeCheckFailed =>
      'Impossible d’inspecter le worktree.';

  @override
  String get projectWorktreeRunCheck => 'Exécuter la vérification';

  @override
  String get projectWorktreeTaskPickerTitle =>
      'Choisir une vérification nommée';

  @override
  String get projectWorktreeTaskEmpty =>
      'Aucune vérification nommée pour ce worktree. Ajoutez des tâches dans `.openchat/tasks.json` du projet.';

  @override
  String get projectWorktreeTaskLoadFailed =>
      'Impossible de charger les vérifications nommées depuis ce worktree.';

  @override
  String get projectWorktreeTaskConfirmationTitle =>
      'Exécuter cette vérification ?';

  @override
  String get projectWorktreeTaskCommand => 'Commande';

  @override
  String projectWorktreeTaskTimeout(int seconds) {
    return 'Limite de temps : $seconds secondes';
  }

  @override
  String get projectWorktreeTaskRun => 'Exécuter la vérification';

  @override
  String projectWorktreeTaskRunning(String task) {
    return 'Vérification en cours : $task';
  }

  @override
  String get projectWorktreeTaskStopping => 'Arrêt de la vérification…';

  @override
  String get projectWorktreeTaskStop => 'Arrêter la vérification';

  @override
  String get projectWorktreeTaskCancelled => 'La vérification a été annulée.';

  @override
  String get projectWorktreeTaskTimedOut =>
      'La vérification a atteint sa limite de temps.';

  @override
  String projectWorktreeTaskExitCode(int code) {
    return 'Vérification terminée avec le code $code.';
  }

  @override
  String get projectWorktreeTaskExitCodeUnavailable =>
      'La vérification s’est terminée sans code de sortie.';

  @override
  String get projectWorktreeTaskOutputTruncated =>
      'La sortie est limitée aux 128 premiers Kio.';

  @override
  String get projectWorktreeTaskRunFailed =>
      'Impossible d’exécuter la vérification dans le bac à sable du worktree.';

  @override
  String projectWorktreeTaskResultTitle(String task) {
    return 'Résultat de la vérification : $task';
  }

  @override
  String get projectWorktreeTaskOutput => 'Sortie';

  @override
  String get projectWorktreeTaskNoOutput =>
      'La vérification n’a produit aucune sortie.';

  @override
  String get projectWorktreeTaskDenied =>
      'Les autorisations du projet interdisent les vérifications nommées.';

  @override
  String get agentRunManagerTitle => 'Exécutions';

  @override
  String get agentRunManagerDescription =>
      'Consultez les travaux actifs, en pause ou interrompus dans vos conversations.';

  @override
  String get agentRunLoading => 'Chargement des exécutions';

  @override
  String get agentRunLoadFailed =>
      'Impossible de charger l’état des exécutions.';

  @override
  String get agentRunEmpty =>
      'Aucune exécution active, en pause ou interrompue.';

  @override
  String get agentRunRefresh => 'Actualiser la liste';

  @override
  String get agentRunOpenConversation => 'Ouvrir la conversation';

  @override
  String get agentRunStatusRunning => 'En cours';

  @override
  String get agentRunStatusPaused => 'En pause';

  @override
  String get agentRunStatusInterrupted => 'Interrompue';

  @override
  String get agentRunStatusUnavailable => 'État indisponible';

  @override
  String get agentRunStatusCompleted => 'Terminée';

  @override
  String get agentRunStatusFailed => 'Échec';

  @override
  String get agentRunStatusCancelled => 'Annulée';

  @override
  String get agentRunSubagent => 'Exécution enfant';

  @override
  String agentRunSubagentTask(String objective) {
    return 'Tâche enfant : $objective';
  }

  @override
  String get agentRunLiveStarting => 'Démarrage de l’analyse déléguée…';

  @override
  String get agentRunLiveThinking => 'Analyse de la tâche déléguée…';

  @override
  String agentRunLiveUsingTool(String tool) {
    return 'Utilisation de $tool';
  }

  @override
  String get agentRunEndedInAnotherChat =>
      'Une exécution dans une autre conversation est terminée.';

  @override
  String get newProjectConversation =>
      'Démarrer une nouvelle discussion de projet';

  @override
  String get newConversation => 'Démarrer une nouvelle conversation';

  @override
  String get conversationTitle => 'Nouvelle discussion';

  @override
  String get conversationMemory => 'Mémoire de la conversation';

  @override
  String get conversationMemoryDescription =>
      'Examinez le contexte compacté de cette conversation et recherchez ses anciens messages.';

  @override
  String conversationMemoryCurrentConversation(String title) {
    return 'Conversation sélectionnée : $title';
  }

  @override
  String get conversationMemoryNoConversation =>
      'Ouvrez une conversation pour inspecter sa mémoire.';

  @override
  String get contextUsageTitle => 'Utilisation du contexte';

  @override
  String contextUsageUsed(String count) {
    return 'Utilisation : environ $count jetons';
  }

  @override
  String contextUsageSummary(String used, String limit, String percent) {
    return '~$used / $limit jetons ($percent)';
  }

  @override
  String contextUsageModelLimit(String count) {
    return '$count jetons';
  }

  @override
  String get contextUsageNoModelLimit => 'Limite du modèle inconnue.';

  @override
  String contextUsageProviderMeasurement(String count) {
    return 'Dernière mesure du fournisseur : $count jetons';
  }

  @override
  String contextUsageInstructionsEstimate(String count, String percent) {
    return 'Instructions : $count jetons · $percent';
  }

  @override
  String contextUsageToolDefinitionsEstimate(String count, String percent) {
    return 'Définitions d’outils : $count jetons · $percent';
  }

  @override
  String contextUsageMessagesEstimate(String count, String percent) {
    return 'Messages : $count jetons · $percent';
  }

  @override
  String contextUsageAttachmentsEstimate(String count, String percent) {
    return 'Pièces jointes : $count jetons · $percent';
  }

  @override
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name : $count jetons · $percent';
  }

  @override
  String contextUsageDraftAttachment(String name) {
    return 'Brouillon · $name';
  }

  @override
  String contextUsageUserMessagesEstimate(String count, String percent) {
    return 'Utilisateur : $count jetons · $percent';
  }

  @override
  String contextUsageAssistantMessagesEstimate(String count, String percent) {
    return 'Assistant : $count jetons · $percent';
  }

  @override
  String contextUsageToolsEstimate(String count, String percent) {
    return 'Utilisation des outils : $count jetons · $percent';
  }

  @override
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name : $count jetons · $percent';
  }

  @override
  String contextUsageMemoryEstimate(String count, String percent) {
    return 'Mémoire compactée : $count jetons · $percent';
  }

  @override
  String contextUsageDraftEstimate(String count, String percent) {
    return 'Brouillon : $count jetons · $percent';
  }

  @override
  String contextUsageFreeSpaceEstimate(String count, String percent) {
    return 'Espace libre : jetons $count · $percent';
  }

  @override
  String get contextUsageOverLimit => 'Limite du modèle dépassée.';

  @override
  String get contextUsageMeasurementUnavailable =>
      'Les détails de la mémoire n\'ont pas pu être chargés.';

  @override
  String get contextUsageInstructionUnavailable =>
      'Les détails des instructions n\'ont pas pu être chargés.';

  @override
  String get contextUsageConfigurationUnavailable =>
      'Impossible de charger les estimations des instructions et des définitions d’outils.';

  @override
  String get contextUsageConfigurationLoading =>
      'Préparation des estimations des instructions et des définitions d’outils…';

  @override
  String get conversationMemorySemanticTitle => 'Recherche sémantique';

  @override
  String get conversationMemorySemanticDescription =>
      'Téléchargez un modèle multilingue d\'environ 136 Mo pour retrouver les anciens messages formulés différemment.';

  @override
  String get conversationMemorySemanticPrepare => 'Préparer';

  @override
  String get conversationMemorySemanticChecking =>
      'Vérification de la recherche sémantique locale…';

  @override
  String get conversationMemorySemanticPreparing =>
      'Téléchargement et vérification du modèle…';

  @override
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  ) {
    return '$percent% téléchargé · $downloaded / $total MB';
  }

  @override
  String get conversationMemorySemanticIndexing =>
      'Création de l’index des archives locales…';

  @override
  String get conversationMemorySemanticCancelling =>
      'Annulation du téléchargement…';

  @override
  String get conversationMemorySemanticDownloadCancelled =>
      'Téléchargement annulé. La recherche par mot-clé reste disponible.';

  @override
  String get conversationMemorySemanticPrepareFailed =>
      'Impossible de préparer le modèle. Réessayez ; les parties téléchargées valides seront réutilisées.';

  @override
  String get conversationMemorySemanticKeywordSearchFallback =>
      'La recherche par mots-clés reste disponible avant la préparation.';

  @override
  String get conversationMemorySemanticReady =>
      'La recherche sémantique locale est prête';

  @override
  String get conversationMemorySemanticIndexNotice =>
      'Le modèle fonctionne sur cet appareil. La première recherche peut indexer les messages plus anciens et les détails des outils enregistrés localement et prendre plus de temps.';

  @override
  String get conversationMemorySummaryTitle => 'Contexte compacté';

  @override
  String get conversationMemoryNoSummary =>
      'Il n’y a pas encore de résumé compacté.';

  @override
  String get conversationMemoryCheckpointDescription =>
      'Le fournisseur stocke le contexte sous forme de point de contrôle réutilisable au lieu d\'un texte récapitulatif lisible. L\'historique complet des messages reste dans les archives.';

  @override
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  ) {
    return 'Dernière demande · $provider · $model · Jetons d\'entrée $count';
  }

  @override
  String get conversationMemorySearchTitle => 'Rechercher dans les archives';

  @override
  String conversationWorkspaceContext(String workspace) {
    return 'Espace de travail : $workspace';
  }

  @override
  String outputSourceProvider(String provider) {
    return 'Fournisseur : $provider';
  }

  @override
  String outputSourceModel(String model) {
    return 'Modèle : $model';
  }

  @override
  String get conversationMemorySearching =>
      'Recherche dans les conversations archivées…';

  @override
  String get conversationMemorySearchHint =>
      'Entrez un sujet ou une expression plus ancienne...';

  @override
  String get conversationMemorySearchAction => 'Rechercher';

  @override
  String get conversationMemorySearchQueryTooShort =>
      'Entrez au moins deux caractères à rechercher.';

  @override
  String get conversationMemorySearchInstruction =>
      'Les messages terminés et les résultats des outils sont recherchés uniquement dans cette conversation.';

  @override
  String get conversationMemoryArchiveSettingsTitle =>
      'Indexation de l’archive';

  @override
  String get conversationMemoryArchiveSettingsDescription =>
      'Choisissez les textes enregistrés de la conversation qui peuvent être ajoutés à la recherche locale de l’archive. Désactiver une option supprime ses index de recherche et sémantiques dérivés ; la conversation d’origine reste enregistrée.';

  @override
  String get conversationMemoryArchiveIncludeConversation =>
      'Inclure cette conversation';

  @override
  String get conversationMemoryArchiveConversationIncluded =>
      'Les messages et les résultats d’outils inclus peuvent apparaître dans la recherche de l’archive.';

  @override
  String get conversationMemoryArchiveConversationExcluded =>
      'Les index dérivés de cette conversation sont supprimés et elle ne sera plus indexée.';

  @override
  String get conversationMemoryArchiveToolsTitle => 'Résultats des outils';

  @override
  String get conversationMemoryArchiveToolIncluded =>
      'Les détails enregistrés de cet outil peuvent apparaître dans la recherche de l’archive.';

  @override
  String get conversationMemoryArchiveToolExcluded =>
      'Les détails enregistrés de cet outil sont supprimés des index de l’archive.';

  @override
  String get conversationMemoryArchiveNoTools =>
      'Aucun résultat d’outil terminé n’est enregistré dans cette conversation.';

  @override
  String get conversationMemoryArchiveSettingsSaveFailed =>
      'Impossible d’enregistrer les paramètres d’indexation de l’archive. Réessayez.';

  @override
  String get conversationMemorySearchNoResults =>
      'Aucune entrée correspondante dans l’archive.';

  @override
  String get conversationMemorySearchFailed =>
      'Impossible de rechercher dans l’archive de la conversation. Réessayez.';

  @override
  String get conversationMemoryLoadFailed =>
      'Impossible de charger le contexte compacté. Réessayez.';

  @override
  String get conversationMemoryResetAction => 'Réinitialiser le contexte';

  @override
  String get conversationMemoryResetTitle =>
      'Réinitialiser le contexte compacté ?';

  @override
  String get conversationMemoryResetConfirmation =>
      'Le contexte compacté enregistré et la dernière mesure de demande seront supprimés. L’historique complet des messages et les archives resteront ; le contexte compacté peut être recréé lors d’une requête ultérieure si nécessaire.';

  @override
  String get conversationMemoryResetConfirm => 'Réinitialiser';

  @override
  String get conversationMemoryResetFailed =>
      'Le contexte compacté n\'a pas pu être réinitialisé.';

  @override
  String get conversationMemoryUserMessage => 'Message utilisateur';

  @override
  String get conversationMemoryAssistantMessage => 'Réponse de l\'assistant';

  @override
  String get renameConversation => 'Modifier le titre du chat';

  @override
  String get editConversationTags => 'Modifier les tags';

  @override
  String get conversationTagsDialogTitle => 'Tags de la conversation';

  @override
  String get conversationTagsFieldLabel => 'Tags';

  @override
  String get conversationTagsFieldHint => 'Séparez les tags par des virgules';

  @override
  String get conversationTagsHelp => 'Jusqu’à 12 tags de 32 caractères chacun.';

  @override
  String get conversationTagsSaveFailed => 'Impossible d’enregistrer les tags.';

  @override
  String get saveHistorySearchTitle =>
      'Enregistrer la recherche dans l’historique';

  @override
  String get savedHistorySearchName => 'Nom de la recherche';

  @override
  String get savedHistorySearchesTitle => 'Recherches enregistrées';

  @override
  String get savedHistorySearchesEmpty =>
      'Aucune recherche enregistrée pour le moment.';

  @override
  String get deleteSavedHistorySearch => 'Supprimer la recherche enregistrée';

  @override
  String get saveCurrentHistorySearch => 'Enregistrer la recherche actuelle';

  @override
  String get savedHistorySearchesLoadFailed =>
      'Impossible de charger les recherches enregistrées.';

  @override
  String get savedHistorySearchSaveFailed =>
      'Impossible de modifier les recherches enregistrées.';

  @override
  String get savedHistorySearchLimitReached =>
      'Vous pouvez enregistrer jusqu’à 20 recherches.';

  @override
  String get pinConversation => 'Épingler le chat';

  @override
  String get unpinConversation => 'Désépingler le chat';

  @override
  String get conversationTitleRequired =>
      'Le titre du chat ne peut pas être vide.';

  @override
  String get conversationBranchEditTitle =>
      'Modifier le message et créer une branche';

  @override
  String get conversationBranchEditLabel => 'Message';

  @override
  String get conversationBranchStart => 'Créer la branche';

  @override
  String conversationBranchTitle(String title) {
    return '$title (branche)';
  }

  @override
  String get conversationBranchCreateFailed =>
      'La branche du message n’a pas pu être créée.';

  @override
  String get conversationTitleSaveFailed =>
      'Le titre du chat n\'a pas pu être enregistré.';

  @override
  String get conversationModelSaveFailed =>
      'Le modèle de discussion n\'a pas pu être enregistré. Le modèle précédent est toujours sélectionné.';

  @override
  String get moreOptions => 'Plus d\'options';

  @override
  String get emptyChatWelcomeTitle => 'Par quoi commencer ?';

  @override
  String get emptyChatWelcomeBody =>
      'Posez une question ou décrivez ce que vous avez en tête.';

  @override
  String get emptyChatSelectModelTitle => 'Choisissez un modèle pour commencer';

  @override
  String get emptyChatSelectModelBody =>
      'Choisissez un modèle dans le menu ci-dessous, puis écrivez votre message.';

  @override
  String get switchToDarkMode => 'Passer au thème sombre';

  @override
  String get switchToLightMode => 'Passer au thème clair';

  @override
  String get theme => 'Thème';

  @override
  String get keyboardHint =>
      'Entrée pour envoyer · Maj+Entrée pour une nouvelle ligne';

  @override
  String get minimizeWindow => 'Réduire la fenêtre';

  @override
  String get maximizeWindow => 'Agrandir la fenêtre';

  @override
  String get restoreWindow => 'Restaurer la fenêtre';

  @override
  String get modelSelection => 'Choisir le modèle';

  @override
  String get noModelConnected => 'Aucun modèle n\'est encore connecté';

  @override
  String get noModelConnectedBody =>
      'Dans Connexions, connectez-vous à ChatGPT, ajoutez une clé API d’un fournisseur compatible ou choisissez un modèle local disponible. Vérifiez la connexion, puis sélectionnez un modèle ci-dessous.';

  @override
  String get providerNeedsVerificationTitle =>
      'Vérifiez la connexion du fournisseur';

  @override
  String get providerNeedsVerificationBody =>
      'Une session ou une clé API enregistrée ne confirme pas que le fournisseur est accessible. Ouvrez Connexions pour vérifier la connexion et actualiser la liste des modèles.';

  @override
  String get close => 'Fermer';

  @override
  String get reasoning => 'Raisonnement';

  @override
  String get reasoningMedium => 'Moyen';

  @override
  String get reasoningMinimal => 'Minimal';

  @override
  String get reasoningLow => 'Faible';

  @override
  String get reasoningHigh => 'Élevé';

  @override
  String get reasoningExtraHigh => 'Très élevé';

  @override
  String get reasoningMax => 'Maximum';

  @override
  String get reasoningUltra => 'Ultra';

  @override
  String get reasoningDefault => 'Par défaut';

  @override
  String get reasoningDefaultHint =>
      'Aucun niveau de raisonnement personnalisé n\'est envoyé. Le comportement par défaut du fournisseur est utilisé ; le niveau n\'est pas adapté à la difficulté de la tâche.';

  @override
  String get stop => 'Arrêter';

  @override
  String get modelsLoading => 'Chargement des modèles...';

  @override
  String get modelsUnavailable => 'Modèles indisponibles';

  @override
  String get noModelsAvailable =>
      'Aucun modèle n\'est actuellement disponible pour ce compte.';

  @override
  String get providerDataUnavailable =>
      'Le fournisseur a renvoyé des données qu’OpenChat n’a pas pu lire. Actualisez la connexion et réessayez.';

  @override
  String get oauthResponseInvalid =>
      'Le résultat de la connexion n\'a pas pu être lu. Essayez de vous connecter à nouveau.';

  @override
  String get modelCatalogUnavailable =>
      'La liste des modèles n\'est pas disponible. Actualisez la connexion du fournisseur et réessayez.';

  @override
  String get messageSaveFailed =>
      'Votre message n\'a pas pu être enregistré dans l\'historique des discussions locales.';

  @override
  String get chatHistoryUnavailable =>
      'Impossible de mettre à jour l’historique des conversations. Réessayez.';

  @override
  String get chatRequestFailed =>
      'La réponse n\'a pas pu être complétée. Vos messages enregistrés sont toujours disponibles.';

  @override
  String get goalAlreadyActive =>
      'Reprenez ou arrêtez l’objectif actif avant d’en démarrer un autre dans cette conversation.';

  @override
  String get goalStateUnavailable =>
      'OpenChat n’a pas pu vérifier si cette conversation a déjà un objectif actif. Réessayez.';

  @override
  String get cachedCatalog => 'modèles en cache';

  @override
  String get attachFile => 'Joindre un fichier';

  @override
  String get attachmentsUnavailable =>
      'Les pièces jointes ne sont pas encore disponibles.';

  @override
  String get removeAttachment => 'Supprimer la pièce jointe';

  @override
  String get previewImage => 'Agrandir l’image';

  @override
  String get attachmentUnavailable =>
      'Cette pièce jointe n\'est pas disponible.';

  @override
  String get attachmentCountExceeded =>
      'Vous pouvez joindre jusqu\'à 10 fichiers et 3 images.';

  @override
  String get attachmentFileTooLarge =>
      'Le fichier dépasse la limite de taille autorisée.';

  @override
  String get attachmentTotalTooLarge =>
      'Les pièces jointes ne peuvent pas dépasser 14 Mo au total.';

  @override
  String get unsupportedAttachmentFile =>
      'Ce type de fichier n\'est pas pris en charge.';

  @override
  String get attachmentReadFailed =>
      'Le fichier n\'a pas pu être lu. Sélectionnez-le à nouveau et réessayez.';

  @override
  String get attachmentSaveFailed =>
      'La pièce jointe n\'a pas pu être enregistrée. Sélectionnez-le à nouveau et réessayez.';

  @override
  String get attachmentMustBeUtf8 =>
      'Les pièces jointes texte doivent être encodées en UTF-8.';

  @override
  String get attachmentInvalidImage =>
      'Le format du fichier image n\'a pas pu être vérifié.';

  @override
  String get modelDoesNotSupportImages =>
      'Le modèle sélectionné ne prend pas en charge les pièces jointes d\'images.';

  @override
  String get messageHint => 'Écrire un message...';

  @override
  String get send => 'Envoyer';

  @override
  String get welcomeTitle => 'Démarrer une conversation';

  @override
  String get welcomeBody =>
      'Connectez un modèle d\'IA avant de démarrer une conversation.';

  @override
  String get historyOpen => 'Ouvrir l\'historique des discussions';

  @override
  String get assistantDisclaimer =>
      'Les réponses de l’IA peuvent être inexactes. Vérifiez les informations importantes.';

  @override
  String get modelRequired =>
      'Connectez un modèle avant d\'envoyer un message.';

  @override
  String get selectedModelUnavailable =>
      'Ce modèle n\'est plus disponible. Choisissez un autre modèle.';

  @override
  String get messageModelUnavailable => 'Modèle indisponible';

  @override
  String get userMessage => 'Utilisateur';

  @override
  String get copyMessage => 'Copier le message';

  @override
  String get messageCopied => 'Message copié dans le presse-papiers.';

  @override
  String get messageCopyFailed =>
      'Le message n\'a pas pu être copié dans le presse-papiers.';

  @override
  String get today => 'Aujourd\'hui';

  @override
  String get unavailableTime => '—:—';

  @override
  String get unavailableValue => '—';

  @override
  String responseTokenRate(String rate) {
    return '$rate jetons/s';
  }

  @override
  String responseTokenCount(String count) {
    return '$count jetons';
  }

  @override
  String get responseCompleted => 'Réponse terminée';

  @override
  String responseCompletedWithDuration(String duration) {
    return 'Réponse terminée · $duration';
  }

  @override
  String get reasoningSummary => 'Résumé du raisonnement';

  @override
  String reasoningSummaryWithDuration(String duration) {
    return 'Résumé du raisonnement · $duration';
  }

  @override
  String get reasoningSummaryTooltip =>
      'Un résumé fourni par le modèle. La durée correspond au temps qu\'il a fallu pour arriver dans le flux ; il ne s’agit pas de données de chaîne de pensée cachées.';

  @override
  String get toolRunning => 'En cours';

  @override
  String get unknownTool => 'Autre outil';

  @override
  String terminalSession(String terminal) {
    return 'Terminal · $terminal';
  }

  @override
  String get toolWaitingForUser => 'En attente de votre réponse';

  @override
  String get toolCompleted => 'Terminé';

  @override
  String get toolFailed => 'Échec';

  @override
  String get toolInput => 'Entrée';

  @override
  String get toolOutput => 'Sortie';

  @override
  String get toolListFiles => 'Lister les fichiers';

  @override
  String get toolSearchFiles => 'Rechercher des fichiers';

  @override
  String get toolReadFile => 'Lire le fichier';

  @override
  String get toolGetFileInfo => 'Obtenir des informations sur le fichier';

  @override
  String get toolWriteFile => 'Écrire dans un fichier';

  @override
  String get toolEditFile => 'Modifier le fichier';

  @override
  String get toolExecuteCommand => 'Exécuter la commande';

  @override
  String get toolRunProjectTask => 'Exécuter la tâche du projet';

  @override
  String toolConfiguredProjectTool(String name) {
    return 'Outil du projet : $name';
  }

  @override
  String get toolDelegateTask => 'Déléguer l’analyse';

  @override
  String get toolPermissionTask => 'Tâche';

  @override
  String get toolPermissionTimeout => 'Délai (secondes)';

  @override
  String get toolSendTerminalInput => 'Envoyer l\'entrée du terminal';

  @override
  String get toolGitStatus => 'État Git';

  @override
  String get toolGitDiff => 'Diff Git';

  @override
  String get toolGitHistory => 'Historique Git';

  @override
  String get toolGitBranch => 'Branche';

  @override
  String get toolGitUpstream => 'Branche distante';

  @override
  String get toolGitAhead => 'En avance';

  @override
  String get toolGitBehind => 'En retard';

  @override
  String get toolGitStaged => 'Indexé';

  @override
  String get toolGitUnstaged => 'Non indexé';

  @override
  String get toolGitNoChanges => 'L’arbre de travail est propre.';

  @override
  String get toolGitNoDiff => 'Aucun diff à afficher.';

  @override
  String get toolGitNoHistory => 'Aucun commit trouvé.';

  @override
  String get toolWebSearch => 'Recherche sur le Web';

  @override
  String get toolReadUrlContent => 'Lire la page Web';

  @override
  String get toolSearchQuery => 'Requête de recherche';

  @override
  String get toolLocalWebSource => 'Recherche Web locale OpenChat';

  @override
  String get toolLocalPageSource => 'Lecture locale de page OpenChat';

  @override
  String get toolProviderSource => 'Source du fournisseur';

  @override
  String toolSourceRetrievedAt(String time) {
    return 'Consulté : $time';
  }

  @override
  String get toolSourceDetails => 'Détails de la source';

  @override
  String toolCitationSource(String sourceId) {
    return 'Source $sourceId';
  }

  @override
  String get toolWebSearchNoResults => 'Aucun résultat Web trouvé.';

  @override
  String toolWebSearchResultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count résultats',
      one: '1 résultat',
      zero: '0 résultat',
    );
    return '$_temp0';
  }

  @override
  String get toolUrl => 'URL';

  @override
  String toolReadUrlLength(int count) {
    return 'Caractères $count';
  }

  @override
  String get toolOpenUrl => 'Ouvrir dans le navigateur';

  @override
  String get toolCopyUrl => 'Copier URL';

  @override
  String get toolCopyContent => 'Copier le contenu';

  @override
  String get toolCopyFailed => 'Le contenu n\'a pas pu être copié.';

  @override
  String get toolOperationWorking => 'Cette opération est en cours.';

  @override
  String get toolOperationFailed => 'L\'opération n\'a pas pu être complétée.';

  @override
  String get toolOperationUnavailable =>
      'Le résultat n\'a pas pu être affiché.';

  @override
  String get toolOperationTruncated =>
      'Seule une partie du résultat est disponible.';

  @override
  String get toolSearchNoMatches => 'Aucune correspondance trouvée.';

  @override
  String get toolSearchMoreResults =>
      'D’autres correspondances sont disponibles.';

  @override
  String toolSearchMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count correspondances',
      one: '1 correspondance',
      zero: '0 correspondance',
    );
    return '$_temp0';
  }

  @override
  String get toolReadNoLines => 'Il n\'y a aucune ligne dans cette plage.';

  @override
  String get toolReadMoreLines => 'D\'autres lignes sont disponibles.';

  @override
  String toolReadLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count lignes',
      one: '1 ligne',
      zero: '0 ligne',
    );
    return '$_temp0';
  }

  @override
  String get toolFileTypeFile => 'Fichier';

  @override
  String get toolFileTypeDirectory => 'Dossier';

  @override
  String toolWriteSuccess(String size) {
    return '$size écrit';
  }

  @override
  String get toolFilePreview => 'Aperçu du contenu écrit';

  @override
  String toolEditSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# modifications appliquées',
      one: '# modification appliquée',
    );
    return '$_temp0';
  }

  @override
  String get toolEditBefore => 'Avant';

  @override
  String get toolEditAfter => 'Après';

  @override
  String get toolPermissionCommand => 'Commande';

  @override
  String get toolPermissionTerminalId => 'ID du terminal';

  @override
  String get toolPermissionInput => 'Entrée du terminal';

  @override
  String get toolTerminalNoOutput => 'Aucune sortie produite.';

  @override
  String get toolTerminalWaitingOutput =>
      'En attente de sortie ou d\'entrée...';

  @override
  String get toolTerminalRunning => 'En cours d\'exécution...';

  @override
  String get toolTerminalTerminated => 'Terminé';

  @override
  String get toolTerminalWaitingForInput => 'En attente d’une entrée';

  @override
  String toolTerminalExitCode(int code) {
    return 'Code de sortie : $code';
  }

  @override
  String get toolTerminalCopied => 'Copié dans le presse-papiers';

  @override
  String get toolTechnicalDetails => 'Détails';

  @override
  String toolFileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments',
      one: '1 élément',
      zero: 'Aucun élément',
    );
    return '$_temp0';
  }

  @override
  String get toolEmptyListing =>
      'Il n\'y a aucun élément à afficher dans ce dossier.';

  @override
  String get toolListingUnavailable =>
      'Cette liste de fichiers n\'a pas pu être affichée.';

  @override
  String get toolListingIncomplete =>
      'Certains éléments n\'ont pas pu être répertoriés.';

  @override
  String get toolMoreFilesAvailable => 'D’autres éléments sont disponibles.';

  @override
  String get toolDesktopLocation => 'Bureau';

  @override
  String get toolProjectLocation => 'Dossier du projet';

  @override
  String get toolOpenChatLocation => 'Dossier de données d\'application';

  @override
  String get responseFailed => 'Échec de la réponse';

  @override
  String get responseStopped => 'Réponse arrêtée';

  @override
  String secondsShort(int count) {
    return '$count sec';
  }

  @override
  String get usageQuotas => 'Quotas d\'utilisation';

  @override
  String get usageQuotasDescription =>
      'Affichez les quotas restants, les limites d\'utilisation et les temps de réinitialisation pour tous vos comptes ChatGPT connectés.';

  @override
  String get usageQuotasLoadFailed =>
      'Impossible de charger les quotas d’utilisation.';

  @override
  String get statistics => 'Statistiques';

  @override
  String get statisticsFilters => 'Filtres';

  @override
  String get statisticsDescription =>
      'Consultez l’utilisation des jetons au fil du temps par fournisseur, modèle et conversation.';

  @override
  String get statisticsLoadFailed =>
      'Les statistiques d\'utilisation n\'ont pas pu être chargées.';

  @override
  String get statisticsUnavailable =>
      'Les statistiques d\'utilisation locales sont indisponibles pour le moment.';

  @override
  String get statisticsRetry => 'Recharger';

  @override
  String get statisticsDateRange => 'Période';

  @override
  String get statisticsProvider => 'Fournisseur';

  @override
  String get statisticsRunId => 'ID d’exécution';

  @override
  String statisticsRunIdValue(String runId) {
    return 'ID d’exécution : $runId';
  }

  @override
  String get statisticsModel => 'Modèle';

  @override
  String get statisticsOperation => 'Type d\'opération';

  @override
  String get statisticsReasoningEffort => 'Niveau de raisonnement';

  @override
  String get statisticsFastModeFilter => 'Mode rapide';

  @override
  String get statisticsAll => 'Tout';

  @override
  String get statisticsUnspecified => 'Non défini';

  @override
  String get statisticsClearFilters => 'Effacer les filtres';

  @override
  String get statisticsTotalTokens => 'Total des jetons';

  @override
  String get statisticsInputTokens => 'Jetons d\'entrée';

  @override
  String get statisticsOutputTokens => 'Jetons de sortie';

  @override
  String get statisticsReasoningTokens => 'Jetons de raisonnement';

  @override
  String get statisticsCachedInputTokens => 'Jetons d\'entrée en cache';

  @override
  String get statisticsCacheWriteTokens => 'Jetons écrits dans le cache';

  @override
  String get statisticsRequests => 'Requêtes';

  @override
  String get statisticsSuccessfulRequests => 'Réussies';

  @override
  String get statisticsFailedRequests => 'Échouées';

  @override
  String get statisticsCancelledRequests => 'Arrêtées';

  @override
  String get statisticsInterruptedRequests => 'Interrompues';

  @override
  String get statisticsPendingRequests => 'En cours';

  @override
  String get statisticsConversations => 'Conversations';

  @override
  String get statisticsCoverage => 'Couverture des données d\'utilisation';

  @override
  String get statisticsInputCoverage =>
      'Requêtes avec jetons d\'entrée déclarés';

  @override
  String get statisticsOutputCoverage =>
      'Requêtes avec jetons de sortie déclarés';

  @override
  String get statisticsReasoningCoverage =>
      'Requêtes avec jetons de raisonnement déclarés';

  @override
  String statisticsCoverageText(int total, int reported) {
    return 'Le fournisseur a renvoyé l\'utilisation des jetons pour $reported requêtes sur $total.';
  }

  @override
  String get statisticsProviderReportedCost =>
      'Coût indiqué par le fournisseur';

  @override
  String statisticsCostCoverage(int reported) {
    return 'Le coût est disponible pour $reported requêtes.';
  }

  @override
  String get statisticsModelsDevCatalogCost =>
      'Coût selon les tarifs catalogue de models.dev';

  @override
  String statisticsModelsDevCatalogCostCoverage(int priced) {
    return 'Équivalent au tarif catalogue calculé pour $priced requêtes.';
  }

  @override
  String statisticsModelsDevPricingCurrent(String date) {
    return 'Catalogue tarifaire models.dev récupéré le $date.';
  }

  @override
  String statisticsModelsDevPricingStale(String date) {
    return 'models.dev est inaccessible ; les tarifs en cache du $date sont utilisés.';
  }

  @override
  String get statisticsModelsDevPricingUnavailable =>
      'Les tarifs models.dev sont indisponibles. Aucun coût n\'est calculé sans correspondance exacte du fournisseur et du modèle.';

  @override
  String get statisticsUsageTrend => 'Utilisation au fil du temps';

  @override
  String get statisticsDaily => 'Par jour';

  @override
  String get statisticsMonthly => 'Par mois';

  @override
  String get statisticsProviders => 'Utilisation par fournisseur';

  @override
  String get statisticsModels => 'Utilisation par modèle';

  @override
  String get statisticsReasoningLevels => 'Niveau de raisonnement choisi';

  @override
  String get statisticsOperations => 'Types d\'opération';

  @override
  String get statisticsFastModeUsage => 'Utilisation du mode rapide';

  @override
  String get statisticsRequested => 'Demandé';

  @override
  String get statisticsNotRequested => 'Non demandé';

  @override
  String get statisticsServiceTiers => 'Niveaux de service renvoyés';

  @override
  String get statisticsServiceTier => 'Niveau de service';

  @override
  String get statisticsNoBreakdownData =>
      'Aucune donnée à afficher pour cette période.';

  @override
  String get statisticsNoConversationData =>
      'Aucune utilisation par conversation pour cette période.';

  @override
  String get statisticsOpenConversation => 'Ouvrir la conversation';

  @override
  String get statisticsRequestDetails => 'Historique des requêtes';

  @override
  String get statisticsRequestPayload => 'Données de la requête envoyée';

  @override
  String get statisticsRequestContextUnavailable =>
      'Le résumé de la requête envoyée est indisponible.';

  @override
  String statisticsRequestSourceMessages(String ids) {
    return 'Identifiants des messages inclus : $ids';
  }

  @override
  String statisticsRequestArchivedMessages(String ids) {
    return 'Identifiants des messages récupérés : $ids';
  }

  @override
  String statisticsRequestSummaryBoundary(String id) {
    return 'Résumé couvrant les messages jusqu’à : $id';
  }

  @override
  String statisticsRequestSourceAttachments(String files) {
    return 'Pièces jointes envoyées : $files';
  }

  @override
  String get statisticsRequestSourcesTruncated =>
      'Certains détails des sources sont omis de cet enregistrement.';

  @override
  String statisticsRequestMessageCount(int count) {
    return 'Messages envoyés : $count';
  }

  @override
  String statisticsRequestImageCount(int count) {
    return 'Images envoyées : $count';
  }

  @override
  String statisticsRequestToolResultCount(int count) {
    return 'Résultats d’outils envoyés : $count';
  }

  @override
  String statisticsRequestInstructionBytes(int count) {
    return 'Taille des instructions : $count octets';
  }

  @override
  String statisticsRequestRoles(String roles) {
    return 'Rôles des messages : $roles';
  }

  @override
  String statisticsRequestTools(String names) {
    return 'Définitions d’outils : $names';
  }

  @override
  String statisticsRequestCacheControls(String names) {
    return 'Contrôles du cache : $names';
  }

  @override
  String get statisticsNone => 'Aucun';

  @override
  String get statisticsRequestTime => 'Heure de la requête';

  @override
  String get statisticsConversationTitle => 'Titre de la conversation';

  @override
  String get statisticsStatus => 'Statut';

  @override
  String get statisticsUsageSource => 'Source des données d\'utilisation';

  @override
  String get statisticsNoRequestData =>
      'Aucune requête ne correspond à ces filtres.';

  @override
  String statisticsShowingRows(int start, int end, int total) {
    return '$start - $end sur $total';
  }

  @override
  String get statisticsExportCsv => 'Exporter en CSV';

  @override
  String get statisticsExporting => 'Export en cours';

  @override
  String get statisticsExported =>
      'Les statistiques ont été exportées dans un fichier CSV.';

  @override
  String get statisticsExportFailed =>
      'Les statistiques n\'ont pas pu être exportées.';

  @override
  String get statisticsQuotaHistory => 'Historique des quotas ChatGPT';

  @override
  String get statisticsQuotaSnapshot => 'Relevé de quota';

  @override
  String get statisticsNoQuotaHistory =>
      'Aucun relevé de quota ChatGPT n\'a été enregistré sur cette période.';

  @override
  String get statisticsUsed => 'Utilisé';

  @override
  String get statisticsResetAt => 'Réinitialisation';

  @override
  String get statisticsQuotaAllowed => 'Requêtes autorisées';

  @override
  String get statisticsQuotaBlocked => 'Requêtes bloquées';

  @override
  String get statisticsQuotaUnknown => 'État d\'utilisation inconnu';

  @override
  String get statisticsQuotaFreshnessCurrent => 'Données actuelles';

  @override
  String get statisticsQuotaFreshnessStale => 'Données obsolètes';

  @override
  String get statisticsQuotaFreshnessUnknown => 'État des données inconnu';

  @override
  String get statisticsNotReported => 'Non indiqué';

  @override
  String get statisticsLegacyDataNote =>
      'Les anciens messages peuvent ne contenir que les jetons de sortie ; les données manquantes du modèle et des jetons d\'entrée ne sont pas déduites.';

  @override
  String get statisticsModelsDevPricingNote =>
      'Les équivalents de prix catalogue utilisent les tarifs actuels de models.dev et ne sont pas des factures fournisseur. Les abonnements ChatGPT OAuth, les requêtes Fast et les niveaux de service non pris en charge sont exclus.';

  @override
  String get statisticsLegacyOutput => 'Jetons de sortie d\'anciens messages';

  @override
  String get statisticsChatGptOAuth => 'ChatGPT OAuth';

  @override
  String get statisticsChatGptApi => 'ChatGPT API';

  @override
  String get statisticsOperationChat => 'Conversation';

  @override
  String get statisticsOperationToolFollowUp => 'Suite d\'outil';

  @override
  String get statisticsOperationCompaction => 'Compression du contexte';

  @override
  String get statisticsOperationTitleGeneration =>
      'Génération du titre de conversation';

  @override
  String get statisticsOperationLegacy => 'Ancien message';

  @override
  String get statisticsStatusCompleted => 'Terminée';

  @override
  String get statisticsStatusFailed => 'Échouée';

  @override
  String get statisticsStatusCancelled => 'Arrêtée';

  @override
  String get statisticsStatusInterrupted => 'Interrompue';

  @override
  String get statisticsStatusPending => 'En cours';

  @override
  String get statisticsStatusLegacy => 'Données historiques';

  @override
  String get refreshAll => 'Tout actualiser';

  @override
  String get noChatGptAccountsForQuota =>
      'Aucun compte ChatGPT connecté trouvé.';

  @override
  String get noChatGptAccountsForQuotaDescription =>
      'Connectez votre compte ChatGPT dans l\'onglet Connexions pour afficher vos limites d\'utilisation et vos quotas.';

  @override
  String get goToConnections => 'Accéder aux connexions';

  @override
  String get activeAccountBadge => 'Actif';

  @override
  String workspaceQuotaLabel(String name) {
    return 'Espace de travail ChatGPT : $name';
  }

  @override
  String get modelsPageDescription =>
      'Trouvez des modèles à exécuter sur cet appareil. Les modèles téléchargés figurent dans Modèles locaux.';

  @override
  String get modelSortDownloads => 'Les plus téléchargés';

  @override
  String get modelSortLikes => 'Les plus appréciés';

  @override
  String get modelSortRecentlyUpdated => 'Récemment mis à jour';

  @override
  String get modelPreviousPage => 'Précédent';

  @override
  String get modelNextPage => 'Suivant';

  @override
  String modelPageLabel(int page) {
    return 'Page $page';
  }

  @override
  String get modelFormatGguf => 'GGUF · llama.cpp';

  @override
  String get huggingFaceModelSearchHint => 'Rechercher des modèles';

  @override
  String get otherProvider => 'Autre fournisseur';

  @override
  String get modelSearchRefresh => 'Actualiser les résultats du modèle';

  @override
  String get modelSearchEmpty =>
      'Aucun modèle ne correspond à cette recherche.';

  @override
  String get modelSearchFailed =>
      'Les modèles Hugging Face n\'ont pas pu être chargés.';

  @override
  String get modelSearchUnavailable =>
      'Hugging Face n\'a pas pu être atteint. Vérifiez votre connexion et réessayez.';

  @override
  String get modelSearchRateLimited =>
      'Hugging Face reçoit trop de demandes. Attendez un moment et réessayez.';

  @override
  String get modelSearchInvalidResponse =>
      'Hugging Face a renvoyé des données de modèle qu’OpenChat n’a pas pu lire. Réessayez bientôt.';

  @override
  String get modelSearchTimedOut =>
      'Hugging Face a mis trop longtemps à répondre. Réessayez.';

  @override
  String get modelChooseForDetails =>
      'Choisissez un modèle pour inspecter ses fichiers.';

  @override
  String get modelDownloadsLabel => 'Téléchargements';

  @override
  String get modelLikesLabel => 'Mentions J’aime';

  @override
  String get modelLicenseLabel => 'Licence';

  @override
  String get modelRevisionLabel => 'Révision';

  @override
  String get modelFilesLabel => 'Fichiers de modèle';

  @override
  String get modelVisionComponentsLabel => 'Composants de vision';

  @override
  String get modelMtpComponentsLabel => 'Composants MTP';

  @override
  String get modelAuxiliaryComponentsLabel => 'Autres composants auxiliaires';

  @override
  String get modelDownloadComponentButton => 'Télécharger ce composant';

  @override
  String get modelComponentDownloaded => 'Composant téléchargé';

  @override
  String get modelShowMoreComponents => 'Afficher plus de composants';

  @override
  String get modelReadmeLabel => 'Description du modèle';

  @override
  String get modelReadmeMissing => 'Ce modèle n\'a pas de README.';

  @override
  String get modelReadmeAccessDenied =>
      'L\'accès à ce référentiel est requis pour visualiser sa description.';

  @override
  String get modelReadmeTooLarge =>
      'Le README est trop grand pour être affiché.';

  @override
  String get modelReadmeUnavailable =>
      'La description du modèle n\'a pas pu être chargée.';

  @override
  String get modelDownloadOptionsLabel => 'Options de téléchargement';

  @override
  String get modelDownloadGroupLabel => 'Ensemble de fichiers';

  @override
  String get modelDownloadSizeLabel => 'Taille';

  @override
  String get modelDownloadButton => 'Télécharger le modèle';

  @override
  String get modelCancelDownload => 'Annuler le téléchargement';

  @override
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount) {
    return 'Fichier $fileIndex de $fileCount : $fileName';
  }

  @override
  String get modelDownloadComplete =>
      'Modèle téléchargé et ajouté aux modèles locaux.';

  @override
  String get modelDownloadCancelled =>
      'Téléchargement du modèle annulé. Vous pourrez le reprendre plus tard.';

  @override
  String get modelDownloadFailed => 'Le modèle n\'a pas pu être téléchargé.';

  @override
  String get modelDownloadProgressUnavailable =>
      'La progression du téléchargement n\'a pas pu être lue.';

  @override
  String get modelRevisionChanged =>
      'Ce modèle a changé sur Hugging Face. Rechargez ses fichiers et réessayez.';

  @override
  String get modelDownloadAccessNeeded =>
      'Ce dépôt est restreint ou privé et nécessite un accès Hugging Face.';

  @override
  String get modelNoCompatibleFiles =>
      'Aucun fichier de modèle compatible complet n\'a été trouvé dans ce référentiel.';

  @override
  String get modelUnknownDownloadSize =>
      'La taille du fichier n\'est pas disponible, ce téléchargement ne peut donc pas démarrer en toute sécurité.';

  @override
  String get modelDetailsLoading => 'Chargement des fichiers de modèle…';

  @override
  String get modelNoFiles =>
      'Aucun fichier compatible n\'est disponible pour ce format.';

  @override
  String get modelGatedBadge => 'Accès requis';

  @override
  String get modelPrivateBadge => 'Privé';

  @override
  String get modelSavedToFolder =>
      'Les téléchargements sont enregistrés dans le dossier choisi pour ce moteur dans les paramètres.';

  @override
  String get userQuestionTitle => 'L’assistant attend votre réponse';

  @override
  String get userQuestionRequiredHint =>
      'Les questions obligatoires sont signalées';

  @override
  String get userQuestionSubmit => 'Envoyer la réponse';

  @override
  String get userQuestionResuming => 'Reprise de l’assistant';

  @override
  String get userQuestionUnavailable =>
      'Cette question n’est plus disponible. Rechargez la conversation.';

  @override
  String get userQuestionRequiredValidation =>
      'Répondez à toutes les questions obligatoires pour continuer.';

  @override
  String get userQuestionSubmitFailed =>
      'Votre réponse n’a pas pu être enregistrée. Réessayez.';

  @override
  String get userQuestionRequiredLabel => 'Obligatoire';

  @override
  String get userQuestionContinue => 'Reprendre l’assistant';

  @override
  String get userQuestionSaved =>
      'Votre réponse est enregistrée. Reprenez quand vous le souhaitez.';

  @override
  String get userQuestionLoadFailed =>
      'La question en attente n’a pas pu être chargée. Réessayez.';

  @override
  String get userQuestionResumeFailed =>
      'La réponse est enregistrée, mais l’assistant n’a pas pu reprendre. Réessayez.';

  @override
  String get userQuestionNotificationTitle => 'OpenChat vous attend';

  @override
  String get userQuestionNotificationBody => 'L’IA attend votre réponse.';

  @override
  String get notificationResponseReady => 'Une réponse est prête.';

  @override
  String get assistantResponseNotificationReplyEmpty =>
      'Saisissez une réponse avant de l’envoyer.';

  @override
  String get assistantResponseNotificationReplyTooLong =>
      'Cette réponse est trop longue pour être envoyée depuis une notification. Utilisez le champ de saisie de la conversation.';

  @override
  String get assistantResponseNotificationReplyUnavailable =>
      'Cette notification n’est plus à jour ou la conversation est indisponible. Ouvrez la conversation et envoyez un nouveau message.';

  @override
  String get assistantResponseNotificationReplyNotSent =>
      'La réponse rapide n’a pas pu être envoyée. Vérifiez la conversation et réessayez.';

  @override
  String fileChangesSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# fichiers modifiés',
      one: '# fichier modifié',
    );
    return '$_temp0';
  }

  @override
  String fileChangesLineCounts(int added, int removed) {
    return '+$added / -$removed';
  }

  @override
  String get fileChangesSomeCountsUnavailable =>
      'Nombre de lignes indisponible';

  @override
  String get fileChangesView => 'Voir les modifications';

  @override
  String get fileChangesTrackingFailed =>
      'Certaines modifications n’ont pas pu être suivies. La liste est peut-être incomplète.';

  @override
  String get fileChangesTitle => 'Modifications de cette conversation';

  @override
  String get fileChangesOpenButton => 'Modifications';

  @override
  String get fileChangesLoadFailed =>
      'Impossible de charger les modifications de la conversation.';

  @override
  String get fileChangesDiffFailed =>
      'Impossible de charger le diff du fichier.';

  @override
  String get fileChangesConflict =>
      'Ce fichier a changé après la modification de l’IA. Il n’a pas été touché.';

  @override
  String get fileChangesRevertFailed => 'Impossible d’annuler la modification.';

  @override
  String get fileChangesEmpty =>
      'Aucune modification de fichier n’a été enregistrée pour cette conversation.';

  @override
  String get fileChangesDiffTitle =>
      'Sélectionnez un fichier pour voir son diff';

  @override
  String get fileChangesBinary =>
      'Les modifications des fichiers binaires ne peuvent pas être affichées en texte.';

  @override
  String get fileChangesDiffUnavailable =>
      'Aucun diff texte n’est disponible pour ce fichier.';

  @override
  String get fileChangesDiffTruncated =>
      'Le diff est long. Seul le début est affiché.';

  @override
  String get fileChangesActive => 'Modifié';

  @override
  String get fileChangesReverted => 'Annulé';

  @override
  String get fileChangesRevert => 'Annuler';

  @override
  String fileChangesMoreFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# fichiers supplémentaires',
      one: '# fichier supplémentaire',
    );
    return '$_temp0';
  }

  @override
  String get fileChangesUnavailableTitle => 'Modifications indisponibles';

  @override
  String get slashCommandPlanLabel => 'Plan';

  @override
  String get slashCommandGoalLabel => 'Objectif';

  @override
  String get slashCommandNewLabel => 'Nouveau';

  @override
  String get slashCommandSearchLabel => 'Rechercher';

  @override
  String get slashCommandSummarizeLabel => 'Résumer';

  @override
  String get slashCommandSummarizeDescription =>
      'Crée un résumé concis de cette conversation avec les décisions, les résultats et les points en suspens.';

  @override
  String get slashCommandSummarizePrompt =>
      'Résumez cette conversation. Présentez brièvement les décisions, le travail terminé et les points en suspens.';

  @override
  String get slashCommandSummarizeNoMessages =>
      'Aucun message à résumer. Envoyez d’abord au moins un message dans cette discussion.';

  @override
  String get slashCommandSummarizeLoading =>
      'Les messages sont encore en cours de chargement ; la conversation ne peut pas encore être résumée.';

  @override
  String get slashCommandSummarizeUnavailable =>
      'Cette commande ne peut pas être exécutée pour le moment. Vérifiez la connexion du modèle et attendez la fin des opérations en cours.';

  @override
  String get slashCommandRenameLabel => 'Renommer';

  @override
  String get slashCommandRenameDescription =>
      'Renommer cette conversation. Saisissez un nouveau titre après la commande.';

  @override
  String get slashCommandRenameTitleRequired =>
      'Saisissez un titre après /rename.';

  @override
  String get slashCommandRenameNoConversation =>
      'Ouvrez une conversation avant de la renommer.';

  @override
  String get slashCommandRenameSuccess => 'Conversation renommée.';

  @override
  String get slashCommandModelLabel => 'Modèle';

  @override
  String get slashCommandModelDescription =>
      'Passer à un modèle disponible en indiquant son nom ou son ID exact.';

  @override
  String get slashCommandModelNameRequired =>
      'Saisissez le nom ou l’ID du modèle après /model.';

  @override
  String get slashCommandModelNoMatch =>
      'Aucun modèle disponible chez le fournisseur actuel ne correspond à ce nom ou à cet ID.';

  @override
  String get slashCommandModelAmbiguous =>
      'Plusieurs modèles disponibles correspondent. Utilisez l’ID exact du modèle.';

  @override
  String get slashCommandModelUnavailable =>
      'La sélection du modèle est indisponible pendant le chargement des modèles ou la génération d’une réponse.';

  @override
  String get slashCommandCompactLabel => 'Compacter le contexte';

  @override
  String get slashCommandCompactDescription =>
      'Résumer localement les anciens messages pour les prochaines requêtes tout en gardant l’historique complet consultable.';

  @override
  String get slashCommandCompactNoConversation =>
      'Ouvrez une conversation avant d’en compacter le contexte.';

  @override
  String get slashCommandCompactNoArguments =>
      '/compact ne prend aucun argument.';

  @override
  String get slashCommandCompactNoContextWindow =>
      'Aucune limite de contexte n’est disponible pour ce modèle. Définissez-en une dans les préférences des modèles.';

  @override
  String get slashCommandCompactBusy =>
      'Attendez la fin de la réponse en cours avant de compacter le contexte.';

  @override
  String get slashCommandCompactSuccess =>
      'L’ancien contexte de la conversation a été compacté. L’historique complet reste consultable.';

  @override
  String get slashCommandCompactNothing =>
      'Il n’y a pas encore assez d’ancien contexte à compacter.';

  @override
  String get slashCommandCompactFailed =>
      'Impossible de compacter l’ancien contexte. L’historique complet reste disponible.';

  @override
  String get slashCommandReasoningLabel => 'Raisonnement';

  @override
  String get slashCommandReasoningDescription =>
      'Définir un niveau de raisonnement pris en charge par le modèle sélectionné ou choisir la valeur par défaut.';

  @override
  String get slashCommandReasoningLevelRequired =>
      'Saisissez un niveau pris en charge après /reasoning ou utilisez /reasoning default.';

  @override
  String get slashCommandReasoningUnsupported =>
      'Le modèle sélectionné ne prend pas en charge ce niveau de raisonnement.';

  @override
  String get goalSlashCommand => '/goal';

  @override
  String get goalCommandDescription =>
      'Lancer ce message comme objectif et continuer jusqu’à sa réalisation ou jusqu’à avoir besoin de vous.';

  @override
  String get goalObjectiveRequired => 'Saisissez un objectif avant l’envoi.';

  @override
  String get goalWorking => 'Objectif en cours';

  @override
  String get goalPaused => 'Objectif en pause';

  @override
  String get goalInterrupted => 'Objectif interrompu';

  @override
  String get goalCompleted => 'Objectif terminé';

  @override
  String get goalStopped => 'Objectif arrêté';

  @override
  String get goalFailed => 'Échec de l’objectif';

  @override
  String get goalPausedForQuota =>
      'En pause car le quota ou la limite du modèle est atteint.';

  @override
  String get goalPausedForBlocker => 'En pause à cause d’un blocage.';

  @override
  String get goalPausedForUserInput =>
      'En pause dans l’attente de votre réponse.';

  @override
  String get goalPausedByUser => 'Mis en pause par vous.';

  @override
  String get goalPausedAfterError => 'En pause après une erreur de requête.';

  @override
  String get goalPauseAction => 'Mettre l’objectif en pause';

  @override
  String get goalResumeAction => 'Reprendre l’objectif';

  @override
  String get goalStopAction => 'Arrêter l’objectif';

  @override
  String goalElapsedTime(String time) {
    return 'Durée écoulée : $time';
  }

  @override
  String goalTasksCount(int completed, int total) {
    return 'Tâches : $completed sur $total';
  }

  @override
  String get supportDiagnosticsTitle => 'Diagnostic d’assistance';

  @override
  String get supportDiagnosticsDescription =>
      'Consultez l’état de l’application et du stockage avant d’enregistrer le fichier d’assistance.';

  @override
  String get reviewSupportDiagnostics => 'Vérifier l’export';

  @override
  String get supportDiagnosticsPreviewTitle =>
      'Vérifier le diagnostic d’assistance';

  @override
  String get supportDiagnosticsPreviewDescription =>
      'Le fichier contient les informations ci-dessous. Le contenu des conversations, les identifiants et les chemins de fichiers sont exclus.';

  @override
  String get supportDiagnosticsExportAction => 'Enregistrer le JSON';

  @override
  String get supportDiagnosticsSaved =>
      'Le diagnostic d’assistance a été enregistré.';

  @override
  String get supportDiagnosticsLoadFailed =>
      'Impossible de charger le diagnostic. Vérifiez le service local et réessayez.';

  @override
  String get supportDiagnosticsInvalidResponse =>
      'Le service local a renvoyé des données de diagnostic inattendues.';

  @override
  String get supportDiagnosticsExportFailed =>
      'Impossible d’enregistrer le fichier de diagnostic d’assistance.';

  @override
  String get supportDiagnosticsPrivacyNote =>
      'Seuls les champs affichés dans l’aperçu sont enregistrés. Le fichier n’est pas envoyé à OpenChat.';

  @override
  String get supportDiagnosticsPreparing => 'Préparation…';

  @override
  String get settingsConnectors => 'Connecteurs';

  @override
  String get settingsConnectorsDescription =>
      'Gérez les modèles locaux et ajoutez-les aux projets.';

  @override
  String get connectorCatalogIntro =>
      'Enregistrez dans cette application les paramètres de connexion MCP réutilisables. Un serveur ajouté à un projet reste désactivé ; ses identifiants et autorisations restent propres à ce projet.';

  @override
  String get connectorCatalogEmpty =>
      'Aucun modèle de connexion n’a été ajouté.';

  @override
  String get connectorCatalogAdd => 'Ajouter un modèle';

  @override
  String get connectorCatalogEdit => 'Modifier le modèle';

  @override
  String get connectorCatalogDelete => 'Supprimer le modèle';

  @override
  String get connectorCatalogDeleteConfirm =>
      'Le modèle sera supprimé de l’application. Les connecteurs déjà ajoutés aux projets y resteront.';

  @override
  String get connectorCatalogInstall => 'Ajouter au projet actif';

  @override
  String get connectorCatalogNoProject =>
      'Ouvrez un projet pour y ajouter un connecteur.';

  @override
  String get connectorCatalogSecurityNotice =>
      'Les modèles sont conservés localement sans valeurs secrètes. L’ajout crée un serveur de projet désactivé et ses outils nécessitent une approbation. OpenChat ne télécharge ni ne vérifie le logiciel du serveur.';

  @override
  String get connectorCatalogName => 'Nom du modèle';

  @override
  String get connectorCatalogId => 'ID du serveur';

  @override
  String get connectorCatalogPublisher => 'Éditeur déclaré';

  @override
  String get connectorCatalogSourceUrl => 'URL source';

  @override
  String get connectorCatalogDataDisclosure =>
      'Données que ce connecteur peut recevoir';

  @override
  String get connectorCatalogTransport => 'Type de connexion';

  @override
  String get connectorCatalogProgram => 'Chemin absolu du programme';

  @override
  String get connectorCatalogArguments => 'Arguments, un par ligne';

  @override
  String get connectorCatalogEnvironmentVariables =>
      'Noms des variables d’identifiants, un par ligne';

  @override
  String get connectorCatalogEndpoint => 'Point de terminaison HTTPS';

  @override
  String get connectorCatalogAuthVariable => 'Nom de variable d’identifiant';

  @override
  String get connectorCatalogSave => 'Enregistrer le modèle';

  @override
  String get connectorCatalogInstallPrompt =>
      'Vérifiez l’éditeur, la destination et les données indiquées. Le serveur sera ajouté désactivé et ses outils demanderont une approbation. Les appels peuvent envoyer leurs arguments au connecteur et leurs résultats au fournisseur de modèle sélectionné.';

  @override
  String get connectorCatalogInvalid =>
      'Vérifiez les champs obligatoires, les URL HTTPS, l’ID et les paramètres de connexion.';

  @override
  String get connectorCatalogDuplicate =>
      'Cet ID ou ce nom est déjà utilisé par un modèle ou un serveur du projet.';

  @override
  String get connectorCatalogLoadFailed =>
      'Impossible de lire le catalogue local des connecteurs.';

  @override
  String get connectorCatalogSaveFailed =>
      'Impossible d’enregistrer le modèle de connexion.';

  @override
  String get connectorCatalogInstallFailed =>
      'Impossible d’ajouter le connecteur au projet. Vérifiez le service local et les paramètres du projet.';

  @override
  String get connectorCatalogInstalled =>
      'Le connecteur désactivé a été ajouté au projet. Ajoutez ses identifiants dans les paramètres MCP du projet si nécessaire.';

  @override
  String get connectorCatalogSearch => 'Rechercher des modèles';

  @override
  String get connectorCatalogNoResults =>
      'Aucun modèle ne correspond à cette recherche.';

  @override
  String get connectorCatalogVersion => 'Version déclarée';

  @override
  String get connectorCatalogRequestedAccess => 'Accès demandé';

  @override
  String get connectorCatalogAuthentication => 'Authentification';

  @override
  String get connectorCatalogNoBearerCredential =>
      'Aucun identifiant Bearer n’est déclaré. Le serveur peut tout de même appliquer sa propre politique d’accès.';

  @override
  String get connectorCatalogProjectBearerCredential =>
      'S’il est configuré, l’identifiant Bearer est limité à ce projet et à ce serveur, puis envoyé à ce point de terminaison HTTPS lors de la connexion.';

  @override
  String get connectorCatalogNoCredentialVariables =>
      'Ce processus local ne demande aucune variable d’environnement.';

  @override
  String get connectorCatalogProjectEnvironmentCredentials =>
      'Lorsqu’elles sont configurées, les valeurs d’environnement déclarées sont limitées à ce projet et à ce serveur, puis transmises uniquement à son processus local.';

  @override
  String get connectorCatalogRemovalImpact =>
      'Supprimer ce modèle ne supprime ni les serveurs des projets ni leurs identifiants enregistrés. Supprimez-les séparément dans les paramètres MCP du projet.';

  @override
  String get libraryTabModels => 'Modèles';

  @override
  String get libraryTabMcp => 'Outils MCP';

  @override
  String get libraryTabPrompts => 'Instructions (Bientôt)';

  @override
  String get libraryTabPersonalities => 'Personnalités (Bientôt)';

  @override
  String get mcpCatalogDescription =>
      'Découvrez, installez et configurez des serveurs Model Context Protocol.';

  @override
  String get mcpSearchHint => 'Rechercher des outils MCP...';

  @override
  String get mcpInstalled => 'Installé';

  @override
  String get mcpNotInstalled => 'Non installé';

  @override
  String get mcpActive => 'Actif (Ajouté aux instructions)';

  @override
  String get mcpInactive => 'Inactif';

  @override
  String get mcpInstall => 'Installer';

  @override
  String get mcpConfigureEnv => 'Variables d\'environnement (Env)';

  @override
  String get mcpEnvDialogTitle => 'Variables d\'environnement MCP';

  @override
  String get mcpEnvDialogDescription =>
      'Ces valeurs sont stockées en toute sécurité via le Gestionnaire d\'identifiants Windows.';

  @override
  String get mcpEnvSaveSuccess =>
      'Variables d\'environnement enregistrées en toute sécurité.';

  @override
  String get mcpEnabledSuccess =>
      'Outil MCP activé et ajouté aux instructions.';

  @override
  String get mcpDisabledSuccess =>
      'Outil MCP désactivé et retiré des instructions.';

  @override
  String get mcpNoToolsFound => 'Aucun outil trouvé pour ce MCP.';

  @override
  String get mcpUninstall => 'Désinstaller';

  @override
  String get mcpUninstallConfirm =>
      'Voulez-vous vraiment désinstaller cet outil MCP ?';

  @override
  String get mcpDiscoverTools => 'Récupérer les définitions';

  @override
  String get mcpDiscovering => 'Récupération des définitions...';

  @override
  String get mcpDiscoverSuccess =>
      'Définitions des outils récupérées avec succès.';

  @override
  String get mcpDiscoverFailed =>
      'Impossible de récupérer les définitions des outils. Vérifiez la commande et les variables d\'environnement.';

  @override
  String get mcpNoToolsDiscovered =>
      'Aucune définition d\'outil récupérée pour le moment.';
}
