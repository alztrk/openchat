// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Turkish (`tr`).
class AppLocalizationsTr extends AppLocalizations {
  AppLocalizationsTr([String locale = 'tr']) : super(locale);

  @override
  String get appTitle => 'OpenChat';

  @override
  String get newChat => 'Yeni sohbet';

  @override
  String get chats => 'Sohbetler';

  @override
  String get collapseSidebars => 'Kenar çubuklarını daralt';

  @override
  String get showSidebars => 'Kenar çubuklarını göster';

  @override
  String get home => 'Anasayfa';

  @override
  String get extensions => 'Eklentiler';

  @override
  String get scheduled => 'Zamanlananlar';

  @override
  String get design => 'Tasarım';

  @override
  String get security => 'Güvenlik';

  @override
  String get sectionUnavailable => 'Bu bölüm henüz kullanılamıyor.';

  @override
  String get settings => 'Ayarlar';

  @override
  String get settingsDescription => 'Bağlantılar, görünüm ve yerel veriler';

  @override
  String get connections => 'Bağlantılar';

  @override
  String get sharedInstructions => 'Ortak talimatlar';

  @override
  String get sharedInstructionsDescription =>
      'Bu talimatlar ChatGPT ve OpenCode\'a her sohbette gönderilir. Dosya araçlarının erişimi Araç erişimi ayarına uyar.';

  @override
  String get sharedInstructionsHint =>
      'Yanıtların nasıl verilmesini istediğini yaz...';

  @override
  String get sharedInstructionsLoadFailed =>
      'Ortak talimatlar yüklenemedi. Yeniden deneyin.';

  @override
  String get sharedInstructionsSaveFailed => 'Ortak talimatlar kaydedilemedi.';

  @override
  String get sharedInstructionsSaved => 'Ortak talimatlar kaydedildi.';

  @override
  String get sharedInstructionsTooLong =>
      'Ortak talimatlar 4096 karakteri aşamaz.';

  @override
  String get chatGptProvider => 'ChatGPT';

  @override
  String get openCodeProvider => 'OpenCode';

  @override
  String get favoriteModels => 'Favoriler';

  @override
  String get favoriteModelsEmpty => 'Henüz favori model yok.';

  @override
  String get modelSearchHint => 'Model ara...';

  @override
  String get modelSearchNoResults => 'Aramayla eşleşen model yok.';

  @override
  String get addModelFavorite => 'Favori modellere ekle';

  @override
  String get removeModelFavorite => 'Favorilerden çıkar';

  @override
  String get openCodeConsole => 'OpenCode Console';

  @override
  String get openCodeConsoleDescription =>
      'Ücretsiz modeller anahtar olmadan kullanılabilir. Console bakiyesi ekledikten sonra ücretli modeller istek başına ücretlendirilir. Bu modeller için Console servis API anahtarı ekleyin.';

  @override
  String openCodeKeySaved(Object suffix) {
    return 'Sonu ••••$suffix olan Console API anahtarı bu cihazda güvenli şekilde kayıtlı.';
  }

  @override
  String get openCodeNoKey =>
      'Console API anahtarı yok. Ücretsiz modeller kullanılabilir.';

  @override
  String get openCodeApiKey => 'OpenCode Console API anahtarı';

  @override
  String get openCodeKeyInvalid =>
      'Boş olmayan, tek satırlı ve en fazla 4096 karakterlik bir API anahtarı girin.';

  @override
  String get openCodeKeyStorageFailed =>
      'Console API anahtarı güvenli şekilde okunamadı veya kaydedilemedi.';

  @override
  String get openCodePaidModel => 'Ücretli';

  @override
  String get openCodeFreeModel => 'Ücretsiz';

  @override
  String get add => 'Ekle';

  @override
  String get edit => 'Düzenle';

  @override
  String get exportConversation => 'Sohbeti dışa aktar';

  @override
  String get deleteConversation => 'Sohbeti sil';

  @override
  String get confirmDeleteConversationTitle => 'Sohbet silinsin mi?';

  @override
  String confirmDeleteConversation(String title) {
    return '“$title” sohbeti ve içindeki tüm mesajlar bu cihazdan silinecek. Bu işlem geri alınamaz.';
  }

  @override
  String get stopResponseBeforeDelete =>
      'Silmeden önce devam eden yanıtı durdur.';

  @override
  String get conversationDeleted => 'Sohbet silindi.';

  @override
  String get conversationDeleteFailed => 'Sohbet silinemedi. Tekrar dene.';

  @override
  String get conversationExported =>
      'Sohbet Markdown dosyası olarak dışa aktarıldı.';

  @override
  String get conversationExportFailed =>
      'Sohbet dışa aktarılamadı. Tekrar dene.';

  @override
  String get conversationExportProvider => 'Sağlayıcı';

  @override
  String get conversationExportModel => 'Model';

  @override
  String get conversationExportCreated => 'Oluşturulma';

  @override
  String get conversationExportStatus => 'Durum';

  @override
  String get toolPermissions => 'Araç erişimi';

  @override
  String get toolPermissionsDescription =>
      'Yapay zekanın yerel dosya araçlarını hangi klasörlerde ve hangi izinle kullanacağını seç.';

  @override
  String get toolPermissionRequireApproval => 'Onay İste';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Her araç çağrısında izin sorulur. Erişim proje klasörü ve %LOCALAPPDATA%\\Zihora ile sınırlıdır.';

  @override
  String get toolPermissionFullAccess => 'Tam erişim';

  @override
  String get toolPermissionFullAccessDescription =>
      'Mevcut okuma araçları izin sormadan tüm klasörlere erişebilir. Dosyalar değiştirilmez ve komut çalıştırılmaz.';

  @override
  String get toolPermissionSettingsLoadFailed =>
      'Araç erişim ayarı yüklenemedi.';

  @override
  String get toolPermissionSettingsSaveFailed =>
      'Araç erişim ayarı kaydedilemedi. Tekrar dene.';

  @override
  String get toolPermissionRequestTitle => 'Araç kullanım izni';

  @override
  String get toolPermissionRequestDescription =>
      'Yapay zeka bu aracı belirtilen konumda kullanmak istiyor. İzin yalnızca bu çağrı için geçerli olacak.';

  @override
  String get toolPermissionTarget => 'Erişilecek konum';

  @override
  String get toolPermissionTool => 'Araç';

  @override
  String get toolPermissionArguments => 'İşlem ayrıntıları';

  @override
  String get toolPermissionDeny => 'Reddet';

  @override
  String get toolPermissionStopResponse => 'Yanıtı durdur';

  @override
  String get toolPermissionAllowOnce => 'Bu çağrıya izin ver';

  @override
  String get toolDenied => 'Reddedildi';

  @override
  String get toolCancelled => 'İptal edildi';

  @override
  String get toolAwaitingApproval => 'İzin bekleniyor';

  @override
  String get conversationExportToolActivity => 'Araç etkinliği';

  @override
  String get responseReplaceFailed =>
      'Yeni yanıt kaydedildi ancak önceki yanıt değiştirilemedi.';

  @override
  String get responseRetryNotCompleted =>
      'Yeni yanıt tamamlanmadı. Önceki yanıt korundu.';

  @override
  String get responseRetryCleanupFailed =>
      'Yeni deneme temizlenemedi. Sohbet geçmişini yenile.';

  @override
  String get responseInProgress => 'Yanıt sürüyor';

  @override
  String get responseRetryUnavailable =>
      'Bu yanıt yeniden denenemiyor. Yeni bir mesaj gönder.';

  @override
  String get providerRateLimited =>
      'Sağlayıcı bir kullanım sınırı bildirdi. Daha sonra tekrar dene.';

  @override
  String get providerAuthenticationRequired =>
      'Sağlayıcı isteği reddetti. Bağlantıyı ve model erişimini kontrol et.';

  @override
  String get providerRequestFailed =>
      'Sağlayıcı yanıtı tamamlayamadı. Kayıtlı mesajların kullanılabilir durumda.';

  @override
  String get apiKey => 'API anahtarı';

  @override
  String get apiKeyInputLabel => 'API anahtarı';

  @override
  String get apiKeyRequired => 'Bir API anahtarı gir.';

  @override
  String get apiKeyInvalidFormat =>
      'Geçerli bir OpenAI API anahtarı gir. Anahtar sk- ile başlamalı.';

  @override
  String get apiKeyAlreadySaved => 'Bu API anahtarı zaten kayıtlı.';

  @override
  String get apiKeySaved =>
      'API anahtarı bu cihaza güvenli şekilde kaydedildi.';

  @override
  String get apiKeySaveFailed =>
      'API anahtarı güvenli şekilde kaydedilemedi. Tekrar dene.';

  @override
  String get apiKeyLoadFailed => 'Kayıtlı API anahtarları yüklenemedi.';

  @override
  String get savedApiKey => 'Kayıtlı anahtar';

  @override
  String savedApiKeyWithSuffix(String suffix) {
    return 'Sonu ••••$suffix olan anahtar';
  }

  @override
  String get showApiKey => 'API anahtarını göster';

  @override
  String get hideApiKey => 'API anahtarını gizle';

  @override
  String get retry => 'Tekrar dene';

  @override
  String get save => 'Kaydet';

  @override
  String get saving => 'Kaydediliyor...';

  @override
  String get oauth => 'OAuth';

  @override
  String get oauthSigningIn => 'Oturum açılıyor...';

  @override
  String get oauthBrowserWaiting => 'Tarayıcıda oturum açma işlemini tamamla.';

  @override
  String get oauthConnectionsLoadFailed =>
      'ChatGPT OAuth bağlantıları yüklenemedi.';

  @override
  String get oauthSignInFailed =>
      'ChatGPT oturumu açılamadı. Tarayıcıyı kontrol edip tekrar dene.';

  @override
  String get oauthConnectionAdded => 'ChatGPT hesabı bağlandı.';

  @override
  String get oldCredentialCleanupFailed =>
      'Hesap bağlandı ancak eski bir kimlik bilgisi silinemedi. OpenChat’i yeniden başlatıp tekrar dene.';

  @override
  String get connectionSelectionFailed => 'ChatGPT hesabı seçilemedi.';

  @override
  String get removeChatGptConnection => 'ChatGPT bağlantısını kaldır';

  @override
  String get removeConnectionAction => 'Kaldır';

  @override
  String confirmRemoveConnection(String name) {
    return '$name ve kayıtlı kimlik bilgileri kaldırılsın mı? Sohbetlerin ve mesajların bu cihazda kalacak.';
  }

  @override
  String get connectionRemoveSucceeded =>
      'Bağlantı kaldırıldı. Sohbetlerin ve mesajların kullanılabilir durumda.';

  @override
  String get connectionRemoveFailed => 'Bağlantı kaldırılamadı. Tekrar dene.';

  @override
  String get workspaceSelectionFailed => 'ChatGPT çalışma alanı seçilemedi.';

  @override
  String get chatGptAccount => 'ChatGPT hesabı';

  @override
  String get planUnavailable => 'Plan bilgisi yok';

  @override
  String accountPlan(String plan) {
    return 'Plan: $plan';
  }

  @override
  String get connectionNeedsSignIn =>
      'Bu hesabı kullanmak için yeniden oturum aç.';

  @override
  String get connectionSelected => 'Seçili';

  @override
  String get useConnection => 'Hesabı kullan';

  @override
  String get selectWorkspace => 'Çalışma alanı seç';

  @override
  String get workspaceWithoutName => 'Çalışma alanı';

  @override
  String get workspace => 'Çalışma alanı';

  @override
  String get workspaceUnavailable => 'Çalışma alanı bilgisi yok.';

  @override
  String get selectAccountForWorkspace =>
      'Çalışma alanı seçmek için önce bu hesabı seç.';

  @override
  String get accountEmailUnavailable => 'E-posta bilgisi alınamadı';

  @override
  String accountUsage(String plan) {
    return 'Kullanım · $plan';
  }

  @override
  String get refreshUsage => 'Kullanımı yenile';

  @override
  String get ordinaryUsageAvailable => 'Normal kullanım kullanılabilir.';

  @override
  String get ordinaryUsageUnavailable =>
      'Normal kullanım şu anda kullanılamıyor.';

  @override
  String get ordinaryUsageUnknown => 'Kullanılabilirlik belirlenemedi.';

  @override
  String usageUpdatedAt(String time) {
    return 'Güncelleme: $time';
  }

  @override
  String get usageLoadFailed => 'Kullanım bilgisi yüklenemedi.';

  @override
  String get usageFiveHour => '5 Saatlik';

  @override
  String get usageWeekly => 'Haftalık';

  @override
  String quotaResetsAt(String time) {
    return 'Sıfırlanma: $time';
  }

  @override
  String creditExpiresAt(String time) {
    return 'Son kullanma: $time';
  }

  @override
  String creditGrantedAt(String time) {
    return 'Tanımlanma: $time';
  }

  @override
  String get noResetCredits => 'Kullanılabilir sıfırlama hakkı yok.';

  @override
  String usageUsedPercent(String percent) {
    return '%$percent kullanıldı';
  }

  @override
  String get resetCreditCountUnavailable => 'Sıfırlama hakkı sayısı alınamadı.';

  @override
  String resetCreditsAvailable(int count) {
    return 'Kullanılabilir sıfırlama hakkı: $count';
  }

  @override
  String get resetCredit => 'Sıfırlama hakkı';

  @override
  String get statusUnavailable => 'Durum bilgisi yok';

  @override
  String get resetCreditDetailsUnavailable =>
      'Sıfırlama hakkı ayrıntıları döndürülmedi.';

  @override
  String get noChatGptConnections => 'Henüz ChatGPT bağlantısı yok.';

  @override
  String get apiKeyConnectionUnavailable =>
      'API anahtarı bağlantısı henüz eklenmedi.';

  @override
  String get oauthConnectionUnavailable => 'OAuth bağlantısı henüz eklenmedi.';

  @override
  String get titleGenerationTarget => 'Otomatik sohbet başlıkları';

  @override
  String get titleGenerationTargetDescription =>
      'Sohbet hesabında farklı bir model yoksa OpenChat burada seçtiğin hesabı kullanabilir. Normal kullanım hakkı yoksa başlık oluşturmaz.';

  @override
  String get titleUseConversationAccount => 'Sohbet hesabını kullan';

  @override
  String get titleAccountUnavailable => 'Seçili başlık hesabı kullanılamıyor';

  @override
  String get titleWorkspaceHint => 'Başlıklar için çalışma alanı seç';

  @override
  String get titleWorkspaceRequired =>
      'Bu hesapla başlık oluşturmak için önce çalışma alanı seç.';

  @override
  String get titlePreferenceLoadFailed => 'Başlık hesabı tercihi yüklenemedi.';

  @override
  String get titlePreferenceSaveFailed =>
      'Başlık hesabı tercihi kaydedilemedi.';

  @override
  String get appearance => 'Görünüm';

  @override
  String get themeSettingDescription => 'Uygulamanın görünümünü seçin.';

  @override
  String get language => 'Uygulama dili';

  @override
  String get languageSettingDescription =>
      'Uygulamanın kullanacağı dili seçin.';

  @override
  String get systemLanguage => 'Cihaz dili';

  @override
  String get englishLanguage => 'İngilizce';

  @override
  String get turkishLanguage => 'Türkçe';

  @override
  String get languageSaveFailed =>
      'Dil tercihi kaydedilemedi. Lütfen tekrar deneyin.';

  @override
  String get systemTheme => 'Sistem';

  @override
  String get lightTheme => 'Açık';

  @override
  String get darkTheme => 'Koyu';

  @override
  String get localData => 'Yerel veriler';

  @override
  String get conversationHistory => 'Sohbet geçmişi';

  @override
  String get historyDeviceDescription => 'Konuşmalar bu cihazda saklanır.';

  @override
  String get historyDeviceStatus => 'Bu cihazda';

  @override
  String get historyCheckingDescription => 'Yerel sohbet geçmişi hazırlanıyor.';

  @override
  String get historyCheckingStatus => 'Hazırlanıyor';

  @override
  String get historyStorageUnavailableDescription =>
      'Yerel sohbet geçmişi açılamadı. Uygulamayı yeniden başlatıp tekrar dene.';

  @override
  String get historyStorageUnavailableStatus => 'Kullanılamıyor';

  @override
  String get historyLoading => 'Sohbetler yükleniyor...';

  @override
  String get historyLoadFailed =>
      'Sohbet geçmişi yüklenemedi. Uygulamayı yeniden başlat.';

  @override
  String get messageHistoryLoadFailed => 'Bu sohbetin mesajları yüklenemedi.';

  @override
  String get messageHistoryLoading => 'Sohbet mesajları yükleniyor.';

  @override
  String get clearConversationHistory => 'Tüm geçmişi sil';

  @override
  String get clearConversationHistoryDescription =>
      'Bu cihazdaki sohbetleri ve mesajları kalıcı olarak sil.';

  @override
  String get confirmClearHistoryTitle => 'Tüm sohbet geçmişi silinsin mi?';

  @override
  String get confirmClearHistoryBody =>
      'Bu cihazdaki tüm sohbetleri ve mesajları kalıcı olarak siler. Bu işlem geri alınamaz.';

  @override
  String get cancel => 'Vazgeç';

  @override
  String get deleteAll => 'Tümünü sil';

  @override
  String get clearingHistory => 'Siliniyor...';

  @override
  String get clearHistorySucceeded => 'Sohbet geçmişi silindi.';

  @override
  String get clearHistoryFailed =>
      'Sohbet geçmişi silinemedi. Lütfen tekrar deneyin.';

  @override
  String get themeSaveFailed =>
      'Tema tercihi kaydedilemedi. Lütfen tekrar deneyin.';

  @override
  String get searchChats => 'Sohbetlerde ara';

  @override
  String get searchChatsHint => 'Sohbetlerde ara';

  @override
  String get projects => 'Projeler';

  @override
  String get noProjects => 'Henüz proje yok';

  @override
  String get createProject => 'Proje oluştur';

  @override
  String get projectName => 'Proje adı';

  @override
  String get projectNameRequired => 'Bir proje adı gir.';

  @override
  String get projectFolder => 'Proje klasörü';

  @override
  String get chooseProjectFolder => 'Klasör seç';

  @override
  String get projectFolderNotSelected => 'Devam etmek için bir klasör seç.';

  @override
  String get projectFolderSelectionFailed => 'Klasör seçilemedi.';

  @override
  String get projectCreateFailed => 'Proje oluşturulamadı.';

  @override
  String get projectCreated => 'Proje oluşturuldu.';

  @override
  String get projectLoadFailed => 'Projeler yüklenemedi.';

  @override
  String get projectMoveFailed => 'Sohbet projeye taşınamadı.';

  @override
  String get pinnedChats => 'Sabitlenenler';

  @override
  String get noPinnedChats => 'Henüz sabitlenen sohbet yok';

  @override
  String get noChatsTitle => 'Henüz sohbet yok';

  @override
  String get noChatsSearchTitle => 'Aranacak sohbet yok';

  @override
  String get showMore => 'Daha fazla göster';

  @override
  String get projectOptions => 'Proje seçenekleri';

  @override
  String get newProjectConversation => 'Projede yeni sohbet başlat';

  @override
  String get newConversation => 'Yeni bir sohbet başlat';

  @override
  String get conversationTitle => 'Yeni sohbet';

  @override
  String get renameConversation => 'Sohbet başlığını düzenle';

  @override
  String get pinConversation => 'Sohbeti sabitle';

  @override
  String get unpinConversation => 'Sabitlemeyi kaldır';

  @override
  String get conversationTitleRequired => 'Sohbet başlığı boş olamaz.';

  @override
  String get conversationTitleSaveFailed => 'Sohbet başlığı kaydedilemedi.';

  @override
  String get conversationModelSaveFailed =>
      'Sohbet modeli kaydedilemedi. Önceki model kullanılmaya devam ediyor.';

  @override
  String get moreOptions => 'Diğer seçenekler';

  @override
  String get emptyChatWelcomeTitle => 'Nasıl yardımcı olabilirim?';

  @override
  String get emptyChatWelcomeBody => 'Aklındaki soruyu yaz, sohbeti başlat.';

  @override
  String get switchToDarkMode => 'Koyu temaya geç';

  @override
  String get switchToLightMode => 'Açık temaya geç';

  @override
  String get theme => 'Tema';

  @override
  String get keyboardHint => 'Enter ile gönder · Shift+Enter ile yeni satır';

  @override
  String get minimizeWindow => 'Pencereyi küçült';

  @override
  String get maximizeWindow => 'Pencereyi büyüt';

  @override
  String get restoreWindow => 'Pencereyi geri yükle';

  @override
  String get modelSelection => 'Model seç';

  @override
  String get noModelConnected => 'Henüz bağlı bir model yok';

  @override
  String get noModelConnectedBody =>
      'Sohbet başlatmak için bir ChatGPT hesabı bağlayıp model seç.';

  @override
  String get close => 'Kapat';

  @override
  String get reasoning => 'Akıl yürütme';

  @override
  String get reasoningMedium => 'Orta';

  @override
  String get reasoningMinimal => 'En az';

  @override
  String get reasoningLow => 'Düşük';

  @override
  String get reasoningHigh => 'Yüksek';

  @override
  String get reasoningExtraHigh => 'Çok yüksek';

  @override
  String get reasoningMax => 'En yüksek';

  @override
  String get reasoningUltra => 'En üst';

  @override
  String get stop => 'Durdur';

  @override
  String get modelsLoading => 'Modeller yükleniyor...';

  @override
  String get modelsUnavailable => 'Modeller kullanılamıyor';

  @override
  String get noModelsAvailable =>
      'Bu hesap için şu anda kullanılabilir model yok.';

  @override
  String get chatGptDataUnavailable =>
      'ChatGPT, OpenChat’in okuyamadığı bir bilgi döndürdü. Bağlantıyı yenileyip tekrar dene.';

  @override
  String get modelCatalogUnavailable =>
      'Model listesi kullanılamıyor. Hesap bağlantısını yenileyip tekrar dene.';

  @override
  String get messageSaveFailed =>
      'Mesajın yerel sohbet geçmişine kaydedilemedi.';

  @override
  String get chatHistoryUnavailable =>
      'Sohbet geçmişi güncellenemedi. Lütfen tekrar dene.';

  @override
  String get chatRequestFailed =>
      'ChatGPT yanıtı tamamlayamadı. Kayıtlı mesajların kullanılabilir durumda.';

  @override
  String get chatGptRateLimited =>
      'ChatGPT kullanım sınırı bildirdi. Bu hesabın kullanım ayrıntılarını kontrol et.';

  @override
  String get chatGptReauthenticationRequired =>
      'Bu ChatGPT bağlantısı için yeniden oturum açmalısın. Ayarlar’dan hesabı yeniden bağla.';

  @override
  String get chatGptProviderChanged =>
      'ChatGPT’nin yanıt biçimi değişti. OpenChat’i güncelleyip tekrar dene.';

  @override
  String get cachedCatalog => 'önbellekteki modeller';

  @override
  String get reasoningUnavailable =>
      'Model bağlanana kadar akıl yürütme kullanılamaz.';

  @override
  String get attachFile => 'Dosya ekle';

  @override
  String get attachmentsUnavailable => 'Dosya ekleme henüz kullanılamıyor.';

  @override
  String get messageHint => 'Mesajını yaz...';

  @override
  String get sendMessage => 'Mesajı gönder';

  @override
  String get send => 'Gönder';

  @override
  String get welcomeTitle => 'Sohbet başlat';

  @override
  String get welcomeBody =>
      'Sohbet başlatmadan önce bir yapay zekâ modeli bağla.';

  @override
  String get historyOpen => 'Sohbet geçmişini aç';

  @override
  String get assistantDisclaimer =>
      'Yapay zekâ yanıtları hatalı olabilir. Önemli bilgileri kontrol et.';

  @override
  String get modelRequired => 'Mesaj göndermek için önce bir model bağla.';

  @override
  String get messageModelUnavailable => 'Model bilgisi yok';

  @override
  String get userMessage => 'Kullanıcı';

  @override
  String get copyMessage => 'Mesajı kopyala';

  @override
  String get messageCopied => 'Mesaj panoya kopyalandı.';

  @override
  String get messageCopyFailed => 'Mesaj panoya kopyalanamadı.';

  @override
  String get today => 'Bugün';

  @override
  String get unavailableTime => '—:—';

  @override
  String get unavailableValue => '—';

  @override
  String responseMetadata(String rate, String tokens, String time) {
    return '$rate t/s · $tokens token · $time';
  }

  @override
  String get responseCompleted => 'Yanıt tamamlandı';

  @override
  String responseCompletedWithDuration(String duration) {
    return 'Yanıt tamamlandı · $duration';
  }

  @override
  String get reasoningSummary => 'Akıl yürütme özeti';

  @override
  String reasoningSummaryWithDuration(String duration) {
    return 'Akıl yürütme özeti · $duration';
  }

  @override
  String get reasoningSummaryTooltip =>
      'Modelin sağladığı özet. Süre, özetin akışta görünmesi için geçen zamandır; gizli düşünce verisi değildir.';

  @override
  String get toolRunning => 'Çalışıyor';

  @override
  String get toolCompleted => 'Tamamlandı';

  @override
  String get toolFailed => 'Hata';

  @override
  String get toolInput => 'Girdi';

  @override
  String get toolOutput => 'Çıktı';

  @override
  String get toolListFiles => 'Dosyaları listele';

  @override
  String get toolSearchFiles => 'Dosyalarda ara';

  @override
  String get toolReadFile => 'Dosyayı oku';

  @override
  String get toolGetFileInfo => 'Dosya bilgisi al';

  @override
  String get toolTechnicalDetails => 'Ayrıntılar';

  @override
  String toolFileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe',
      one: '1 öğe',
      zero: '0 öğe',
    );
    return '$_temp0';
  }

  @override
  String get toolEmptyListing => 'Bu klasörde gösterilecek öğe yok.';

  @override
  String get toolListingUnavailable => 'Dosya listesi görüntülenemiyor.';

  @override
  String get toolListingIncomplete => 'Bazı öğeler listelenemedi.';

  @override
  String get toolMoreFilesAvailable => 'Diğer öğeler de var.';

  @override
  String get toolDesktopLocation => 'Masaüstü';

  @override
  String get toolProjectLocation => 'Proje klasörü';

  @override
  String get toolZihoraLocation => 'Uygulama veri klasörü';

  @override
  String get responseFailed => 'Yanıt alınamadı';

  @override
  String get responseStopped => 'Yanıt durduruldu';

  @override
  String secondsShort(int count) {
    return '$count sn';
  }
}
