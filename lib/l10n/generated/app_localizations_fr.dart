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
      'Les outils de fichiers demandent une autorisation avant chaque appel et sont limités au dossier du projet et à %LOCALAPPDATA%\\OpenChat. L’exécution de commandes n’est pas disponible.';

  @override
  String get toolPermissionFullAccess => 'Accès complet';

  @override
  String get toolPermissionFullAccessDescription =>
      'Les outils de fichiers peuvent lire et modifier des fichiers dans n\'importe quel dossier sans rien demander. L\'exécution de la commande n\'est pas disponible.';

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
  String get searchChatsHint => 'Recherchez vos discussions';

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
  String get pinConversation => 'Épingler le chat';

  @override
  String get unpinConversation => 'Désépingler le chat';

  @override
  String get conversationTitleRequired =>
      'Le titre du chat ne peut pas être vide.';

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
  String responseMetadata(String rate, String tokens, String time) {
    return '$rate jetons/s · $tokens jetons · $time';
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
  String get toolSendTerminalInput => 'Envoyer l\'entrée du terminal';

  @override
  String get toolWebSearch => 'Recherche sur le Web';

  @override
  String get toolReadUrlContent => 'Lire la page Web';

  @override
  String get toolSearchQuery => 'Requête de recherche';

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
}
