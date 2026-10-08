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
  String get workspaces => 'Çalışma alanları';

  @override
  String get workspacesDescription => 'İlgili sohbetleri bir arada tutun.';

  @override
  String get workspaceCreate => 'Yeni çalışma alanı';

  @override
  String get workspaceCreateAction => 'Çalışma alanı oluştur';

  @override
  String get workspaceName => 'Çalışma alanı adı';

  @override
  String get workspaceNameRequired => 'Bir çalışma alanı adı girin.';

  @override
  String get workspaceEmptyTitle => 'Henüz çalışma alanı yok';

  @override
  String get workspaceEmptyDescription =>
      'İlgili sohbetleri gruplamak için bir çalışma alanı oluşturun.';

  @override
  String get workspaceNoConversations =>
      'Bu çalışma alanında henüz sohbet yok.';

  @override
  String get workspaceMoveConversation => 'Sohbeti taşı';

  @override
  String get workspaceUnassignedChats => 'Çalışma alanına eklenmemiş sohbetler';

  @override
  String get workspaceNoWorkspace => 'Çalışma alanı yok';

  @override
  String workspaceConversationCount(int count) {
    return 'Sohbet sayısı: $count';
  }

  @override
  String get workspaceRename => 'Çalışma alanını yeniden adlandır';

  @override
  String get workspaceDelete => 'Çalışma alanını sil';

  @override
  String workspaceDeleteConfirmation(String name) {
    return '“$name” silinsin mi? Sohbetleri Sohbetler bölümünde kalır.';
  }

  @override
  String get workspaceLoadFailed => 'Çalışma alanları yüklenemedi.';

  @override
  String get workspaceSaveFailed => 'Çalışma alanı kaydedilemedi.';

  @override
  String get workspaceDeleteFailed => 'Çalışma alanı silinemedi.';

  @override
  String get workspaceMoveFailed => 'Sohbet taşınamadı.';

  @override
  String get outputs => 'Çıktılar';

  @override
  String get outputsDescription =>
      'Yararlı yanıtları kaydedin ve buradan yeniden açın.';

  @override
  String get outputsEmptyTitle => 'Henüz kaydedilmiş yanıt yok';

  @override
  String get outputsEmptyDescription =>
      'Bir yanıtı burada tutmak için altındaki Yanıtı kaydet seçeneğini kullanın.';

  @override
  String get outputsLoadFailed => 'Kaydedilmiş yanıtlar yüklenemedi.';

  @override
  String get saveResponse => 'Yanıtı kaydet';

  @override
  String get removeSavedResponse => 'Kaydedilmiş yanıtı kaldır';

  @override
  String get outputSaveFailed => 'Yanıt kaydedilemedi.';

  @override
  String get outputRemoveFailed => 'Kaydedilmiş yanıt kaldırılamadı.';

  @override
  String get outputOpenConversation => 'Sohbeti aç';

  @override
  String get outputConversationUnavailable => 'Sohbet artık kullanılamıyor';

  @override
  String outputSavedAt(String date) {
    return 'Kaydedilme tarihi: $date';
  }

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
  String get models => 'Modeller';

  @override
  String get modelsDescription =>
      'Bağlı sağlayıcıların modellerini yönetin, varsayılan modeli belirleyin ve istemediğiniz modelleri gizleyin.';

  @override
  String get localEngines => 'Yerel motorlar';

  @override
  String get localEnginesDescription =>
      'Doğrulanmış yerel motorları kurun, kendi model dosyalarınızı kaydedin ve motorun çalışma durumunu kontrol edin. Modelleri motor klasörüne taşıyabilir, kopyalayabilir veya mevcut konumunda bırakabilirsiniz.';

  @override
  String get localEnginesUnavailable =>
      'Yerel motor servisi henüz kullanılamıyor.';

  @override
  String get localEnginesLoadFailed => 'Yerel motor bilgileri yüklenemedi.';

  @override
  String get localEnginesEmpty => 'Kullanılabilir yerel motor sürümü yok.';

  @override
  String get localEnginesReload => 'Yenile';

  @override
  String localEngineRelease(String tag) {
    return 'Sürüm $tag';
  }

  @override
  String get localEngineVariants => 'Paketler';

  @override
  String get localEngineStable => 'Kararlı';

  @override
  String get localEnginePreview => 'Ön izleme';

  @override
  String get localEngineNightly => 'Gecelik';

  @override
  String get localEngineRecommended => 'Önerilen';

  @override
  String get localEngineAvailable => 'Kullanılabilir';

  @override
  String get localEngineInstalled => 'Kurulu';

  @override
  String get localEngineNotInstalled => 'Kurulu değil';

  @override
  String get localEngineBlocked => 'Engellendi';

  @override
  String get localEngineDeprecated => 'Kullanımdan kaldırıldı';

  @override
  String get localEngineWindowsDeprecatedReason =>
      'Windows\'ta vLLM ve ExLlama\'nın yeni kurulumları devre dışı. Mevcut motor dosyaları ve model kayıtları korunur.';

  @override
  String get localEngineUnsupportedPlatform => 'Desteklenmeyen platform';

  @override
  String get localEngineHardwareUnavailable => 'Donanım kullanılamıyor';

  @override
  String get localEngineDriverUnsupported => 'NVIDIA sürücüsünü güncelleyin';

  @override
  String get localEngineDriverVersionUnavailable =>
      'NVIDIA sürücü sürümü doğrulanamadı';

  @override
  String get localEngineVllmBlockedReason =>
      'vLLM için 580 veya üzeri NVIDIA sürücüsü ve mevcut bir Linux ortamı gerekir. Windows\'ta GPU erişimi olan, önceden kurulmuş bir WSL2 dağıtımı kullanılır.';

  @override
  String get localEngineExllamaBlockedReason =>
      'ExLlamaV3 çalışma zamanı henüz kurulum için hazır değil. Kurulum seçeneği sunulmadan önce TabbyAPI, PyTorch, Triton, Flash Linear Attention ve Python bağımlılıklarının tamamı sabitlenip doğrulanmalı.';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Gereksinimler: $requirements';
  }

  @override
  String get localEngineInstall => 'Kur';

  @override
  String get localEngineInstalling => 'Kurulum hazırlanıyor...';

  @override
  String get localEngineInstallProgress => 'Yerel motor kurulum ilerlemesi';

  @override
  String get localEngineCancelInstall => 'Kurulumu iptal et';

  @override
  String get localEngineCancellingInstall => 'İptal ediliyor...';

  @override
  String get localEngineInstallFailed =>
      'Yerel motor kurulamadı. Katalog yenilendi.';

  @override
  String get localEngineHealth => 'Çalışma durumu';

  @override
  String get localEngineExecutable => 'llama-server çalıştırılabilir dosyası';

  @override
  String get localEngineExecutableDescription =>
      'Kayıtlı GGUF modellerini OpenChat\'in başlatması için bir llama-server dosyası seçin. Bu seçenek, kendiniz başlattığınız bir sunucuya bağlanmaktan ayrıdır.';

  @override
  String get localEngineExecutableChoose => 'Çalıştırılabilir dosya seç';

  @override
  String get localEngineExecutableClear => 'Seçimi temizle';

  @override
  String get localEngineExecutableNotConfigured =>
      'Bir dosya seçilmedi. OpenChat, kurulu bir paketi kullanabilir.';

  @override
  String get localEngineExecutableMissing =>
      'Kaydedilen çalıştırılabilir dosya bulunamadı. Dosyayı yeniden seçin.';

  @override
  String get localEngineExecutableInvalid =>
      'Var olan bir llama-server çalıştırılabilir dosyası seçin.';

  @override
  String get localEngineSettingsFailed =>
      'llama-server dosyası ayarı kaydedilemedi.';

  @override
  String get localEngineExternalServerCheck => 'Çalışan llama-server\'ı ara';

  @override
  String get localEngineExternalServerNotConnected =>
      'Kullanıcının başlattığı bir sunucuya bağlı değil. OpenChat bu sunucuyu başlatmaz veya durdurmaz.';

  @override
  String get localEngineExternalServerConnecting =>
      'Seçilen yerel sunucu denetleniyor...';

  @override
  String get localEngineExternalServerFoundTitle =>
      'Çalışan llama-server bulundu';

  @override
  String localEngineExternalServerFoundDescription(int port) {
    return 'Bir llama.cpp sunucusu $port portunu dinliyor. OpenChat bağlanıp modellerini listeleyecek. OpenChat\'teki bağlantıyı kesmek sunucuyu durdurmaz.';
  }

  @override
  String get localEngineExternalServerNotNow => 'Şimdi değil';

  @override
  String get localEngineExternalServerConnect => 'Bağlan';

  @override
  String localEngineExternalServerConnected(int port, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count model kullanılabilir',
      one: '1 model kullanılabilir',
    );
    return '$port portuna bağlandı. $_temp0.';
  }

  @override
  String get localEngineManagedModelSection =>
      'OpenChat\'in yönettiği modeller';

  @override
  String localEngineManagedServerModelSection(int port) {
    return 'OpenChat sunucusu · 127.0.0.1:$port';
  }

  @override
  String localEngineExternalModelSection(int port) {
    return 'Kullanıcı sunucusu · 127.0.0.1:$port';
  }

  @override
  String get localEngineExternalServerDisconnect => 'Bağlantıyı kes';

  @override
  String get localEngineExternalServerNotFound =>
      'Çalışan bir llama-server bulunamadı.';

  @override
  String get localEngineExternalServerScanFailed =>
      'Çalışan llama-server süreçleri denetlenemedi.';

  @override
  String get localEngineExternalServerConnectFailed =>
      'Bağlanılamadı. llama-server\'ın hazır olduğunu ve yerel model uç noktasını sunduğunu kontrol edin.';

  @override
  String get localEngineExternalServerAuthRequired =>
      'Bu sunucu kimlik doğrulaması istiyor. OpenChat diğer süreçlerin kimlik bilgilerini okumaz veya yeniden kullanmaz.';

  @override
  String get localEngineRunning => 'Çalışıyor';

  @override
  String get localEngineStopped => 'Durduruldu';

  @override
  String get localEngineUnhealthy => 'Yanıt vermiyor';

  @override
  String get localEngineUnavailable => 'Bu motor şu an çalıştırılamıyor.';

  @override
  String get localEngineStartModel => 'Modeli başlat';

  @override
  String get localEngineStopModel => 'Motoru durdur';

  @override
  String get localModels => 'Kayıtlı modeller';

  @override
  String get localModelsEmpty => 'Bu motor için kayıtlı model yok.';

  @override
  String get localModelsPageTitle => 'Yerel modeller';

  @override
  String get localModelsPageDescription =>
      'İndirilen ve kaydedilen yerel modelleri görüntüleyin.';

  @override
  String get localModelsPageEmpty => 'Henüz kayıtlı bir yerel model yok.';

  @override
  String get localModelsLoadFailed => 'Yerel modeller yüklenemedi.';

  @override
  String get localModelsRefresh => 'Yenile';

  @override
  String get localModelsDiscover => 'Modelleri keşfet';

  @override
  String get localModelAddFile => 'Model dosyası ekle';

  @override
  String get localModelAddFolder => 'Model klasörü ekle';

  @override
  String get localModelStorageChoiceTitle => 'Modelin konumunu seçin';

  @override
  String localModelStorageChoiceTarget(String folder) {
    return 'Seçilen model klasörü: $folder';
  }

  @override
  String get localModelDirectoryTitle => 'Model klasörü';

  @override
  String get localModelDirectoryDescription =>
      'Bu motorun modellerinin kaydedileceği klasörü seçin. Bu ayar mevcut model kayıtlarını taşımaz.';

  @override
  String get localModelChooseDirectory => 'Klasör seç';

  @override
  String get localModelUseDefaultDirectory => 'Varsayılanı kullan';

  @override
  String get localModelScanDirectory => 'Klasörü tara';

  @override
  String get localModelScanningDirectory => 'Klasör taranıyor...';

  @override
  String get localModelDiscoveryTitle => 'Kayıtsız modeller bulundu';

  @override
  String localModelDiscoveryPrompt(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'OpenChat bu klasörde kayıtlı olmayan $count desteklenen model buldu. Kaydedilsin mi?',
      one: 'OpenChat bu klasörde kayıtlı olmayan 1 desteklenen model buldu. Kaydedilsin mi?',
    );
    return '$_temp0';
  }

  @override
  String get localModelDiscoveryTruncated =>
      'Tarama güvenli sınırına ulaştı. Daha fazla model bulmak için daha küçük bir klasör seçin.';

  @override
  String get localModelDiscoveryEmpty =>
      'Bu klasörde yeni ve desteklenen bir model bulunamadı.';

  @override
  String get localModelDiscoveryRegisterAll => 'Bulunan modelleri kaydet';

  @override
  String localModelDiscoveryRegistered(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count model kaydedildi.',
      one: '1 model kaydedildi.',
    );
    return '$_temp0';
  }

  @override
  String localModelDiscoveryPartial(int registered, int total) {
    return '$total modelden $registered tanesi kaydedildi. Bazı modeller kaydedilemedi.';
  }

  @override
  String get localModelDirectoryUnavailable =>
      'Bu model klasörüne erişilemiyor. Var olan ve OpenChat\'in erişebildiği bir klasör seçin.';

  @override
  String get localModelDiscoveryFailed =>
      'Model klasörü taranamadı. Erişim izinlerini kontrol edip yeniden deneyin.';

  @override
  String get localModelMoveToFolder => 'Modeli bu klasöre taşı';

  @override
  String get localModelCopyToFolder => 'Modeli bu klasöre kopyala';

  @override
  String get localModelKeepInPlace => 'Model olduğu yerde kalsın';

  @override
  String get localModelSaving => 'Model kaydediliyor...';

  @override
  String get localModelTransferError =>
      'Model taşınamadı veya kopyalanamadı. Özgün model yerinde bırakıldı.';

  @override
  String get localModelTransferRecoveryError =>
      'Model kaydedilemedi veya eski konumuna döndürülemedi. Tam bir kopyası OpenChat model klasöründe kaldı; kaydetmek için o klasörden yeniden seçin.';

  @override
  String get localModelRemove => 'Kaydı kaldır';

  @override
  String get localModelRemoveConfirmation =>
      'Model dosyası silinmeden yalnızca OpenChat kaydı kaldırılır. Devam edilsin mi?';

  @override
  String get localModelCancelStart => 'Başlatmayı iptal et';

  @override
  String get localModelStopping => 'Motor durduruluyor...';

  @override
  String get localModelActionError => 'Yerel model işlemi tamamlanamadı.';

  @override
  String get localModelPathMissing =>
      'Model yolu bulunamadı. Dosyayı geri taşıyın veya kaydı kaldırın.';

  @override
  String get localModelEngineNotReady =>
      'Motor kurulup çalıştırılabilir duruma gelince kullanılabilir.';

  @override
  String get localModelInvalid =>
      'Seçilen dosya veya klasör bu motor için geçerli bir model değil.';

  @override
  String get localModelPathError =>
      'Model dosyasına veya klasörüne erişilemiyor.';

  @override
  String get localModelStoragePathError =>
      'OpenChat model klasörleri oluşturulamadı. Depolama alanını ve izinleri kontrol edip yeniden deneyin.';

  @override
  String get localModelStorageError => 'Model kaydı veritabanına yazılamadı.';

  @override
  String get localModelStartError =>
      'Model başlatılamadı. Motorun kurulumunu ve model dosyasını kontrol edin.';

  @override
  String get localModelStartTimeout =>
      'Model belirtilen süre içinde hazır olmadı. Daha küçük bir model deneyin veya donanımınızı kontrol edin.';

  @override
  String get localModelRuntimeUnavailable =>
      'Yerel model yanıt vermeyi durdurdu. Ayarlar > Yerel motorlar bölümünden yeniden başlatıp tekrar deneyin.';

  @override
  String get localModelContextUnavailable =>
      'Yerel motor etkin bağlam sınırını bildirmedi. llama.cpp\'yi güncelleyip veya yeniden kurup tekrar deneyin.';

  @override
  String get localModelInferenceFailed =>
      'Yerel model bu isteği işleyemedi. Sohbet şablonunu ve kullanılabilir belleği kontrol edin.';

  @override
  String get localModelSaved => 'Yerel model kaydedildi.';

  @override
  String get localModelRemoved => 'Yerel model kaydı kaldırıldı.';

  @override
  String get localEngineStageDownloading => 'İndiriliyor';

  @override
  String get localEngineStageVerifying => 'Doğrulanıyor';

  @override
  String get localEngineStageExtracting => 'Çıkartılıyor';

  @override
  String get localEngineStageRuntimeSetup =>
      'Python çalışma zamanı hazırlanıyor';

  @override
  String get localEngineStagePublishing => 'Kurulum tamamlanıyor';

  @override
  String get localEngineStageReady => 'Hazır';

  @override
  String get defaultModel => 'Varsayılan';

  @override
  String get setDefaultModel => 'Varsayılan Yap';

  @override
  String get clearDefaultModel => 'Varsayılanı Kaldır';

  @override
  String defaultModelUpdated(String model) {
    return 'Varsayılan model güncellendi: $model';
  }

  @override
  String get defaultModelCleared => 'Varsayılan model kaldırıldı.';

  @override
  String get hideModel => 'Gizle';

  @override
  String get showModel => 'Göster';

  @override
  String get hiddenModel => 'Gizli';

  @override
  String modelHidden(String model) {
    return 'Model gizlendi: $model';
  }

  @override
  String modelUnhidden(String model) {
    return 'Model görünür yapıldı: $model';
  }

  @override
  String get noModelsFound => 'Model bulunamadı.';

  @override
  String get refreshModels => 'Modelleri Yenile';

  @override
  String get connectedProvidersModels => 'Bağlı sağlayıcı modelleri';

  @override
  String get noConnectedProviders =>
      'Henüz bağlı bir sağlayıcı yok. Bağlantılar sekmesinden hesap bağlayabilir veya API anahtarı ekleyebilirsiniz.';

  @override
  String get sharedInstructions => 'Ortak talimatlar';

  @override
  String get sharedInstructionsDescription =>
      'Bu talimatlar bağlı olan tüm sağlayıcılara her sohbette gönderilir. Dosya araçlarının erişimi Araç erişimi ayarına uyar.';

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
      'Google AI Studio API anahtarını ekleyerek erişebildiğin modelleri kullan. Ücretsiz kullanım model ve kota sınırına bağlıdır.';

  @override
  String get groqApiDescription =>
      'Groq API anahtarıyla hesabında erişime açık modelleri kullan. Ücret ve kullanım limitleri planına ve modele göre değişir.';

  @override
  String get cerebrasApiDescription =>
      'Cerebras API anahtarıyla hesabında araç destekli modelleri kullan. Erişim, ücret ve limitler modele ve hesabına göre değişir.';

  @override
  String get openRouterApiDescription =>
      'Yalnızca katalogda girdi ve çıktı fiyatı 0 \$ olan, metin ve araç desteği görünen modeller listelenir. Gerçek erişim ve limitler değişebilir.';

  @override
  String get mistralApiDescription =>
      'Mistral API anahtarını ekleyerek hesabında erişime açık sohbet modellerini kullan. Ücretsiz kullanım, ücret ve limitler Mistral planına göre değişir.';

  @override
  String get geminiUnpaidDataNotice =>
      'Gemini API\'nin ücretsiz katmanında gönderdiğin içerikler Google ürünlerini geliştirmek için kullanılabilir. Hassas içerik göndermeden önce Google\'ın veri kullanım koşullarını incele.';

  @override
  String get providerApiKey => 'API anahtarı';

  @override
  String get providerKeySaved =>
      'API anahtarı bu cihazda güvenli şekilde kayıtlı.';

  @override
  String providerKeySavedSuffix(Object suffix) {
    return 'Sonu ••••$suffix olan API anahtarı bu cihazda güvenli şekilde kayıtlı.';
  }

  @override
  String get providerNoKey => 'API anahtarı bağlı değil.';

  @override
  String get providerKeyInvalid =>
      'Bu sağlayıcı için geçerli bir API anahtarı gir. Boşluk kullanma; anahtar en fazla 4096 karakter olabilir.';

  @override
  String get providerKeyStorageFailed =>
      'API anahtarı güvenli şekilde okunamadı veya kaydedilemedi.';

  @override
  String get favoriteModels => 'Favoriler';

  @override
  String get favoriteModelsEmpty => 'Henüz favori model yok.';

  @override
  String get modelSearchHint => 'Model ara...';

  @override
  String get chatGptFastModeEnabledTooltip =>
      'Fast mod isteği açık. Abonelik kredisini daha hızlı tüketebilir veya API token maliyetini artırabilir.';

  @override
  String get chatGptFastModeDisabledTooltip =>
      'Fast modu iste. Abonelik kredisini daha hızlı tüketebilir veya API token maliyetini artırabilir; kullanılabilirlik modele bağlıdır.';

  @override
  String get chatGptFastModeUnavailableTooltip =>
      'Seçili model Fast mod desteği bildirmiyor.';

  @override
  String get chatGptFastModeLoadingTooltip => 'Fast mod tercihi yükleniyor…';

  @override
  String get chatGptFastModeSettingsLoadFailed =>
      'Fast mod tercihi yüklenemedi.';

  @override
  String get chatGptFastModeSettingsSaveFailed =>
      'Fast mod tercihi kaydedilemedi.';

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
      'Ücretsiz modeller anahtarsız kullanılabilir. Ücretli modeller için Console API anahtarı ekle; kullanım istek başına Console bakiyenden düşer.';

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
      'Boşluk veya satır sonu içermeyen, boş olmayan bir API anahtarı girin (en fazla 4096 karakter).';

  @override
  String get openCodeKeyStorageFailed =>
      'Console API anahtarı güvenli şekilde okunamadı veya kaydedilemedi.';

  @override
  String get openCodePaidModel => 'Ücretli';

  @override
  String get openCodeFreeModel => 'Ücretsiz';

  @override
  String get modelSourceApi => 'API';

  @override
  String get modelSourceOAuth => 'OAuth';

  @override
  String get openCodeFreeModels => 'Ücretsiz modeller';

  @override
  String get openCodeApiModels => 'API modelleri';

  @override
  String modelContextWindow(String value) {
    return 'Bağlam penceresi · $value token';
  }

  @override
  String openCodeModelContextWindow(String value) {
    return 'OpenCode kataloğu (Models.dev) · bağlam: $value token';
  }

  @override
  String get add => 'Ekle';

  @override
  String get edit => 'Düzenle';

  @override
  String get exportConversation => 'Sohbeti dışa aktar';

  @override
  String get deleteConversation => 'Sohbeti sil';

  @override
  String get archiveConversation => 'Sohbeti arşivle';

  @override
  String get restoreConversation => 'Sohbeti arşivden çıkar';

  @override
  String get archivedChats => 'Arşivlenenler';

  @override
  String get noArchivedChats => 'Arşivlenmiş sohbet yok.';

  @override
  String get stopResponseBeforeArchive =>
      'Bu sohbeti arşivlemeden önce devam eden yanıtı durdur.';

  @override
  String get conversationArchived => 'Sohbet arşivlendi.';

  @override
  String get conversationRestored => 'Sohbet arşivden çıkarıldı.';

  @override
  String get conversationArchiveFailed => 'Sohbet arşivlenemedi. Tekrar dene.';

  @override
  String get conversationRestoreFailed =>
      'Sohbet arşivden çıkarılamadı. Tekrar dene.';

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
  String get conversationDeletedFileChangesCleanupFailed =>
      'Sohbet silindi ancak dosya değişikliği yedekleri temizlenemedi.';

  @override
  String get conversationHistoryClearedFileChangesCleanupFailed =>
      'Sohbet geçmişi temizlendi ancak dosya değişikliği yedekleri silinemedi.';

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
  String get toolPermissionRequireApproval => 'Onay iste';

  @override
  String get selectedModelDoesNotSupportToolCalls =>
      'Bu model araç çağrılarını desteklemiyor. Araç erişimi ayarı bu modelde etkili olmaz.';

  @override
  String get selectedModelToolSupportUnknown =>
      'Bu model araç çağrısı desteğini bildirmiyor. OpenChat araçları göndermeyi deneyecek; sağlayıcı isteği reddedebilir.';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Her dosya, web ve terminal çağrısından önce onay ister. Dosya erişimi proje ve OpenChat klasörleriyle sınırlıdır.';

  @override
  String get toolPermissionApproveSafeOperations => 'Benim için onayla';

  @override
  String get toolPermissionApproveSafeOperationsDescription =>
      'Proje ve OpenChat\'te salt okunur dosya işlemlerini otomatik yapar; değişiklik, web ve terminal için sorar.';

  @override
  String get toolPermissionFullAccess => 'Tam erişim';

  @override
  String get toolPermissionFullAccessDescription =>
      'Onay sormaz; dosya araçları her klasöre erişir, web ve terminal araçları onaysız çalışır.';

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
  String get toolPermissionRequestExpired =>
      'Bu araç izni isteği artık etkin değil.';

  @override
  String get toolPermissionResponseFailed =>
      'Seçimin gönderilemedi. İsteği yeniden deneyebilirsin.';

  @override
  String get toolPermissionContent => 'Yazılacak içerik';

  @override
  String get toolPermissionOldText => 'Bulunacak metin';

  @override
  String get toolPermissionNewText => 'Yerine yazılacak metin';

  @override
  String get toolPermissionQuery => 'Arama metni';

  @override
  String get toolPermissionOffset => 'Başlangıç noktası';

  @override
  String get toolPermissionLimit => 'En çok sonuç';

  @override
  String get toolPermissionStartLine => 'Başlangıç satırı';

  @override
  String get toolPermissionLineCount => 'Satır sayısı';

  @override
  String get toolPermissionIncludeHidden => 'Gizli dosyaları dahil et';

  @override
  String get commonYes => 'Evet';

  @override
  String get commonNo => 'Hayır';

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
  String responseVersionCount(int current, int total) {
    return 'Yanıt $current / $total';
  }

  @override
  String get previousResponseVersion => 'Önceki yanıt';

  @override
  String get nextResponseVersion => 'Sonraki yanıt';

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
  String get providerToolRequestRejected =>
      'Sağlayıcı, araç içeren isteği reddetti. Modelin araç desteğini kontrol et veya araç kullanabildiği belirtilen bir model seç.';

  @override
  String get providerNetworkUnavailable =>
      'Sağlayıcıya ulaşılamadı. Bağlantını kontrol edip yeniden dene.';

  @override
  String get contextWindowExceeded =>
      'Sohbet bu modelin bağlam sınırı için fazla büyük. Son mesajı kısalt veya daha geniş bağlamlı bir model seç. Sohbet geçmişin kayıtlı.';

  @override
  String get openCodeFreeTierRestricted =>
      'OpenCode ücretsiz modelleri yalnızca OpenCode uygulamasında kullanılabilir.';

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
  String get usageMonthly => 'Aylık';

  @override
  String workspaceNumbered(int number) {
    return 'Çalışma alanı $number';
  }

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
  String get resetCreditAvailableStatus => 'Kullanılabilir';

  @override
  String get useResetCredit => 'Hakkı kullan';

  @override
  String get resetCreditRedeeming => 'Kullanılıyor…';

  @override
  String get confirmResetCreditTitle => 'Sıfırlama hakkı kullanılsın mı?';

  @override
  String get confirmResetCreditMessage =>
      'Bir sıfırlama hakkı gönderilecek. Bu işlem geri alınamaz. Devam etmek istiyor musun?';

  @override
  String get confirmResetCreditAction => 'Hakkı kullan';

  @override
  String get resetCreditApplied => 'Kullanım sınırı sıfırlandı.';

  @override
  String get resetCreditAlreadyUsed => 'Bu sıfırlama hakkı zaten kullanılmış.';

  @override
  String get resetCreditNothingToReset =>
      'Şu anda sıfırlanacak bir kullanım sınırı yok.';

  @override
  String get resetCreditNoLongerAvailable =>
      'Bu sıfırlama hakkı artık kullanılabilir değil. Kullanım bilgilerini yenile.';

  @override
  String get resetCreditOutcomeUnknown =>
      'İşlemin sonucu doğrulanamadı. Aynı hakkı yeniden kullanmadan önce kullanım bilgilerini yenile.';

  @override
  String get resetCreditRefreshRequired =>
      'Son isteğin sonucu doğrulanamadı. Yeniden denemeden önce kullanım bilgilerini yenile.';

  @override
  String get resetCreditRejected =>
      'ChatGPT bu hesapta sıfırlama isteğini kabul etmedi.';

  @override
  String get resetCreditSignInRequired =>
      'ChatGPT hesabında yeniden oturum açıp tekrar dene.';

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
  String get conversationWidth => 'Yanıt genişliği';

  @override
  String get conversationWidthDescription =>
      'Sohbet yanıtlarının satır genişliğini seçin.';

  @override
  String get widthNarrow => 'Dar';

  @override
  String get widthNormal => 'Normal';

  @override
  String get widthWide => 'Geniş';

  @override
  String get conversationTextSize => 'Yazı boyutu';

  @override
  String get conversationTextSizeDescription =>
      'Uygulamadaki yazıların boyutunu seçin.';

  @override
  String get textSizeSmall => 'Küçük';

  @override
  String get textSizeNormal => 'Normal';

  @override
  String get textSizeLarge => 'Büyük';

  @override
  String get appFont => 'Yanıt yazı tipi';

  @override
  String get appFontDescription =>
      'Asistan yanıtlarında kullanılacak yazı tipini seçin.';

  @override
  String get appearancePreferenceSaveFailed =>
      'Görünüm tercihi kaydedilemedi. Lütfen tekrar deneyin.';

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
  String get spanishLanguage => 'İspanyolca';

  @override
  String get germanLanguage => 'Almanca';

  @override
  String get frenchLanguage => 'Fransızca';

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
  String get conversationArchiveTitle => 'Sohbet arşivleri';

  @override
  String get conversationArchiveDescription =>
      'Bu cihazdaki seçili sohbetleri şifreli arşiv olarak dışa aktarın veya geri yükleyin.';

  @override
  String get conversationArchiveIncludesNotice =>
      'Seçili mesajlar, araç girdileri ve çıktıları, akıl yürütme özetleri, sohbet başına bellek ayarları ve ekli dosyalar arşivlenir. Sohbet metinlerinde hassas bilgiler veya yerel dosya yolları bulunabilir. Sağlayıcı kimlik bilgileri, bağlı hesaplar, proje bağlantıları ve genel ayarlar arşivlenmez.';

  @override
  String get conversationArchiveUnavailable =>
      'Yerel arşiv servisi veya sohbet geçmişi kullanılamıyor.';

  @override
  String get exportConversations => 'Sohbetleri dışa aktar';

  @override
  String get importConversations => 'Arşivi içe aktar';

  @override
  String get conversationArchiveNoConversations =>
      'Dışa aktarılacak sohbet yok.';

  @override
  String get conversationArchiveSelectTitle =>
      'Dışa aktarılacak sohbetleri seç';

  @override
  String get conversationArchiveSearch => 'Sohbetlerde ara';

  @override
  String conversationArchiveSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# sohbet seçildi',
      one: '# sohbet seçildi',
      zero: 'Sohbet seçilmedi',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchiveSelectAll => 'Görünenlerin tümünü seç';

  @override
  String get conversationArchiveDeselectAll => 'Görünenlerin seçimini kaldır';

  @override
  String get conversationArchiveNoMatches => 'Eşleşen sohbet yok.';

  @override
  String get conversationArchivePassphrase => 'Arşiv parolası';

  @override
  String get conversationArchiveConfirmPassphrase => 'Parolayı doğrula';

  @override
  String get conversationArchivePassphraseHint => 'En az 12 karakter';

  @override
  String get conversationArchivePassphraseTooShort =>
      'En az 12 karakter kullanın.';

  @override
  String get conversationArchivePassphraseTooLong =>
      'Parola en fazla 512 bayt olmalıdır.';

  @override
  String get conversationArchivePassphraseMismatch => 'Parolalar eşleşmiyor.';

  @override
  String get conversationArchivePassphraseRecovery =>
      'Bu parolayı güvenli bir yerde saklayın. OpenChat parolayı kurtaramaz.';

  @override
  String get conversationArchiveChooseFolder =>
      'Şifreli arşivin kaydedileceği klasörü seçin';

  @override
  String get conversationArchiveChooseFile =>
      'Bir OpenChat sohbet arşivi seçin';

  @override
  String conversationArchiveExportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# sohbet için şifreli arşiv oluşturuldu.',
      one: '# sohbet için şifreli arşiv oluşturuldu.',
    );
    return '$_temp0';
  }

  @override
  String conversationArchiveImportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# sohbet içe aktarıldı.',
      one: '# sohbet içe aktarıldı.',
      zero: 'Sohbet içe aktarılmadı.',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchivePickerFailed =>
      'Dosya veya klasör seçici açılamadı.';

  @override
  String get conversationArchiveInvalidFile =>
      'Seçilen arşiv dosyası için geçerli bir yol alınamadı.';

  @override
  String get conversationArchiveInvalidResponse =>
      'Arşiv servisi geçersiz veri döndürdü.';

  @override
  String get conversationArchiveExportFailed => 'Sohbet arşivi oluşturulamadı.';

  @override
  String get conversationArchiveImportFailed =>
      'Sohbet arşivi içe aktarılamadı.';

  @override
  String get profileArchiveTitle => 'Uygulama verilerinin yedeği';

  @override
  String get profileArchiveDescription =>
      'Sohbet veritabanını ve bu sohbetlerde kullanılan ekleri şifreli olarak yedekleyin veya geri yükleyin.';

  @override
  String get profileArchiveIncludesNotice =>
      'Sağlayıcı kimlik bilgileri, genel tercihler, model ve motor dosyaları yedeğe dahil değildir. Geri yükleme sırasında mevcut veritabanı ve ekler kurtarma klasöründe korunur.';

  @override
  String get profileArchiveUnavailable =>
      'Yerel uygulama verisi yedekleme servisi kullanılamıyor.';

  @override
  String get profileArchiveExport => 'Uygulama verilerini yedekle';

  @override
  String get profileArchiveRestore => 'Uygulama verilerini geri yükle';

  @override
  String get profileArchiveChooseFolder =>
      'Şifreli yedeğin kaydedileceği klasörü seçin';

  @override
  String get profileArchiveChooseFile =>
      'Bir OpenChat uygulama verisi yedeği seçin';

  @override
  String profileArchiveExportSuccess(
    String conversationCount,
    String messageCount,
    String attachmentCount,
  ) {
    return 'Şifreli yedek oluşturuldu: $conversationCount sohbet, $messageCount mesaj, $attachmentCount ek.';
  }

  @override
  String get profileArchiveRestoreConfirmTitle =>
      'Uygulama verileri değiştirilsin mi?';

  @override
  String get profileArchiveRestoreConfirmBody =>
      'Seçilen yedek, OpenChat bir sonraki açıldığında sohbet veritabanını ve bu sohbetlerde kullanılan ekleri değiştirir. Mevcut veritabanı ve ekler kurtarma klasöründe saklanır. Sağlayıcı kimlik bilgileri, genel tercihler, model ve motor dosyaları yedekte bulunmaz.';

  @override
  String get profileArchiveRestoreConfirmButton =>
      'Geri yükle ve OpenChat\'i kapat';

  @override
  String get profileArchiveRestartTitle =>
      'Yedeği uygulamak için OpenChat\'i kapatın';

  @override
  String profileArchiveRestoreReady(
    int conversationCount,
    int messageCount,
    int attachmentCount,
  ) {
    return 'Yedek $conversationCount sohbet, $messageCount mesaj ve $attachmentCount ek içeriyor. OpenChat\'i şimdi kapatın. Geri yüklenen veriler açılışta doğrulanacak; mevcut profiliniz kurtarma için saklanacak.';
  }

  @override
  String get profileArchiveCloseApp => 'OpenChat\'i kapat';

  @override
  String get profileArchiveCloseFailed =>
      'Yedek uygulanamadı. OpenChat\'i kapatmak için pencereyi kapatın.';

  @override
  String get profileArchiveProcessing =>
      'Profil yedeği şifreleniyor veya doğrulanıyor. Büyük yedekler birkaç dakika sürebilir.';

  @override
  String get profileArchivePickerFailed =>
      'Dosya veya klasör seçici açılamadı.';

  @override
  String get profileArchiveInvalidFile =>
      'Seçilen yedek için geçerli bir dosya yolu alınamadı.';

  @override
  String get profileArchiveInvalidResponse =>
      'Profil yedekleme servisi geçersiz veri döndürdü.';

  @override
  String get profileArchiveExportFailed =>
      'Şifreli profil yedeği oluşturulamadı.';

  @override
  String get profileArchiveRestoreFailed =>
      'Profil yedeği geri yükleme için hazırlanamadı.';

  @override
  String get profileArchiveInvalidArchive =>
      'Yedek geçersiz veya parola yanlış.';

  @override
  String get profileArchivePassphraseInvalid =>
      'En az 12, en fazla 512 bayt uzunluğunda parola kullanın.';

  @override
  String get profileArchiveNotFound =>
      'Seçilen uygulama verisi yedeği bulunamadı.';

  @override
  String get profileArchiveConflict =>
      'Bekleyen bir profil geri yüklemesi var veya seçilen hedef dosya zaten mevcut.';

  @override
  String get profileArchiveBusy => 'Başka bir profil yedekleme işlemi sürüyor.';

  @override
  String get profileArchiveStorageFailed =>
      'Profil yedeği okunamadı, yazılamadı veya güvenli biçimde geri yüklenemedi.';

  @override
  String get profileArchiveLimitExceeded =>
      'Profil yedeği desteklenen boyut veya öğe sınırını aşıyor.';

  @override
  String get profileArchiveSchemaUnsupported =>
      'Bu yedek daha yeni bir OpenChat veritabanı şemasıyla oluşturulmuş.';

  @override
  String get profileArchiveTakingLong =>
      'Profil yedeği beklenenden uzun sürüyor. Yeniden denemeden önce işlemin bitmesini bekleyin.';

  @override
  String get profileArchiveOperationFailed =>
      'Profil yedekleme işlemi tamamlanamadı.';

  @override
  String get conversationArchivePassphraseTitle => 'Arşivin kilidini aç';

  @override
  String get conversationArchivePreviewTitle => 'Arşiv içeriğini gözden geçir';

  @override
  String get conversationArchiveCreatedAt => 'Oluşturulma';

  @override
  String get conversationArchiveConversationCount => 'Sohbetler';

  @override
  String get conversationArchiveMessageCount => 'Mesajlar';

  @override
  String get conversationArchiveAttachmentCount => 'Ekler';

  @override
  String get conversationArchiveDuplicateCount =>
      'Bu cihazda zaten bulunan sohbetler';

  @override
  String get conversationArchiveSkipDuplicates =>
      'Bu cihazda zaten bulunan sohbetleri atla';

  @override
  String get conversationArchiveImportCopies =>
      'Yinelenenleri ayrı kopyalar olarak içe aktar';

  @override
  String get conversationArchiveRestoreNotice =>
      'Geri yüklenen sohbetlerin sağlayıcı hesapları ve projelerle bağlantısı olmaz. Devam etmeden önce yeniden model seçin.';

  @override
  String get conversationArchiveInvalidPassphraseOrFile =>
      'Parola yanlış veya arşiv geçersiz.';

  @override
  String get conversationArchivePassphraseInvalid =>
      'Parola uzunluğu desteklenmiyor.';

  @override
  String get conversationArchiveNotFound =>
      'Seçilen arşiv veya sohbet bulunamadı.';

  @override
  String get conversationArchiveConflict =>
      'Hedef dosya zaten var. Başka bir klasör seçin veya mevcut dosyayı önce çözümleyin.';

  @override
  String get conversationArchiveBusy =>
      'Sohbetleri dışa aktarmadan önce etkin asistan işlemlerini durdurun.';

  @override
  String get conversationArchiveStorageFailed =>
      'Arşiv güvenli biçimde okunamadı, yazılamadı veya geri yüklenemedi.';

  @override
  String get conversationArchiveLimitExceeded =>
      'Arşiv desteklenen boyut sınırını aşıyor.';

  @override
  String get conversationArchiveTakingLong =>
      'Arşiv işlemi beklenenden uzun sürüyor. İşlem sürüyor olabilir; yeniden denemeden önce sohbet listesini kontrol edin.';

  @override
  String get conversationArchiveOperationFailed =>
      'Arşiv işlemi başarısız oldu. Seçilen dosyayı ve boş disk alanını kontrol edip yeniden deneyin.';

  @override
  String get conversationArchiveProcessing =>
      'Arşiv şifreleniyor veya doğrulanıyor. Büyük arşivler birkaç dakika sürebilir.';

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
      'Yerel sohbet geçmişi açılamadı.';

  @override
  String get historyStorageCorruptDescription =>
      'Yerel veritabanı bozuk görünüyor. Verileri korumak için şema güncellemeleri durduruldu; devam etmek için doğrulanmış yedekten kurtarma gerekiyor.';

  @override
  String get historyStorageBackupFailedDescription =>
      'Güncelleme öncesi veritabanı yedeği doğrulanamadığı için işlem durduruldu. Boş disk alanını kontrol edip yeniden dene.';

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
  String get continueLabel => 'Devam et';

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
  String get searchChatsHint => 'Sohbet ve mesajlarda ara';

  @override
  String get searchMessagesTooltip => 'Mesajları ara';

  @override
  String get historySearchDateFilter => 'Tarihe göre filtrele';

  @override
  String get historySearchDateFilterApplied => 'Tarih filtresi etkin';

  @override
  String get historySearchFiltersTitle => 'Arama filtreleri';

  @override
  String get historySearchRouteFilterNote =>
      'Sağlayıcı ve model, her yanıtla kaydedilmiş rotayı belirtir.';

  @override
  String get historySearchProviderFilter => 'Yanıt sağlayıcısı';

  @override
  String get historySearchModelFilter => 'Yanıt modeli';

  @override
  String get historySearchProjectFilter => 'Proje';

  @override
  String get historySearchArchiveFilter => 'Arşiv durumu';

  @override
  String get historySearchAllProviders => 'Tüm sağlayıcılar';

  @override
  String get historySearchAllModels => 'Tüm modeller';

  @override
  String get historySearchAllProjects => 'Tüm projeler';

  @override
  String get historySearchTagFilter => 'Etiket';

  @override
  String get selectConversations => 'Sohbetleri seç';

  @override
  String get cancelSelection => 'Seçimi iptal et';

  @override
  String get archiveSelectedConversations => 'Seçilen sohbetleri arşivle';

  @override
  String get moveSelectedChats => 'Seçilen sohbetleri taşı';

  @override
  String get moveToChats => 'Sohbetlere taşı';

  @override
  String get bookmarkConversation => 'Sohbeti yer imlerine ekle';

  @override
  String get removeConversationBookmark => 'Yer imini kaldır';

  @override
  String get conversationBookmarkFailed => 'Sohbet yer imi güncellenemedi.';

  @override
  String get conversationBookmarked => 'Yer imlerinde';

  @override
  String selectConversation(String title) {
    return '$title sohbetini seç';
  }

  @override
  String conversationsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# sohbet seçildi',
      one: '# sohbet seçildi',
      zero: 'Sohbet seçilmedi',
    );
    return '$_temp0';
  }

  @override
  String get historySearchAllTags => 'Tüm etiketler';

  @override
  String get historySearchAllStatuses => 'Tüm sohbetler';

  @override
  String get historySearchActiveConversations => 'Etkin sohbetler';

  @override
  String get historySearchArchivedConversations => 'Arşivlenmiş sohbetler';

  @override
  String get historySearchClearFilters => 'Filtreleri temizle';

  @override
  String get historySearchApplyFilters => 'Filtreleri uygula';

  @override
  String get historySearchOpenFilters => 'Arama filtrelerini aç';

  @override
  String get historySearchFiltersActive => 'Arama filtreleri etkin';

  @override
  String get historySearchClearDateFilter => 'Tarih filtresini temizle';

  @override
  String get searchMessagesHeader => 'Mesaj eşleşmeleri';

  @override
  String get searchMessagesLoading => 'Mesajlar aranıyor…';

  @override
  String get searchMessagesNoResults => 'Eşleşen mesaj yok.';

  @override
  String get searchMessagesTooShort =>
      'Mesaj aramak için en az iki karakter girin.';

  @override
  String get searchMessagesTooLong =>
      'Arama metni en fazla 512 karakter olabilir.';

  @override
  String get searchMessagesFailed => 'Mesajlar aranamadı. Yeniden deneyin.';

  @override
  String get searchMessageUnavailable => 'Bu mesaj artık kullanılamıyor.';

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
  String get projectToolRulesTitle => 'Proje araç izinleri';

  @override
  String get projectToolRulesDescription =>
      'Her araç için bir kural seçin. Kural belirlenmeyen araçlar genel izin ayarını kullanır. İzin verme, dosya kapsamı ve işlem yalıtımı sınırlarına uymaya devam eder.';

  @override
  String get projectToolRuleInherit => 'Genel ayarı kullan';

  @override
  String get projectToolRuleAsk => 'Onay iste';

  @override
  String get projectToolRuleAllow => 'İzin ver';

  @override
  String get projectToolRuleDeny => 'Reddet';

  @override
  String get projectToolRulesLoadFailed =>
      'Proje araç izinleri yüklenemedi. Yeni bir istek göndermeden önce kayıtlı ayarları kontrol edin.';

  @override
  String get projectToolRulesSaveFailed =>
      'Proje araç izinleri kaydedilemedi. Önceki kurallar geçerli olmaya devam ediyor.';

  @override
  String get projectOptionsTitle => 'Proje seçenekleri';

  @override
  String get projectOptionsToolPermissions => 'Araç izinleri';

  @override
  String get projectOptionsMcpServers => 'MCP sunucuları';

  @override
  String get projectOptionsWorktrees => 'Git worktree\'leri';

  @override
  String get projectMcpTitle => 'Proje MCP sunucuları';

  @override
  String get projectMcpDescription =>
      'Bu proje için yerel stdio MCP sunucularını yapılandırın. Etkin sunucular yalnızca araç destekli bir sohbet araçlarını istediğinde başlatılır.';

  @override
  String get projectMcpPermissionsLocal =>
      'İzin seçimleri bu proje için yerel olarak saklanır ve depoya yazılmaz.';

  @override
  String get projectMcpPermissionScope => 'Bu sunucunun izni';

  @override
  String get projectMcpNoCredentials =>
      'Kimlik bilgilerini bağımsız değişkenlere yazmayın. Ortam değişkeni adlarını ekleyin, ardından değerleri anahtar düğmesinden güvenli biçimde saklayın.';

  @override
  String get projectMcpEmpty => 'Bu proje için MCP sunucusu yapılandırılmadı.';

  @override
  String get projectMcpAdd => 'Sunucu ekle';

  @override
  String get projectMcpEdit => 'Sunucuyu düzenle';

  @override
  String get projectMcpRemove => 'Sunucuyu kaldır';

  @override
  String get projectMcpServerId => 'Sunucu kimliği';

  @override
  String get projectMcpProgram => 'Programın mutlak yolu';

  @override
  String get projectMcpArguments => 'Argümanlar';

  @override
  String get projectMcpArgumentsHint =>
      'Her satıra bir argüman yazın. Boş satırlar yok sayılır.';

  @override
  String get projectMcpEnabled => 'Etkin';

  @override
  String get projectMcpCheck => 'Bağlantıyı denetle';

  @override
  String get projectMcpChecking => 'Bağlantı denetleniyor…';

  @override
  String projectMcpConnected(int count) {
    return 'Bağlandı; $count araç bulundu';
  }

  @override
  String get projectMcpCheckFailed =>
      'Sunucu başlatılamadı veya araç listesini vermedi.';

  @override
  String get projectMcpLoadFailed =>
      'Proje MCP kataloğu yüklenemedi. `.openchat/mcp.json` dosyasındaki geçersiz veya güvenli olmayan kayıtları kontrol edin.';

  @override
  String get projectMcpSaveFailed =>
      'MCP kataloğu kaydedilemedi. Sunucu kimliklerini ve programların mutlak yollarını kontrol edin.';

  @override
  String get projectMcpDuplicateId =>
      'Her sunucunun kimliği benzersiz olmalıdır.';

  @override
  String get projectMcpInvalidId =>
      '1–24 küçük harf, rakam veya alt çizgi kullanın.';

  @override
  String get projectMcpProgramRequired =>
      'Sunucu programının mutlak yolunu girin.';

  @override
  String get projectMcpTransport => 'Bağlantı türü';

  @override
  String get projectMcpTransportStdio => 'Yerel işlem (stdio)';

  @override
  String get projectMcpTransportHttp => 'Uzak sunucu (Streamable HTTP)';

  @override
  String get projectMcpEndpoint => 'HTTPS uç noktası';

  @override
  String get projectMcpEndpointHint =>
      'Uzak MCP sunucusunun HTTPS uç noktasını kullanın. Özel ve yerel ağ adresleri engellenir.';

  @override
  String get projectMcpEndpointRequired =>
      'Geçerli bir genel HTTPS uç noktası girin.';

  @override
  String get projectMcpAuthVariable => 'Güvenli kimlik bilgisi adı';

  @override
  String get projectMcpAuthVariableHint =>
      'İsteğe bağlıdır. Sunucuyu ekledikten sonra anahtar düğmesiyle değerini kaydedin.';

  @override
  String get projectMcpEnvironmentVariables =>
      'Kimlik bilgisi ortam değişkenleri';

  @override
  String get projectMcpEnvironmentVariablesHint =>
      'Her satıra bir değişken adı yazın. Sunucuyu kaydedin, ardından anahtar düğmesinden değerleri güvenli biçimde saklayın.';

  @override
  String get projectMcpEnvironmentVariablesInvalid =>
      'Harf, rakam ve alt çizgi kullanarak en fazla 32 benzersiz ortam değişkeni adı girin.';

  @override
  String get projectMcpCredentials => 'MCP kimlik bilgileri';

  @override
  String get projectMcpEnvironmentVariablesRequired =>
      'Kimlik bilgilerini saklamadan önce sunucuyu düzenleyip ortam değişkeni adlarını ekleyin.';

  @override
  String get projectMcpCredentialHint =>
      'Değerler Windows Kimlik Bilgisi Yöneticisi\'nde saklanır ve yalnızca bu sunucu sürecine aktarılır.';

  @override
  String get projectMcpSecret => 'Gizli değer';

  @override
  String get projectMcpCredentialStored =>
      'Bir değer güvenli biçimde saklanıyor.';

  @override
  String get projectMcpCredentialMissing => 'Saklanmış değer yok.';

  @override
  String get projectMcpCredentialSave => 'Güvenli sakla';

  @override
  String get projectMcpCredentialRemove => 'Saklanan değeri kaldır';

  @override
  String get projectMcpCredentialUnavailable =>
      'Kimlik bilgisine erişilemedi. Windows Kimlik Bilgisi Yöneticisi\'ni kontrol edip yeniden deneyin.';

  @override
  String get projectMcpSecretRequired => 'Saklamadan önce bir değer girin.';

  @override
  String get projectMcpSecretTooLarge =>
      'Değer 2.500 bayt veya daha küçük olmalıdır.';

  @override
  String get projectWorktreesTitle => 'Yalıtılmış worktree\'ler';

  @override
  String get projectWorktreesDescription =>
      'Mevcut commit\'ten ayrı bir klasörde dal oluşturun. Kaydedilmemiş değişiklikler kopyalanmaz.';

  @override
  String get projectWorktreesLoading => 'Worktree\'ler yükleniyor…';

  @override
  String get projectWorktreesEmpty =>
      'Bu proje için henüz worktree oluşturulmadı.';

  @override
  String get projectWorktreeCreate => 'Worktree oluştur';

  @override
  String get projectWorktreeCreateFailed =>
      'Worktree oluşturulamadı. Klasörün bir Git deposu olduğunu kontrol edin.';

  @override
  String get projectWorktreeLoadFailed => 'Proje worktree\'leri yüklenemedi.';

  @override
  String get projectWorktreeOperationFailed =>
      'Git işlemi başarısız oldu. Depo durumunu kontrol edip yeniden deneyin.';

  @override
  String get projectWorktreeNotRepository =>
      'Bu proje klasörü bir Git deposunun içinde değil.';

  @override
  String projectWorktreeBranch(String branch) {
    return 'Dal: $branch';
  }

  @override
  String projectWorktreePath(String path) {
    return 'Klasör: $path';
  }

  @override
  String get projectWorktreeStatusClean => 'Kaydedilmemiş değişiklik yok';

  @override
  String projectWorktreeStatusChanges(int count) {
    return 'Değişen dosya: $count';
  }

  @override
  String get projectWorktreeReview => 'Değişiklikleri incele';

  @override
  String get projectWorktreeUse => 'Proje olarak kullan';

  @override
  String get projectWorktreeRemove => 'Worktree\'yi kaldır';

  @override
  String get projectWorktreeRemoveTitle => 'Worktree kaldırılsın mı?';

  @override
  String projectWorktreeRemoveDescription(String branch) {
    return '$branch dalındaki kaydedilmemiş ve izlenmeyen dosyalar silinir. Dal ve commit\'leri korunur.';
  }

  @override
  String get projectWorktreeReviewTitle => 'Worktree değişiklikleri';

  @override
  String get projectWorktreeNoChanges =>
      'Kaydedilmemiş değişiklik yok. Commit edilmiş değişiklikler bu dalda kalır.';

  @override
  String get projectWorktreeStagedDiff => 'Hazırlanmış değişiklikler';

  @override
  String get projectWorktreeUnstagedDiff => 'Hazırlanmamış değişiklikler';

  @override
  String get projectWorktreeListTruncated =>
      'Yalnızca ilk 20 worktree gösteriliyor.';

  @override
  String get projectWorktreeCheckFailed => 'Worktree incelenemedi.';

  @override
  String get projectWorktreeRunCheck => 'Kontrolü çalıştır';

  @override
  String get projectWorktreeTaskPickerTitle => 'Adlandırılmış kontrolü seçin';

  @override
  String get projectWorktreeTaskEmpty =>
      'Bu worktree için adlandırılmış kontrol yok. Projede `.openchat/tasks.json` dosyasına görev ekleyin.';

  @override
  String get projectWorktreeTaskLoadFailed =>
      'Adlandırılmış kontroller bu worktree\'den yüklenemedi.';

  @override
  String get projectWorktreeTaskConfirmationTitle =>
      'Bu kontrol çalıştırılsın mı?';

  @override
  String get projectWorktreeTaskCommand => 'Komut';

  @override
  String projectWorktreeTaskTimeout(int seconds) {
    return 'Süre sınırı: $seconds saniye';
  }

  @override
  String get projectWorktreeTaskRun => 'Kontrolü çalıştır';

  @override
  String projectWorktreeTaskRunning(String task) {
    return 'Kontrol çalışıyor: $task';
  }

  @override
  String get projectWorktreeTaskStopping => 'Kontrol durduruluyor…';

  @override
  String get projectWorktreeTaskStop => 'Kontrolü durdur';

  @override
  String get projectWorktreeTaskCancelled => 'Kontrol iptal edildi.';

  @override
  String get projectWorktreeTaskTimedOut => 'Kontrol süre sınırına ulaştı.';

  @override
  String projectWorktreeTaskExitCode(int code) {
    return 'Kontrol $code çıkış koduyla tamamlandı.';
  }

  @override
  String get projectWorktreeTaskExitCodeUnavailable =>
      'Kontrol bir çıkış kodu bildirmeden tamamlandı.';

  @override
  String get projectWorktreeTaskOutputTruncated =>
      'Çıktı ilk 128 KiB ile sınırlandırıldı.';

  @override
  String get projectWorktreeTaskRunFailed =>
      'Kontrol worktree yalıtım alanında çalıştırılamadı.';

  @override
  String projectWorktreeTaskResultTitle(String task) {
    return 'Kontrol sonucu: $task';
  }

  @override
  String get projectWorktreeTaskOutput => 'Çıktı';

  @override
  String get projectWorktreeTaskNoOutput => 'Kontrol çıktı üretmedi.';

  @override
  String get projectWorktreeTaskDenied =>
      'Proje izinleri adlandırılmış kontrolleri çalıştırmayı engelliyor.';

  @override
  String get agentRunManagerTitle => 'Çalışmalar';

  @override
  String get agentRunManagerDescription =>
      'Sohbetlerde devam eden, duraklatılmış veya kesintiye uğramış çalışmaları görüntüleyin.';

  @override
  String get agentRunLoading => 'Çalışmalar yükleniyor';

  @override
  String get agentRunLoadFailed => 'Çalışma durumu yüklenemedi.';

  @override
  String get agentRunEmpty =>
      'Devam eden, duraklatılmış veya kesintiye uğramış çalışma yok.';

  @override
  String get agentRunRefresh => 'Çalışma listesini yenile';

  @override
  String get agentRunOpenConversation => 'Sohbeti aç';

  @override
  String get agentRunStatusRunning => 'Devam ediyor';

  @override
  String get agentRunStatusPaused => 'Duraklatıldı';

  @override
  String get agentRunStatusInterrupted => 'Kesintiye uğradı';

  @override
  String get agentRunStatusUnavailable => 'Durum kullanılamıyor';

  @override
  String get agentRunStatusCompleted => 'Tamamlandı';

  @override
  String get agentRunStatusFailed => 'Başarısız';

  @override
  String get agentRunStatusCancelled => 'İptal edildi';

  @override
  String get agentRunSubagent => 'Alt görev';

  @override
  String agentRunSubagentTask(String objective) {
    return 'Alt görev: $objective';
  }

  @override
  String get agentRunLiveStarting => 'Devredilen analiz başlatılıyor…';

  @override
  String get agentRunLiveThinking => 'Devredilen görev analiz ediliyor…';

  @override
  String agentRunLiveUsingTool(String tool) {
    return '$tool kullanılıyor';
  }

  @override
  String get agentRunEndedInAnotherChat =>
      'Başka bir sohbetteki çalışma sona erdi.';

  @override
  String get newProjectConversation => 'Projede yeni sohbet başlat';

  @override
  String get newConversation => 'Yeni bir sohbet başlat';

  @override
  String get conversationTitle => 'Yeni sohbet';

  @override
  String get conversationMemory => 'Sohbet belleği';

  @override
  String get conversationMemoryDescription =>
      'Bu sohbetin sıkıştırılmış bağlamını incele ve arşivdeki eski mesajlarda ara.';

  @override
  String conversationMemoryCurrentConversation(String title) {
    return 'Seçili sohbet: $title';
  }

  @override
  String get conversationMemoryNoConversation =>
      'Belleği incelemek için önce bir sohbet aç.';

  @override
  String get contextUsageTitle => 'Bağlam kullanımı';

  @override
  String contextUsageUsed(String count) {
    return 'Kullanım: yaklaşık $count token';
  }

  @override
  String contextUsageSummary(String used, String limit, String percent) {
    return 'Yaklaşık $used / $limit token ($percent)';
  }

  @override
  String contextUsageModelLimit(String count) {
    return '$count token';
  }

  @override
  String get contextUsageNoModelLimit => 'Model sınırı bilinmiyor.';

  @override
  String contextUsageProviderMeasurement(String count) {
    return 'Son sağlayıcı ölçümü: $count token';
  }

  @override
  String contextUsageInstructionsEstimate(String count, String percent) {
    return 'Talimatlar: $count token · $percent';
  }

  @override
  String contextUsageToolDefinitionsEstimate(String count, String percent) {
    return 'Araç tanımları: $count token · $percent';
  }

  @override
  String contextUsageMessagesEstimate(String count, String percent) {
    return 'Mesajlar: $count token · $percent';
  }

  @override
  String contextUsageAttachmentsEstimate(String count, String percent) {
    return 'Ek dosyalar: $count token · $percent';
  }

  @override
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count token · $percent';
  }

  @override
  String contextUsageDraftAttachment(String name) {
    return 'Taslak · $name';
  }

  @override
  String contextUsageUserMessagesEstimate(String count, String percent) {
    return 'Kullanıcı: $count token · $percent';
  }

  @override
  String contextUsageAssistantMessagesEstimate(String count, String percent) {
    return 'Yapay zekâ: $count token · $percent';
  }

  @override
  String contextUsageToolsEstimate(String count, String percent) {
    return 'Araç kullanımı: $count token · $percent';
  }

  @override
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count token · $percent';
  }

  @override
  String contextUsageMemoryEstimate(String count, String percent) {
    return 'Sıkıştırılmış bellek: $count token · $percent';
  }

  @override
  String contextUsageDraftEstimate(String count, String percent) {
    return 'Taslak: $count token · $percent';
  }

  @override
  String contextUsageFreeSpaceEstimate(String count, String percent) {
    return 'Boş alan: $count token · $percent';
  }

  @override
  String get contextUsageOverLimit => 'Model sınırı aşılıyor.';

  @override
  String get contextUsageMeasurementUnavailable =>
      'Bellek ayrıntıları alınamadı.';

  @override
  String get contextUsageInstructionUnavailable => 'Talimat bilgisi alınamadı.';

  @override
  String get contextUsageConfigurationUnavailable =>
      'Talimat ve araç tanımı ölçümleri alınamadı.';

  @override
  String get contextUsageConfigurationLoading =>
      'Talimat ve araç tanımı ölçümleri hazırlanıyor…';

  @override
  String get conversationMemorySemanticTitle => 'Anlamsal arama';

  @override
  String get conversationMemorySemanticDescription =>
      'Farklı ifadelerle yazılmış eski mesajları da bulmak için yaklaşık 136 MB çok dilli model indir.';

  @override
  String get conversationMemorySemanticPrepare => 'Hazırla';

  @override
  String get conversationMemorySemanticChecking =>
      'Yerel anlamsal arama durumu denetleniyor…';

  @override
  String get conversationMemorySemanticPreparing =>
      'Model indiriliyor ve doğrulanıyor…';

  @override
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  ) {
    return '%$percent indirildi · $downloaded / $total MB';
  }

  @override
  String get conversationMemorySemanticIndexing =>
      'Yerel arşiv indeksi hazırlanıyor…';

  @override
  String get conversationMemorySemanticCancelling => 'İndirme iptal ediliyor…';

  @override
  String get conversationMemorySemanticDownloadCancelled =>
      'İndirme iptal edildi. Anahtar kelime araması kullanılabilir.';

  @override
  String get conversationMemorySemanticPrepareFailed =>
      'Model hazırlanamadı. Yeniden dene; doğrulanan indirme parçaları yeniden kullanılır.';

  @override
  String get conversationMemorySemanticKeywordSearchFallback =>
      'Hazırlamadan önce anahtar kelime araması kullanılabilir.';

  @override
  String get conversationMemorySemanticReady => 'Yerel anlamsal arama hazır';

  @override
  String get conversationMemorySemanticIndexNotice =>
      'Model bu cihazda çalışır. İlk arama eski mesajları ve kayıtlı araç ayrıntılarını yerel olarak indeksleyebilir ve biraz sürebilir.';

  @override
  String get conversationMemorySummaryTitle => 'Sıkıştırılmış bağlam';

  @override
  String get conversationMemoryNoSummary => 'Henüz sıkıştırılmış bir özet yok.';

  @override
  String get conversationMemoryCheckpointDescription =>
      'Sağlayıcı bağlamı okunabilir bir özet yerine yeniden kullanılabilir bir paket olarak saklıyor. Tam mesaj geçmişi arşivde korunuyor.';

  @override
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  ) {
    return 'Son istek · $provider · $model · $count girdi tokeni';
  }

  @override
  String get conversationMemorySearchTitle => 'Arşivde ara';

  @override
  String get conversationMemorySearchHint => 'Eski bir konu veya ifade yaz...';

  @override
  String get conversationMemorySearchAction => 'Ara';

  @override
  String get conversationMemorySearchQueryTooShort =>
      'Arama için en az iki karakter yaz.';

  @override
  String get conversationMemorySearchInstruction =>
      'Tamamlanmış mesajlar ve araç sonuçları yalnızca bu sohbet içinde aranır.';

  @override
  String get conversationMemoryArchiveSettingsTitle => 'Arşiv indeksleme';

  @override
  String get conversationMemoryArchiveSettingsDescription =>
      'Yerel arşiv aramasına hangi kayıtlı sohbet metinlerinin ekleneceğini seç. Bir öğeyi kapatırsan ona ait arama ve anlamsal indeks verileri silinir; asıl sohbet kayıtlı kalır.';

  @override
  String get conversationMemoryArchiveIncludeConversation =>
      'Bu sohbeti dahil et';

  @override
  String get conversationMemoryArchiveConversationIncluded =>
      'Mesajlar ve dahil edilen araç sonuçları arşiv aramasında yer alabilir.';

  @override
  String get conversationMemoryArchiveConversationExcluded =>
      'Bu sohbetin türetilmiş arşiv indeksleri silinir ve yeni indekslemeye alınmaz.';

  @override
  String get conversationMemoryArchiveToolsTitle => 'Araç sonuçları';

  @override
  String get conversationMemoryArchiveToolIncluded =>
      'Bu aracın kayıtlı ayrıntıları arşiv aramasında yer alabilir.';

  @override
  String get conversationMemoryArchiveToolExcluded =>
      'Bu aracın kayıtlı ayrıntıları arşiv indekslerinden silinir.';

  @override
  String get conversationMemoryArchiveNoTools =>
      'Bu sohbette tamamlanmış araç sonucu yok.';

  @override
  String get conversationMemoryArchiveSettingsSaveFailed =>
      'Arşiv indeksleme ayarları kaydedilemedi. Yeniden dene.';

  @override
  String get conversationMemorySearchNoResults => 'Eşleşen arşiv kaydı yok.';

  @override
  String get conversationMemorySearchFailed =>
      'Sohbet arşivi aranamadı. Yeniden deneyin.';

  @override
  String get conversationMemoryLoadFailed =>
      'Sıkıştırılmış bağlam yüklenemedi. Yeniden deneyin.';

  @override
  String get conversationMemoryResetAction => 'Bağlamı sıfırla';

  @override
  String get conversationMemoryResetTitle =>
      'Sıkıştırılmış bağlam sıfırlansın mı?';

  @override
  String get conversationMemoryResetConfirmation =>
      'Kaydedilmiş sıkıştırılmış bağlam ve son istek ölçümü kaldırılır. Tam mesaj geçmişi ve arşiv korunur; gerektiğinde bağlam sonraki istek sırasında yeniden oluşturulur.';

  @override
  String get conversationMemoryResetConfirm => 'Sıfırla';

  @override
  String get conversationMemoryResetFailed =>
      'Sıkıştırılmış bağlam sıfırlanamadı.';

  @override
  String get conversationMemoryUserMessage => 'Kullanıcı mesajı';

  @override
  String get conversationMemoryAssistantMessage => 'Asistan yanıtı';

  @override
  String get renameConversation => 'Sohbet başlığını düzenle';

  @override
  String get editConversationTags => 'Etiketleri düzenle';

  @override
  String get conversationTagsDialogTitle => 'Sohbet etiketleri';

  @override
  String get conversationTagsFieldLabel => 'Etiketler';

  @override
  String get conversationTagsFieldHint => 'Etiketleri virgülle ayırın';

  @override
  String get conversationTagsHelp =>
      'En fazla 12 etiket ekleyin; her etiket 32 karakter olabilir.';

  @override
  String get conversationTagsSaveFailed => 'Etiketler kaydedilemedi.';

  @override
  String get saveHistorySearchTitle => 'Geçmiş aramasını kaydet';

  @override
  String get savedHistorySearchName => 'Arama adı';

  @override
  String get savedHistorySearchesTitle => 'Kayıtlı aramalar';

  @override
  String get savedHistorySearchesEmpty => 'Henüz kayıtlı arama yok.';

  @override
  String get deleteSavedHistorySearch => 'Kayıtlı aramayı sil';

  @override
  String get saveCurrentHistorySearch => 'Geçerli aramayı kaydet';

  @override
  String get savedHistorySearchesLoadFailed => 'Kayıtlı aramalar yüklenemedi.';

  @override
  String get savedHistorySearchSaveFailed => 'Kayıtlı aramalar güncellenemedi.';

  @override
  String get savedHistorySearchLimitReached =>
      'En fazla 20 arama kaydedebilirsiniz.';

  @override
  String get pinConversation => 'Sohbeti sabitle';

  @override
  String get unpinConversation => 'Sabitlemeyi kaldır';

  @override
  String get conversationTitleRequired => 'Sohbet başlığı boş olamaz.';

  @override
  String get conversationBranchEditTitle => 'Mesajı düzenle ve yeni dal başlat';

  @override
  String get conversationBranchEditLabel => 'Mesaj';

  @override
  String get conversationBranchStart => 'Dal başlat';

  @override
  String conversationBranchTitle(String title) {
    return '$title (dal)';
  }

  @override
  String get conversationBranchCreateFailed => 'Mesaj dalı oluşturulamadı.';

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
      'Sohbet başlatmak için bir sağlayıcı bağlayıp model seç.';

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
  String get reasoningDefault => 'Varsayılan';

  @override
  String get reasoningDefaultHint =>
      'Özel bir düzey gönderilmez; sağlayıcının varsayılan akıl yürütme davranışı kullanılır. Görev zorluğuna göre seviye seçmez.';

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
  String get providerDataUnavailable =>
      'Sağlayıcıdan alınan veri okunamadı. Bağlantıyı yenileyip tekrar dene.';

  @override
  String get oauthResponseInvalid =>
      'Oturum açma sonucu okunamadı. Yeniden bağlanmayı dene.';

  @override
  String get modelCatalogUnavailable =>
      'Model listesi kullanılamıyor. Sağlayıcı bağlantısını yenileyip tekrar dene.';

  @override
  String get messageSaveFailed =>
      'Mesajın yerel sohbet geçmişine kaydedilemedi.';

  @override
  String get chatHistoryUnavailable =>
      'Sohbet geçmişi güncellenemedi. Lütfen tekrar dene.';

  @override
  String get chatRequestFailed =>
      'Yanıt tamamlanamadı. Kayıtlı mesajların kullanılabilir durumda.';

  @override
  String get goalAlreadyActive =>
      'Bu sohbette yeni bir hedef başlatmadan önce etkin hedefi sürdür veya durdur.';

  @override
  String get goalStateUnavailable =>
      'Bu sohbette etkin hedef olup olmadığı doğrulanamadı. Lütfen tekrar dene.';

  @override
  String get cachedCatalog => 'önbellekteki modeller';

  @override
  String get attachFile => 'Dosya ekle';

  @override
  String get attachmentsUnavailable => 'Dosya ekleme henüz kullanılamıyor.';

  @override
  String get removeAttachment => 'Ek dosyayı kaldır';

  @override
  String get previewImage => 'Görseli büyüt';

  @override
  String get attachmentUnavailable => 'Ek dosyaya erişilemiyor.';

  @override
  String get attachmentCountExceeded =>
      'En fazla 10 dosya ve 3 görsel ekleyebilirsin.';

  @override
  String get attachmentFileTooLarge =>
      'Dosya izin verilen boyut sınırını aşıyor.';

  @override
  String get attachmentTotalTooLarge =>
      'Ek dosyaların toplam boyutu 14 MB\'ı aşamaz.';

  @override
  String get unsupportedAttachmentFile => 'Bu dosya türü desteklenmiyor.';

  @override
  String get attachmentReadFailed =>
      'Dosya okunamadı. Dosyayı yeniden seçip tekrar dene.';

  @override
  String get attachmentSaveFailed =>
      'Ek dosya yerel sohbete kaydedilemedi. Tekrar seçip dene.';

  @override
  String get attachmentMustBeUtf8 => 'Metin dosyası UTF-8 biçiminde olmalıdır.';

  @override
  String get attachmentInvalidImage =>
      'Görsel dosyasının biçimi doğrulanamadı.';

  @override
  String get modelDoesNotSupportImages =>
      'Seçili model görsel eklerini desteklemiyor.';

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
  String get selectedModelUnavailable =>
      'Bu model artık kullanılamıyor. Başka bir model seç.';

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
  String responseTokenRate(String rate) {
    return '$rate t/s';
  }

  @override
  String responseTokenCount(String count) {
    return '$count token';
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
  String get toolWaitingForUser => 'Yanıtın bekleniyor';

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
  String get toolWriteFile => 'Dosyaya yaz';

  @override
  String get toolEditFile => 'Dosyayı düzenle';

  @override
  String get toolExecuteCommand => 'Komut çalıştır';

  @override
  String get toolRunProjectTask => 'Proje görevini çalıştır';

  @override
  String toolConfiguredProjectTool(String name) {
    return 'Proje aracı: $name';
  }

  @override
  String get toolDelegateTask => 'Analizi alt göreve devret';

  @override
  String get toolPermissionTask => 'Görev';

  @override
  String get toolPermissionTimeout => 'Zaman aşımı (saniye)';

  @override
  String get toolSendTerminalInput => 'Terminale girdi gönder';

  @override
  String get toolGitStatus => 'Git durumu';

  @override
  String get toolGitDiff => 'Git farkı';

  @override
  String get toolGitHistory => 'Git geçmişi';

  @override
  String get toolGitBranch => 'Dal';

  @override
  String get toolGitUpstream => 'Üst dal';

  @override
  String get toolGitAhead => 'İleride';

  @override
  String get toolGitBehind => 'Geride';

  @override
  String get toolGitStaged => 'Hazırlanmış';

  @override
  String get toolGitUnstaged => 'Hazırlanmamış';

  @override
  String get toolGitNoChanges => 'Çalışma ağacı temiz.';

  @override
  String get toolGitNoDiff => 'Gösterilecek fark yok.';

  @override
  String get toolGitNoHistory => 'Commit bulunamadı.';

  @override
  String get toolWebSearch => 'Web\'de ara';

  @override
  String get toolReadUrlContent => 'Web sayfasını oku';

  @override
  String get toolSearchQuery => 'Arama sorgusu';

  @override
  String get toolLocalWebSource => 'OpenChat web araması';

  @override
  String get toolLocalPageSource => 'OpenChat sayfa okuması';

  @override
  String get toolProviderSource => 'Sağlayıcı kaynağı';

  @override
  String toolSourceRetrievedAt(String time) {
    return 'Alınma zamanı: $time';
  }

  @override
  String get toolSourceDetails => 'Kaynak ayrıntıları';

  @override
  String toolCitationSource(String sourceId) {
    return 'Kaynak $sourceId';
  }

  @override
  String get toolWebSearchNoResults => 'Web sonucu bulunamadı.';

  @override
  String toolWebSearchResultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count sonuç',
      one: '1 sonuç',
      zero: '0 sonuç',
    );
    return '$_temp0';
  }

  @override
  String get toolUrl => 'URL';

  @override
  String toolReadUrlLength(int count) {
    return '$count karakter';
  }

  @override
  String get toolOpenUrl => 'Tarayıcıda aç';

  @override
  String get toolCopyUrl => 'URL\'yi kopyala';

  @override
  String get toolCopyContent => 'İçeriği kopyala';

  @override
  String get toolCopyFailed => 'İçerik kopyalanamadı.';

  @override
  String get toolOperationWorking => 'İşlem sürüyor.';

  @override
  String get toolOperationFailed => 'İşlem tamamlanamadı.';

  @override
  String get toolOperationUnavailable => 'Sonuç görüntülenemiyor.';

  @override
  String get toolOperationTruncated =>
      'Sonucun yalnızca bir bölümü gösterilebiliyor.';

  @override
  String get toolSearchNoMatches => 'Eşleşme bulunamadı.';

  @override
  String get toolSearchMoreResults => 'Daha fazla eşleşme var.';

  @override
  String toolSearchMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count eşleşme',
      one: '1 eşleşme',
      zero: '0 eşleşme',
    );
    return '$_temp0';
  }

  @override
  String get toolReadNoLines => 'Bu aralıkta satır yok.';

  @override
  String get toolReadMoreLines => 'Daha fazla satır var.';

  @override
  String toolReadLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count satır',
      one: '1 satır',
      zero: '0 satır',
    );
    return '$_temp0';
  }

  @override
  String get toolFileTypeFile => 'Dosya';

  @override
  String get toolFileTypeDirectory => 'Klasör';

  @override
  String toolWriteSuccess(String size) {
    return '$size yazıldı';
  }

  @override
  String get toolFilePreview => 'Yazılan içerik önizlemesi';

  @override
  String toolEditSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# değişiklik uygulandı',
      one: '# değişiklik uygulandı',
    );
    return '$_temp0';
  }

  @override
  String get toolEditBefore => 'Önce';

  @override
  String get toolEditAfter => 'Sonra';

  @override
  String get toolPermissionCommand => 'Komut';

  @override
  String get toolPermissionTerminalId => 'Terminal kimliği';

  @override
  String get toolPermissionInput => 'Terminal girdisi';

  @override
  String get toolTerminalNoOutput => 'Çıktı üretilmedi.';

  @override
  String get toolTerminalWaitingOutput => 'Çıktı veya girdi bekleniyor...';

  @override
  String get toolTerminalRunning => 'Çalışıyor...';

  @override
  String get toolTerminalTerminated => 'Sonlandırıldı';

  @override
  String get toolTerminalWaitingForInput => 'Girdi bekleniyor';

  @override
  String toolTerminalExitCode(int code) {
    return 'Çıkış kodu: $code';
  }

  @override
  String get toolTerminalCopied => 'Panoya kopyalandı';

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
  String get toolOpenChatLocation => 'Uygulama veri klasörü';

  @override
  String get responseFailed => 'Yanıt alınamadı';

  @override
  String get responseStopped => 'Yanıt durduruldu';

  @override
  String secondsShort(int count) {
    return '$count sn';
  }

  @override
  String get usageQuotas => 'Kullanım Kotaları';

  @override
  String get usageQuotasDescription =>
      'Tüm bağlı ChatGPT hesaplarınızın kalan kotalarını, kullanım limitlerini ve sıfırlanma zamanlarını görüntüleyin.';

  @override
  String get statistics => 'İstatistikler';

  @override
  String get statisticsDescription =>
      'Sağlayıcı, model ve sohbet bazında token kullanımını, istekleri ve ChatGPT kota geçmişini inceleyin.';

  @override
  String get statisticsLoadFailed => 'Kullanım istatistikleri yüklenemedi.';

  @override
  String get statisticsUnavailable =>
      'Yerel kullanım istatistikleri şu anda kullanılamıyor.';

  @override
  String get statisticsRetry => 'Yeniden yükle';

  @override
  String get statisticsDateRange => 'Tarih aralığı';

  @override
  String get statisticsProvider => 'Sağlayıcı';

  @override
  String get statisticsRunId => 'Çalışma kimliği';

  @override
  String statisticsRunIdValue(String runId) {
    return 'Çalışma kimliği: $runId';
  }

  @override
  String get statisticsModel => 'Model';

  @override
  String get statisticsOperation => 'İşlem türü';

  @override
  String get statisticsReasoningEffort => 'Reasoning düzeyi';

  @override
  String get statisticsFastModeFilter => 'Fast modu';

  @override
  String get statisticsAll => 'Tümü';

  @override
  String get statisticsUnspecified => 'Belirtilmemiş';

  @override
  String get statisticsClearFilters => 'Filtreleri temizle';

  @override
  String get statisticsTotalTokens => 'Toplam token';

  @override
  String get statisticsInputTokens => 'Giriş tokenı';

  @override
  String get statisticsOutputTokens => 'Çıkış tokenı';

  @override
  String get statisticsReasoningTokens => 'Reasoning tokenı';

  @override
  String get statisticsCachedInputTokens => 'Önbellekten okunan token';

  @override
  String get statisticsCacheWriteTokens => 'Önbelleğe yazılan token';

  @override
  String get statisticsRequests => 'İstek';

  @override
  String get statisticsSuccessfulRequests => 'Başarılı';

  @override
  String get statisticsFailedRequests => 'Başarısız';

  @override
  String get statisticsCancelledRequests => 'Durdurulan';

  @override
  String get statisticsInterruptedRequests => 'Yarım kalan';

  @override
  String get statisticsPendingRequests => 'Devam ediyor';

  @override
  String get statisticsConversations => 'Sohbet';

  @override
  String get statisticsCoverage => 'Kullanım verisi kapsamı';

  @override
  String get statisticsInputCoverage => 'Giriş tokenı bildirilen istekler';

  @override
  String get statisticsOutputCoverage => 'Çıkış tokenı bildirilen istekler';

  @override
  String get statisticsReasoningCoverage =>
      'Reasoning tokenı bildirilen istekler';

  @override
  String statisticsCoverageText(int total, int reported) {
    return '$total isteğin $reported tanesinde sağlayıcı token verisi döndürdü.';
  }

  @override
  String get statisticsProviderReportedCost => 'Sağlayıcının bildirdiği tutar';

  @override
  String statisticsCostCoverage(int reported) {
    return 'Maliyet bilgisi $reported istekte mevcut.';
  }

  @override
  String get statisticsModelsDevCatalogCost =>
      'models.dev liste fiyatına göre hesaplanan tutar';

  @override
  String statisticsModelsDevCatalogCostCoverage(int priced) {
    return '$priced istek için liste fiyatı karşılığı hesaplandı.';
  }

  @override
  String statisticsModelsDevPricingCurrent(String date) {
    return 'models.dev fiyat kataloğu $date tarihinde alındı.';
  }

  @override
  String statisticsModelsDevPricingStale(String date) {
    return 'models.dev erişilemiyor; $date tarihli önbellekteki fiyatlar kullanılıyor.';
  }

  @override
  String get statisticsModelsDevPricingUnavailable =>
      'models.dev fiyat kataloğuna erişilemiyor. Sağlayıcı ve model birebir eşleşmeyen kullanımlar için maliyet hesaplanmıyor.';

  @override
  String get statisticsUsageTrend => 'Zamana göre kullanım';

  @override
  String get statisticsDaily => 'Günlük';

  @override
  String get statisticsMonthly => 'Aylık';

  @override
  String get statisticsProviders => 'Sağlayıcı kullanımı';

  @override
  String get statisticsModels => 'Model kullanımı';

  @override
  String get statisticsReasoningLevels => 'Seçilen reasoning düzeyleri';

  @override
  String get statisticsOperations => 'İşlem türleri';

  @override
  String get statisticsFastModeUsage => 'Fast modu kullanımı';

  @override
  String get statisticsRequested => 'İstendi';

  @override
  String get statisticsNotRequested => 'İstenmedi';

  @override
  String get statisticsServiceTiers => 'Dönen hizmet katmanları';

  @override
  String get statisticsServiceTier => 'Hizmet katmanı';

  @override
  String get statisticsNoBreakdownData => 'Bu aralıkta gösterilecek veri yok.';

  @override
  String get statisticsNoConversationData =>
      'Bu aralıkta sohbet kullanımı yok.';

  @override
  String get statisticsOpenConversation => 'Sohbeti aç';

  @override
  String get statisticsRequestDetails => 'İstek geçmişi';

  @override
  String get statisticsRequestPayload => 'Gönderilen istek verisi';

  @override
  String get statisticsRequestContextUnavailable =>
      'Gönderilen istek özeti kullanılamıyor.';

  @override
  String statisticsRequestSourceMessages(String ids) {
    return 'Dahil edilen ileti kimlikleri: $ids';
  }

  @override
  String statisticsRequestArchivedMessages(String ids) {
    return 'Getirilen ileti kimlikleri: $ids';
  }

  @override
  String statisticsRequestSummaryBoundary(String id) {
    return 'Özetin kapsadığı son ileti: $id';
  }

  @override
  String statisticsRequestSourceAttachments(String files) {
    return 'Gönderilen ekler: $files';
  }

  @override
  String get statisticsRequestSourcesTruncated =>
      'Bu kayıtta bazı kaynak ayrıntıları gösterilmiyor.';

  @override
  String statisticsRequestMessageCount(int count) {
    return 'Gönderilen mesaj: $count';
  }

  @override
  String statisticsRequestImageCount(int count) {
    return 'Gönderilen görsel: $count';
  }

  @override
  String statisticsRequestToolResultCount(int count) {
    return 'Gönderilen araç sonucu: $count';
  }

  @override
  String statisticsRequestInstructionBytes(int count) {
    return 'Talimat boyutu: $count bayt';
  }

  @override
  String statisticsRequestRoles(String roles) {
    return 'Mesaj rolleri: $roles';
  }

  @override
  String statisticsRequestTools(String names) {
    return 'Araç şemaları: $names';
  }

  @override
  String statisticsRequestCacheControls(String names) {
    return 'Cache kontrolleri: $names';
  }

  @override
  String get statisticsNone => 'Yok';

  @override
  String get statisticsRequestTime => 'İstek zamanı';

  @override
  String get statisticsConversationTitle => 'Sohbet başlığı';

  @override
  String get statisticsStatus => 'Durum';

  @override
  String get statisticsUsageSource => 'Kullanım verisi kaynağı';

  @override
  String get statisticsNoRequestData => 'Bu filtrelerle eşleşen istek yok.';

  @override
  String statisticsShowingRows(int start, int end, int total) {
    return '$start - $end / $total';
  }

  @override
  String get statisticsExportCsv => 'CSV dışa aktar';

  @override
  String get statisticsExporting => 'Dışa aktarılıyor';

  @override
  String get statisticsExported => 'İstatistikler CSV dosyasına aktarıldı.';

  @override
  String get statisticsExportFailed => 'İstatistikler dışa aktarılamadı.';

  @override
  String get statisticsQuotaHistory => 'ChatGPT kota geçmişi';

  @override
  String get statisticsQuotaSnapshot => 'Kota görüntüsü';

  @override
  String get statisticsNoQuotaHistory =>
      'Seçili tarih aralığında kaydedilmiş ChatGPT kota görüntüsü yok.';

  @override
  String get statisticsUsed => 'Kullanılan';

  @override
  String get statisticsResetAt => 'Sıfırlanma';

  @override
  String get statisticsQuotaAllowed => 'İsteklere açık';

  @override
  String get statisticsQuotaBlocked => 'İsteklere kapalı';

  @override
  String get statisticsQuotaUnknown => 'Kullanım durumu bilinmiyor';

  @override
  String get statisticsQuotaFreshnessCurrent => 'Güncel veri';

  @override
  String get statisticsQuotaFreshnessStale => 'Önceki veri';

  @override
  String get statisticsQuotaFreshnessUnknown => 'Veri durumu bilinmiyor';

  @override
  String get statisticsNotReported => 'Bildirilmedi';

  @override
  String get statisticsLegacyDataNote =>
      'Eski mesajlarda yalnızca çıkış tokenı saklanmış olabilir; eksik model ve giriş tokenı bilgileri tahmin edilmez.';

  @override
  String get statisticsModelsDevPricingNote =>
      'Katalog karşılıkları güncel models.dev fiyatlarıyla hesaplanır ve sağlayıcı faturası değildir. ChatGPT OAuth aboneliği, Fast istekleri ve desteklenmeyen hizmet katmanları hesaplamaya dahil edilmez.';

  @override
  String get statisticsLegacyOutput => 'Eski mesajlardan gelen çıkış tokenı';

  @override
  String get statisticsChatGptOAuth => 'ChatGPT OAuth';

  @override
  String get statisticsChatGptApi => 'ChatGPT API';

  @override
  String get statisticsOperationChat => 'Sohbet';

  @override
  String get statisticsOperationToolFollowUp => 'Araç devam isteği';

  @override
  String get statisticsOperationCompaction => 'Bağlamı özetleme';

  @override
  String get statisticsOperationTitleGeneration => 'Sohbet başlığı oluşturma';

  @override
  String get statisticsOperationLegacy => 'Eski mesaj';

  @override
  String get statisticsStatusCompleted => 'Tamamlandı';

  @override
  String get statisticsStatusFailed => 'Başarısız';

  @override
  String get statisticsStatusCancelled => 'Durduruldu';

  @override
  String get statisticsStatusInterrupted => 'Yarım kaldı';

  @override
  String get statisticsStatusPending => 'Devam ediyor';

  @override
  String get statisticsStatusLegacy => 'Geçmiş veri';

  @override
  String get refreshAll => 'Tümünü yenile';

  @override
  String get noChatGptAccountsForQuota => 'Bağlı ChatGPT hesabı bulunmuyor.';

  @override
  String get noChatGptAccountsForQuotaDescription =>
      'Kullanım limitlerinizi ve kotalarınızı görüntülemek için Bağlantılar sekmesinden ChatGPT hesabınızı ekleyin.';

  @override
  String get goToConnections => 'Bağlantılara Git';

  @override
  String get activeAccountBadge => 'Aktif';

  @override
  String workspaceQuotaLabel(String name) {
    return 'Çalışma alanı: $name';
  }

  @override
  String get modelsPageDescription =>
      'Hugging Face’teki modelleri bul ve indir.';

  @override
  String get modelSortDownloads => 'En çok indirilen';

  @override
  String get modelSortLikes => 'En çok beğenilen';

  @override
  String get modelSortRecentlyUpdated => 'Son güncellenen';

  @override
  String get modelPreviousPage => 'Önceki';

  @override
  String get modelNextPage => 'Sonraki';

  @override
  String modelPageLabel(int page) {
    return 'Sayfa $page';
  }

  @override
  String get modelFormatGguf => 'GGUF · llama.cpp';

  @override
  String get modelFormatTransformers => 'Transformers · vLLM';

  @override
  String get modelFormatExllama => 'ExLlama · EXL3';

  @override
  String get huggingFaceModelSearchHint => 'Hugging Face modellerinde ara';

  @override
  String get modelSearchRefresh => 'Model sonuçlarını yenile';

  @override
  String get modelSearchEmpty => 'Bu aramayla eşleşen model yok.';

  @override
  String get modelSearchFailed => 'Hugging Face modelleri yüklenemedi.';

  @override
  String get modelSearchUnavailable =>
      'Hugging Face\'e ulaşılamıyor. Bağlantını kontrol edip tekrar dene.';

  @override
  String get modelSearchRateLimited =>
      'Hugging Face çok fazla istek alıyor. Biraz bekleyip tekrar dene.';

  @override
  String get modelSearchInvalidResponse =>
      'Hugging Face\'in gönderdiği model bilgileri okunamadı. Biraz sonra tekrar dene.';

  @override
  String get modelSearchTimedOut =>
      'Hugging Face yanıt vermekte gecikti. Tekrar dene.';

  @override
  String get modelChooseForDetails =>
      'Dosyalarını incelemek için bir model seç.';

  @override
  String get modelDownloadsLabel => 'İndirme sayısı';

  @override
  String get modelLikesLabel => 'Beğeni sayısı';

  @override
  String get modelLicenseLabel => 'Lisans';

  @override
  String get modelRevisionLabel => 'Sürüm';

  @override
  String get modelFilesLabel => 'Model dosyaları';

  @override
  String get modelVisionComponentsLabel => 'Görsel bileşenleri';

  @override
  String get modelMtpComponentsLabel => 'MTP bileşenleri';

  @override
  String get modelAuxiliaryComponentsLabel => 'Diğer yardımcı bileşenler';

  @override
  String get modelDownloadComponentButton => 'Bu bileşeni indir';

  @override
  String get modelComponentDownloaded => 'Bileşen indirildi';

  @override
  String get modelShowMoreComponents => 'Daha fazla bileşen göster';

  @override
  String get modelReadmeLabel => 'Model açıklaması';

  @override
  String get modelReadmeMissing =>
      'Bu model için README açıklaması bulunmuyor.';

  @override
  String get modelReadmeAccessDenied =>
      'Model açıklamasını görüntülemek için bu depoya erişim gerekiyor.';

  @override
  String get modelReadmeTooLarge =>
      'README dosyası görüntülenemeyecek kadar büyük.';

  @override
  String get modelReadmeUnavailable => 'Model açıklaması yüklenemedi.';

  @override
  String get modelDownloadOptionsLabel => 'İndirme seçenekleri';

  @override
  String get modelDownloadGroupLabel => 'Dosya grubu';

  @override
  String get modelDownloadSizeLabel => 'Boyut';

  @override
  String get modelDownloadButton => 'Modeli indir';

  @override
  String get modelCancelDownload => 'İndirmeyi iptal et';

  @override
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount) {
    return 'Dosya $fileIndex/$fileCount: $fileName';
  }

  @override
  String get modelDownloadComplete =>
      'Model indirildi ve Yerel Modeller’e eklendi.';

  @override
  String get modelDownloadCancelled =>
      'Model indirmesi iptal edildi. Daha sonra sürdürebilirsin.';

  @override
  String get modelDownloadFailed => 'Model indirilemedi.';

  @override
  String get modelDownloadProgressUnavailable =>
      'İndirme ilerlemesi okunamadı.';

  @override
  String get modelRevisionChanged =>
      'Model Hugging Face’te değişmiş. Dosyalarını yeniden yükleyip tekrar dene.';

  @override
  String get modelDownloadAccessNeeded =>
      'Bu model deposu erişim istiyor. Hugging Face hesabı bağlantısı henüz desteklenmiyor.';

  @override
  String get modelNoCompatibleFiles =>
      'Bu depoda seçilen biçimle uyumlu eksiksiz model dosyası bulunamadı.';

  @override
  String get modelUnknownDownloadSize =>
      'Dosya boyutu bilinmediği için indirme güvenli biçimde başlatılamıyor.';

  @override
  String get modelDetailsLoading => 'Model dosyaları yükleniyor…';

  @override
  String get modelNoFiles => 'Bu biçim için uyumlu dosya yok.';

  @override
  String get modelGatedBadge => 'Erişim gerekli';

  @override
  String get modelPrivateBadge => 'Özel';

  @override
  String get modelSavedToFolder =>
      'İndirmeler, Ayarlar\'da bu motor için seçilen klasöre kaydedilir.';

  @override
  String get userQuestionTitle => 'Asistan yanıtını bekliyor';

  @override
  String get userQuestionRequiredHint => 'Zorunlu sorular işaretlidir';

  @override
  String get userQuestionSubmit => 'Yanıtı gönder';

  @override
  String get userQuestionResuming => 'Asistan devam ediyor';

  @override
  String get userQuestionUnavailable =>
      'Bu soru artık kullanılmıyor. Konuşmayı yeniden yükle.';

  @override
  String get userQuestionRequiredValidation =>
      'Devam etmek için zorunlu soruları yanıtla.';

  @override
  String get userQuestionSubmitFailed => 'Yanıtın kaydedilemedi. Tekrar dene.';

  @override
  String get userQuestionRequiredLabel => 'Zorunlu';

  @override
  String get userQuestionContinue => 'Asistanı sürdür';

  @override
  String get userQuestionSaved =>
      'Yanıtın kaydedildi. Hazır olduğunda devam et.';

  @override
  String get userQuestionLoadFailed =>
      'Bekleyen soru yüklenemedi. Tekrar dene.';

  @override
  String get userQuestionResumeFailed =>
      'Yanıtın kaydedildi ancak asistan devam edemedi. Tekrar dene.';

  @override
  String get userQuestionNotificationTitle => 'OpenChat seni bekliyor';

  @override
  String get userQuestionNotificationBody => 'Yapay zekâ yanıtını bekliyor.';

  @override
  String get assistantResponseNotificationReplyEmpty =>
      'Göndermeden önce bir yanıt yaz.';

  @override
  String get assistantResponseNotificationReplyTooLong =>
      'Bu yanıt bildirimden gönderilemeyecek kadar uzun. Göndermek için sohbet alanını kullan.';

  @override
  String get assistantResponseNotificationReplyUnavailable =>
      'Bu bildirim artık güncel değil veya sohbet kullanılamıyor. Sohbeti açıp yeni bir mesaj gönder.';

  @override
  String get assistantResponseNotificationReplyNotSent =>
      'Hızlı yanıt şu anda gönderilemedi. Sohbeti kontrol edip tekrar dene.';

  @override
  String fileChangesSummary(int count) {
    return '$count dosya değiştirildi';
  }

  @override
  String fileChangesLineCounts(int added, int removed) {
    return '+$added / -$removed';
  }

  @override
  String get fileChangesSomeCountsUnavailable => 'Satır sayısı hesaplanamadı';

  @override
  String get fileChangesView => 'Değişiklikleri görüntüle';

  @override
  String get fileChangesTrackingFailed =>
      'Bazı dosya değişiklikleri izlenemedi. Liste eksik olabilir.';

  @override
  String get fileChangesTitle => 'Bu sohbetteki değişiklikler';

  @override
  String get fileChangesOpenButton => 'Değişiklikler';

  @override
  String get fileChangesLoadFailed => 'Sohbet değişiklikleri yüklenemedi.';

  @override
  String get fileChangesDiffFailed => 'Dosya farkı yüklenemedi.';

  @override
  String get fileChangesConflict =>
      'Dosya, yapay zekâ düzenlemesinden sonra değişmiş. Dosyaya dokunulmadı.';

  @override
  String get fileChangesRevertFailed => 'Değişiklik geri alınamadı.';

  @override
  String get fileChangesEmpty =>
      'Bu sohbet için kaydedilmiş dosya değişikliği yok.';

  @override
  String get fileChangesDiffTitle => 'Farkını görmek için bir dosya seç';

  @override
  String get fileChangesBinary =>
      'İkili dosya değişiklikleri metin olarak gösterilemiyor.';

  @override
  String get fileChangesDiffUnavailable =>
      'Bu dosya için metin farkı gösterilemiyor.';

  @override
  String get fileChangesDiffTruncated =>
      'Fark uzun olduğu için yalnızca ilk bölümü gösteriliyor.';

  @override
  String get fileChangesActive => 'Değişti';

  @override
  String get fileChangesReverted => 'Geri alındı';

  @override
  String get fileChangesRevert => 'Geri al';

  @override
  String fileChangesMoreFiles(int count) {
    return '$count dosya daha';
  }

  @override
  String get fileChangesUnavailableTitle =>
      'Dosya değişiklikleri kullanılamıyor';

  @override
  String get goalSlashCommand => '/goal';

  @override
  String get goalCommandDescription =>
      'Bu mesajı hedef olarak başlat ve tamamlanana ya da senden bilgi gerekene kadar çalışmayı sürdür.';

  @override
  String get goalObjectiveRequired => 'Hedefi /goal komutundan sonra yaz.';

  @override
  String get goalWorking => 'Hedef üzerinde çalışılıyor';

  @override
  String get goalPaused => 'Hedef duraklatıldı';

  @override
  String get goalInterrupted => 'Hedef kesintiye uğradı';

  @override
  String get goalCompleted => 'Hedef tamamlandı';

  @override
  String get goalStopped => 'Hedef durduruldu';

  @override
  String get goalFailed => 'Hedef başarısız oldu';

  @override
  String get goalPausedForQuota =>
      'Model kotası veya hız sınırı nedeniyle duraklatıldı.';

  @override
  String get goalPausedForBlocker => 'Bir engel nedeniyle duraklatıldı.';

  @override
  String get goalPausedForUserInput =>
      'Senden bilgi beklendiği için duraklatıldı.';

  @override
  String get goalPausedByUser => 'Sen duraklattın.';

  @override
  String get goalPausedAfterError => 'Bir istek hatası nedeniyle duraklatıldı.';

  @override
  String get goalPauseAction => 'Hedefi duraklat';

  @override
  String get goalResumeAction => 'Hedefi sürdür';

  @override
  String get goalStopAction => 'Hedefi durdur';

  @override
  String goalElapsedTime(String time) {
    return 'Geçen süre: $time';
  }
}
