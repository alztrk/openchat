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
  String get chats => 'Discussions';

  @override
  String get collapseSidebars => 'Réduire les barres latérales';

  @override
  String get showSidebars => 'Afficher les barres latérales';

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
  String get connections => 'Connexions';

  @override
  String get models => 'Modèles';

  @override
  String get modelsDescription =>
      'Gérez les modèles des fournisseurs connectés, définissez un modèle par défaut et masquez les modèles dont vous n\'avez pas besoin.';

  @override
  String get localEngines => 'Moteurs locaux';

  @override
  String get localEnginesDescription =>
      'Installez des environnements d\'exécution locaux vérifiés, enregistrez vos propres fichiers de modèle et vérifiez si un environnement d\'exécution est sain. Déplacez ou copiez des modèles dans un dossier de moteur, ou conservez-les là où ils se trouvent.';

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
  String get localEngineNotInstalled => 'Non installé';

  @override
  String get localEngineBlocked => 'Bloqué';

  @override
  String get localEngineDeprecated => 'Obsolète';

  @override
  String get localEngineWindowsDeprecatedReason =>
      'Les nouvelles installations de vLLM et ExLlama sont désactivées sous Windows. Les fichiers et modèles déjà enregistrés sont conservés.';

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
  String get localEngineVllmBlockedReason =>
      'vLLM nécessite un pilote NVIDIA version 580 ou ultérieure et un environnement Linux existant. Sous Windows, il utilise une distribution WSL2 existante avec accès au GPU.';

  @override
  String get localEngineExllamaBlockedReason =>
      'Le runtime ExLlamaV3 n\'est pas encore prêt pour l\'installation. OpenChat doit épingler et vérifier l\'ensemble complet de dépendances TabbyAPI, PyTorch, Triton, Flash Linear Attention et Python avant de le proposer.';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Configuration requise : $requirements';
  }

  @override
  String get localEngineInstall => 'Installer';

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
  String get localEngineStageRuntimeSetup =>
      'Préparation de l’environnement Python';

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
  String get sharedInstructionsDescription =>
      'Ces instructions sont envoyées avec chaque fournisseur connecté. L\'accès aux outils de fichiers suit le paramètre d\'accès aux outils.';

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
  String get geminiApiDescription =>
      'Ajoutez une clé API Google AI Studio pour utiliser les modèles disponibles pour votre compte. L’accès gratuit dépend du modèle et de votre quota.';

  @override
  String get groqApiDescription =>
      'Utilisez les modèles activés pour votre compte avec une clé API Groq. La tarification et les limites d’utilisation varient selon le forfait et le modèle.';

  @override
  String get cerebrasApiDescription =>
      'Utilisez les modèles compatibles avec les outils disponibles pour votre compte avec une clé API Cerebras. L’accès, la tarification et les limites varient selon le modèle et le compte.';

  @override
  String get openRouterApiDescription =>
      'Seuls les modèles affichant un prix de 0 \$ pour l’entrée et la sortie, avec prise en charge du texte et des outils, apparaissent ici. L’accès réel et les limites peuvent changer.';

  @override
  String get mistralApiDescription =>
      'Ajoutez une clé API Mistral pour utiliser les modèles de chat disponibles pour votre compte. L’accès gratuit, la tarification et les limites d’utilisation dépendent de votre forfait Mistral.';

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
  String get favoriteModels => 'Favoris';

  @override
  String get favoriteModelsEmpty => 'Pas encore de modèles favoris.';

  @override
  String get modelSearchHint => 'Rechercher des modèles...';

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
      'Les modèles gratuits fonctionnent sans clé. Ajoutez une clé Console API pour les modèles payants ; chaque demande est facturée sur votre solde Console.';

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
  String get providerRequestFailed =>
      'Le fournisseur n’a pas pu terminer la réponse. Vos messages enregistrés sont toujours disponibles.';

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
  String get workspaceWithoutName => 'Espace de travail';

  @override
  String get workspace => 'Espace de travail';

  @override
  String get workspaceUnavailable =>
      'Aucune information sur l\'espace de travail n\'est disponible.';

  @override
  String get selectAccountForWorkspace =>
      'Sélectionnez ce compte pour choisir son espace de travail.';

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
    return 'Espace de travail $number';
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
      'Lorsque le compte de la conversation ne dispose d’aucun autre modèle, OpenChat peut utiliser le compte choisi ici. La génération de titres est ignorée lorsque l’utilisation normale n’est pas disponible.';

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
  String get appFont => 'Police de l\'application';

  @override
  String get appFontDescription =>
      'Choisissez la police utilisée dans OpenChat.';

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
  String get searchChats => 'Rechercher des discussions';

  @override
  String get searchChatsHint =>
      'Rechercher dans les discussions et les messages';

  @override
  String get searchMessagesTooltip => 'Rechercher des messages';

  @override
  String get historySearchDateFilter => 'Filtrer par date';

  @override
  String get historySearchDateFilterApplied => 'Le filtre de date est actif';

  @override
  String get historySearchFiltersTitle => 'Filtres de recherche';

  @override
  String get historySearchRouteFilterNote =>
      'Le fournisseur et le modèle correspondent à la route enregistrée avec chaque réponse.';

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
  String get searchMessagesLoading => 'Recherche des messages…';

  @override
  String get searchMessagesNoResults => 'Aucun message correspondant';

  @override
  String get searchMessagesTooShort =>
      'Saisissez au moins deux caractères pour rechercher des messages.';

  @override
  String get searchMessagesTooLong =>
      'Le texte de recherche ne peut pas dépasser 512 caractères.';

  @override
  String get searchMessagesFailed =>
      'La recherche des messages a échoué. Réessayez.';

  @override
  String get searchMessageUnavailable => 'Ce message n’est plus disponible.';

  @override
  String get projects => 'Projets';

  @override
  String get noProjects => 'Aucun projet pour l\'instant';

  @override
  String get createProject => 'Créer un projet';

  @override
  String get projectName => 'Nom du projet';

  @override
  String get projectNameRequired => 'Entrez un nom de projet.';

  @override
  String get projectFolder => 'Dossier du projet';

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
  String get projectOptions => 'Options du projet';

  @override
  String get projectToolRulesTitle => 'Autorisations des outils du projet';

  @override
  String get projectToolRulesDescription =>
      'Choisissez une règle pour chaque outil. Les outils sans règle suivent le mode d’autorisation global. Autoriser respecte toujours les limites de fichiers et l’isolation des processus.';

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
  String get projectOptionsToolPermissions => 'Autorisations des outils';

  @override
  String get projectOptionsWorktrees => 'Worktrees Git';

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
  String get emptyChatWelcomeTitle => 'Comment puis-je aider ?';

  @override
  String get emptyChatWelcomeBody =>
      'Posez une question pour commencer à discuter.';

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
      'Connectez un fournisseur et choisissez l\'un de ses modèles pour démarrer une conversation.';

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
  String get sendMessage => 'Envoyer un message';

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
  String get statistics => 'Statistiques';

  @override
  String get statisticsDescription =>
      'Consultez les jetons utilisés, les requêtes et l\'historique des quotas ChatGPT par fournisseur, modèle et conversation.';

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
    return 'Espace de travail : $name';
  }

  @override
  String get modelsPageDescription =>
      'Recherchez et téléchargez des modèles hébergés sur Hugging Face.';

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
  String get modelFormatTransformers => 'Transformers · vLLM';

  @override
  String get modelFormatExllama => 'ExLlama · EXL3';

  @override
  String get huggingFaceModelSearchHint => 'Recherche des modèles Hugging Face';

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
  String get goalSlashCommand => '/goal';

  @override
  String get goalCommandDescription =>
      'Lancer ce message comme objectif et continuer jusqu’à sa réalisation ou jusqu’à avoir besoin de vous.';

  @override
  String get goalObjectiveRequired => 'Saisissez un objectif après /goal.';

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
}
