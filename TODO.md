# TODO — Architecture Migration

> **Durum:** Mimari geçiş fazları tamamlandı ✅; hata ve kullanılabilirlik araştırması devam ediyor.
> **Oluşturulma:** 2026-10-31
> **Son güncelleme:** 2026-11-07
> **Amaç:** OpenChat frontend ve Rust sidecar'ın endüstri standardına (FSD + Modular Monolith) incremental geçiş  

---

## Faz 0 — Hazırlık ✅ TAMAMLANDI (Commit: 41a51e0)

### 0.1 TypeScript Config
- [x] `tsconfig.json` — 5 yeni alias (`@app/*`, `@features/*`, `@entities/*`, `@widgets/*`, `@shared/*`)
- [x] `vite.config.ts` — 5 yeni alias eklendi

### 0.2 Lint Rules
- [x] `scripts/check-imports.mjs` yazıldı (import layer violation detection)

### 0.3 Directory Structure
- [x] `src/app/`, `src/features/`, `src/entities/`, `src/widgets/` oluşturuldu

---

## Faz 1 — Shared Layer Temizliği ✅ TAMAMLANDI (Commit: 2258d61)

- [x] `design-system/` → `shared/components/` (22 components + styles)
- [x] `platform/` → `shared/lib/platform/`
- [x] `localization/` → `shared/lib/i18n/`
- [x] `shared/utils/` → `shared/lib/utils/`
- [x] `shared/hooks/` → `shared/lib/hooks/`
- [x] 131 import path güncellendi
- [x] TypeScript check: clean (0 errors)

---

## Faz 2-3 — Modules → Features ✅ TAMAMLANDI (Commit: a2f5851)

- [x] `modules/conversations/` → `features/chat/` (40+ components, 21 data, 14 stores, 5 hooks)
- [x] `modules/providers/` → `features/provider-settings/` (19 files)
- [x] `modules/settings/` → `features/app-settings/` (41 files)
- [x] `modules/design/` → `features/design-workspace/` (9 files)
- [x] `modules/model-library/` → `features/model-catalog/` (12 files)
- [x] `modules/projects/` → `features/projects/` (16 files)
- [x] `modules/notifications/` → `features/notifications/` (3 files)
- [x] `modules/security-scans/` → `features/security-scans/` (4 files)
- [x] 200+ dosya taşındı, imports güncellendi
- [x] TypeScript check: clean (0 errors)

---

## Faz 4 — App.tsx Küçültme ✅ TAMAMLANDI (Commit: 9fa81fb)

- [x] `app/AppRouter.tsx` — surface routing
- [x] Application orchestration lives in `bootstrap/App.tsx`; the unused `app/AppManager.tsx` copy was removed after its consumer scan.
- [x] `app/AppLayout.tsx` — layout management

---

## Faz 5 — Entity Layer ✅ TAMAMLANDI (Commit: bcbde06)

- [x] `entities/conversation/` — ChatConversation, ChatMessage vb.
- [x] `entities/model/` — ChatModel, ChatModelProvider
- [x] `entities/project/` — ToolPermissionMode

---

## Faz 6 — Rust Sidecar Modularization ✅ TAMAMLANDI (Commits: b6a2017, 7b19a5a, 9de8a21, 72dca0c, ea59bda)

- [x] Domain types, providers, tools, storage, sandbox modüler mimarisi `services/openchat-local-service/src/` altında konsolide edildi
- [x] Crate ayrıştırma denemesinden kalan atıl taslak crate'ler (`crates/openchat-*`) ve çift workspace yapılandırması temizlendi
- [x] Workspace Cargo.toml: `openchat-desktop` ve `openchat-local-service` tek workspace altında hatasız derleniyor

---

## Faz 7 — CI Gates + Dokümantasyon ✅ TAMAMLANDI

- [x] `biome.json` — `noUnusedImports` kuralı ve formatlama ayarları
- [x] `package.json` — `check:imports`, `check:locales`, `test` scriptleri
- [x] `scripts/check-imports.mjs` — intra-feature ayrımı düzeltildi (0 errors)
- [x] Frontend test paketi (121 test, 28 dosya) FSD yollarına uyarlandı ve tüm testler geçiyor
- [x] CI/CD pipeline (`.github/workflows/desktop-ci.yml`) — Linux/Windows doğrulama ve paket build'leri
- [x] `AGENTS.md` — mimari kurallar, CLI ve FSD standartları güncellendi

---

## Özet

| Faz | Status | Dosya Sayısı | Commit |
|-----|--------|-------------|--------|
| 0 | ✅ | 4 | 41a51e0 |
| 1 | ✅ | 230 | 2258d61 |
| 2-3 | ✅ | 200+ | a2f5851 |
| 4 | ✅ | 3 | 9fa81fb |
| 5 | ✅ | 8 | bcbde06 |
| 6 | ✅ | Konsolide | ea59bda |
| 7 | ✅ | 30+ | Son |

**Toplam:** ~13 commit, 600+ dosya değişti.

---

## Faz 8 — Bug Fixes & Test Stabilization (2026-11-01)

### Sonraki işler
- [x] Ayarlar sayfası yeniden tasarlanırken projeleri yönetmek için özel bir bölüm ekle; proje yapılandırma seçeneklerini bu bölümden sun (`ProjectSettingsSection.tsx` ve `SettingsPage.tsx` entegrasyonu tamamlandı).
- [x] Windows bildirim entegrasyonu: `OpenChat.Desktop` AppUserModelID, Start Menu shortcut ve WinRT status/activation akışları doğrulandı.

### 8.1 Fixed Compilation Errors ✅
- [x] `services/openchat-local-service/src/domain/conversations/history.rs:368-373` — Missing `summary_text` wrapper function (syntax error from malformed copy-paste)
- [x] `services/openchat-local-service/src/domain/providers/chatgpt/compaction.rs` — Broken import path for `context_compaction::PruneLevel` + missing test arguments
- [x] `services/openchat-local-service/src/domain/providers/openai_compatible/context_compaction.rs` — Broken import path for `PruneLevel`
- [x] `services/openchat-local-service/src/domain/tools/file_ops.rs` — Added multiple occurrence check for `edit_file` to match test expectations
- [x] `scripts/tasks/check.mjs` — Removed invalid `--test-threads=0` flag

### 8.2 Build Status
- [x] `npm run openchat -- check-types` — **PASSES** (0 errors, 0 warnings)
- [x] Frontend tests — **121/121 PASS**
- [x] Locale check — **PASSES** (5 locales consistent)
- [x] TypeScript check — **PASSES**
- [x] Biome lint — **PASSES**
- [x] FSD Import check — **0 errors, 0 warnings**

### 8.3 Test Stabilization Status

| Category | Status | Verification |
|----------|--------|--------------|
| File changes/file ops/executor/terminal tests | ✅ Stable | Full suite passes with serialized Rust tests |
| RPC project tests | ✅ Fixed | Windows Git executable resolution |
| Context usage estimate | ✅ Fixed | Targeted test passes |
| Compaction logic | ✅ Fixed | Targeted test passes |

**Key Finding:** All "pre-existing" test failures pass when run individually (`cargo test -p openchat-local-service --lib -- <test_name>`). They fail only in the full test suite due to test isolation/state issues.

### 8.4 Recommended Next Steps (Priority Order)

#### High Priority (Quick Wins)
- [x] **Resolve Git executable on Windows** — Rust tests now resolve a real `git.exe` even when PATH exposes a `.cmd` shim
- [x] Full suite test isolation verified after Git executable resolution (`926 passed, 0 failed`)

#### Medium Priority
- [x] Context usage estimate targeted test passes without provider credentials
- [x] Compaction preservation targeted test passes
- [x] Full-suite isolation verified for file changes/file ops/executor/terminal tests

#### Low Priority (Architectural Debt)
- [x] **FSD Architecture Violations** — cross-feature imports reduced to **0 warnings**
  - Pure domain, preference, model, project, notification and provider helpers moved to `@shared`, `@entities` or `@app/runtime`.
  - Remaining feature composition is routed through explicit app-layer runtime bridges.

#### Optional Cleanup
- [x] `summary_text` is covered by conversation history tests; the dead-code allowance remains intentional for test-oriented helper coverage.
- [x] `PruneLevel` is used by the ChatGPT compaction tests; no unused import remains.

---

## Faz 9 — Hata ve Kullanılabilirlik Araştırması (2026-11-04)

> **Kapsam:** Yalnızca araştırma. Bu fazda uygulama kodu, testler veya yapılandırma değiştirilmez. Tam check/test paketi veya installer build çalıştırılmaz; normal kullanıcı akışını gözlemek için izole `dev-test-profile` başlatılmıştır. İlk başlangıç denemesi kullanıcı tarafından kesilmiş, sonraki izole profil açılmıştır. Bulgular kanıt, etki, yeniden üretme koşulu ve mevcut test kapsamıyla gitignored raporda tutulur.

### İnceleme başlıkları

- [x] **Doğrulama döngüsü:** Merkezi `check` görevinin frontend testlerini, locale tutarlılığını, FSD import kurallarını ve Rust kontrollerini kapsayıp kapsamadığını; CI farklarını incele.
- [x] **React kullanıcı akışları:** Sohbet gönderme, yanıt akışı, durdurma, yeniden deneme, sohbet değiştirme, ayarlar/sağlayıcı-model seçimi ve hata/boş/yükleniyor durumlarını statik incele.
- [x] **Tauri host ve IPC:** Uygulama/yan servis başlatma-kapatma, RPC çerçeveleme, komut/olay köprüsü, dialog ve pencere yaşam döngülerindeki başarısızlıkları statik incele.
- [x] **Depolama ve veri bütünlüğü:** Veritabanı başlatma, migration, mesaj/ek kaydı, arşiv/dışa aktarma, bozulma ve yeniden başlatma senaryolarını statik incele.
- [x] **Araçlar ve dosya işlemleri:** İzin, yürütme, kısmi değişiklik, fark gösterimi, checkpoint ve geri alma akışlarını statik incele.
- [x] **Kullanılabilirlik:** Hata mesajlarının çözüm yolu sunmasını; klavye erişimini, pencere boyutu değişimini ve uzun/eksik verili durumları statik incele.
- [x] **Normal kullanıcı ilk kullanım walkthrough'u:** Sohbet, Ayarlar, Kütüphane modeller/MCP/prompt şablonları, Güvenlik taramaları ve Görsel stüdyosu yüzeylerini izole geçici profilde açıp boş, loading, hata ve erişilebilirlik ağacı durumlarını gözle.
- [x] **Kapsam dışı kalan yüzeylerin statik taraması:** Vite loopback bind, browser/Origin/Host/HMR varsayılanları, OAuth callback, yerel model portu ve geliştirme/dağıtım ayrımını incele. Loopback yapılandırmasını dış ağ erişimiyle eş tutma.
- [x] **OpenChat ile OpenChat geliştirme (dogfooding) kaynak taraması:** Proje bağlamı, dosya okuma/yazma, terminal ve değişiklik inceleme/geri alma araçlarını; projeye özel talimatların nasıl yüklendiğini statik incele.
- [x] OpenChat deposunda `.openchat/instructions.md`, `.openchat/rules/`, `.openchat/tasks.json` ve `.openchat/tools.json` bulunup bulunmadığını kontrol et; repo şu an bu OpenChat'e özgü proje ayarlarını içermiyor.
- [x] Kod düzenleme akışının sağlayıcı/model tool-calling desteğine bağlı olduğunu statik incele; her sohbet modelinin kod geliştirme aracı sağlayabildiği varsayılmamalı.

### Bulgular ve takip işleri

- [x] Üç ayrı uzman incelemesi, çapraz kaynak taraması, ilk kullanım arayüz gözlemi, aynı makineden Vite adresini browser'da açma ve CORS yanıt başlığı probu tamamlandı; bulgular gitignored rapora kaydedildi. Normal browser uygulama kabuğunu açtı; Tauri RPC gerektiren alanlar loading/error durumunda kaldı. Görsel stüdyosu geliştirme runtime'ında hata sınırına düştü; eksik yerel dependency dosyası nedeniyle release kusuru olarak genellenmedi.
- [x] FE-01 kod dilimi: Model seçimi `null` olduğunda boş sohbet ekranı artık karşılama metni yerine model seçme yönergesini gösteriyor; seçili model ve katalog boş durumları için kaynak regresyon testi eklendi.
- [ ] FE-01 runtime: Model kataloğu dolu durumda model seçilmeden ve seçildikten sonra boş sohbet başlığını masaüstünde doğrula.
- [x] FE-02 kod dilimi: Composer ataç düğmesinden ayrı, yerelleştirilmiş “kitaplıktan seç” düğmesi eklendi. Kayıtlı ek seçimi composer'a aktarılıyor; dosya seçici, kitaplık ve dialog aynı model/gönderim/okuma/place-holder kilitlerini kullanıyor.
- [x] FE-02 servis sorgusu: Ek kitaplığı SQL'indeki `indexed` takma adı SQLite tarafından `INDEXED` sözdizimi olarak yorumlanıp liste RPC'sini kırıyordu; takma ad `attachment_index` olarak değiştirildi. Boş kitaplık ve kaydedilmiş ek sayfası regresyon testleri eklendi.
- [x] FE-02 runtime boş durumu: Güncel debug derlemesi izole WebView2 profilinde açıldı; Sohbetler > Kitaplıktan seç, servis sorgusu hatasız tamamlandı ve erişilebilirlik ağacında “Kullanılabilir kayıtlı sohbet dosyası yok.” görüldü. Test profili kapatıldı ve doğrulamalı cleanup yardımcısıyla kaldırıldı.
- [ ] FE-02 runtime seçim/hata durumları: Daha önce kaydedilmiş bir sohbet ekini WebView2 masaüstünde kitaplıktan seçip composer'a ekle; tür/model uyumsuzluğu ve artık bulunmayan dosya/ekleme hatası durumlarını doğrula.
- [x] FE-03 kod dilimi: Proje, özel klasör ve Sohbetler bırakma hedeflerinde klasör ilişkisi servis taşıması başarılı olmadan değiştirilmiyor; başarısız mutation mevcut sidebar konumunu koruyor.
- [ ] FE-03 runtime: Proje/özel klasör/Sohbetler hedeflerine başarılı ve başarısız bırakmayı masaüstünde doğrula.
- [x] FE-04 kod dilimi: Özel klasör yazımı artık kalıcı depolama sonucunu bekliyor; UI yalnız başarıdan sonra güncelleniyor, hata halinde önceki düzen korunuyor ve kullanıcıya yerelleştirilmiş hata sunuluyor. Kayıt durumu erişilebilir biçimde duyuruluyor, klasör işlemleri tekilleştiriliyor; sohbet taşıması sonrası yazım hatasında önceki servis konumu telafi edilmeye çalışılıyor.
- [ ] FE-04 runtime: SQLite/servis yazım hatasını masaüstünde enjekte et; klasör oluşturma, yeniden adlandırma, silme ve taşımanın hatada korunmasını, retry ve yeniden açılışta kalıcılığı doğrula. Taşıma telafisi de başarısız olursa uyarı ve mevcut konumu kontrol etme akışını doğrula.
- [x] FE-05 kod dilimi: Başlangıç yüzeyi değişikliği, tercih okumasında zaten kullanılan `isAppSurface` doğrulayıcısına bağlandı; `security` seçeneği diğer yüzeyler gibi kaydediliyor. Regresyon testi ortak doğrulamayı ve kalıcılık öncesi state güncellenmemesini denetliyor.
- [x] FE-05 runtime: İzole Windows WebView2 profilinde “Security scans” başlangıç yüzeyi seçildi, “Ayarlar kaydedildi.” durumu görüldü; pencere kapatılıp aynı profille yeniden açıldığında erişilebilirlik ağacı ana yüzey olarak “Güvenlik taramaları” gösterdi.
- [x] FE-06 kod/test dilimi: Node test runner'ına gerçek React DOM render desteği eklendi. FE-01 boş sohbet durumları, FE-02 kitaplık yükleme/seçim/hata/tekrar deneme, FE-03 başarısız klasör taşıması, FE-05 başlangıç yüzeyi kayıt başarısı/hatası ve FE-08 landmark adı/gezinti düğmesi test ediliyor.
- [x] FE-06 doğrulama sınırı: DOM testlerinin WebView2/Tauri/gerçek ekran okuyucu davranışını kanıtlamadığı kayda alındı; bunlara ait runtime kabulleri ayrı TODO maddelerinde tutuluyor.
- [x] FE-07 kod dilimi: Storage hazır olayı, yalnızca kullanıcı henüz yüzey veya geri/ileri gezinmesi yapmadıysa kayıtlı başlangıç yüzeyini uyguluyor. Erken gezinmede seçili yüzey ve mevcut geçmiş korunuyor; kaynak regresyon testi eklendi.
- [ ] FE-07 runtime: Yavaş storage/legacy tercih aktarımı sırasında başka bir yüzeye geç; storage-ready olayının seçimi ve geri/ileri geçmişini koruduğunu gerçek masaüstünde doğrula. Kullanıcı gezinmezse kayıtlı başlangıç yüzeyinin yine uygulandığını da doğrula.
- [x] FE-08 kod dilimi: Ana gezinme `<nav>` landmark'ı artık Sohbetler'e özel ad yerine beş locale'de “Ana gezinme” anlamındaki `primaryNavigation` etiketiyle adlandırılıyor; gerçek DOM render/erişilebilirlik regresyonu FE-06 testine taşındı.
- [ ] FE-08 runtime: Windows WebView2 erişilebilirlik ağacında ana gezinme landmark'ının seçilen dilde doğru adı taşıdığını doğrula.
- [x] FE-09: MCP kategori etiketleri ve filtreleri ortak kategori normalizasyonuna bağlandı; yedi kategori için etiket ve çapraz filtre eşleşme regresyonları eklendi.
- [x] FE-09 render testi: Gerçek `McpCatalogTab` render'ında yedi yerelleştirilmiş kategori düğmesi, `aria-pressed` ve her kategoriye ait tek sunucu kartı doğrulanıyor.
- [ ] FE-09 masaüstü runtime: Windows WebView2'de etiketleri ve filtre sonuçlarını ayrıca doğrula; önceki UI otomasyon denemesinde hedefe tıklama başarısız olmuştu.
- [x] FE-10: Farklı yüzeye geçişte erişilebilir adı olan ana içerik odağı alıyor; ilk render’da odak değişmiyor ve aynı yüzeye basıldığında mevcut odak davranışı korunuyor. Regresyon testi eklendi.
- [ ] FE-10: Gerçek yüzey geçişi, geri/ileri geçmişi ve geçici sohbet onayından sonraki odağı render edilmiş masaüstü arayüzünde doğrula; Windows UI aracı bu oturumda hedefe tıklayamadı.
- [x] FE-11: Global kısayol dinleyicisi bubble aşamasına taşındı; kayıt modundaki capture listener tuş olayını önce engelliyor. Normal global dispatch ve listener cleanup fazı için regresyon testi eklendi.
- [x] FE-12: Komut paleti başlık çubuğu etiketi düzenlenmiş kısayol bağını gösteriyor; atanmamış durumda yerelleştirilmiş metin kullanıyor. Kaynak regresyon testi eklendi.
- [x] FE-13: Başlık çubuğu aramasında input, görünür recent/search popup ve aktif arama sonucu arasında ARIA ID ilişkileri kuruldu; expanded durumu gerçek popup görünürlüğünden türetiliyor. Kaynak regresyon testi eklendi.
- [x] FE-13 render testi: Gerçek `TitleBarSearch` render'ında arama sonucunun `aria-controls` listbox'ına bağlanması, `aria-activedescendant` değerinin mevcut/seçili option'ı göstermesi, ok tuşuyla gezinme, option'a tıklamayla seçim ve Escape ile kapanma doğrulanıyor. Arama/kalıcı depolama sınırları test modülünde yalıtılmıştır.
- [ ] FE-13: Klavye/fare ile arama sonuçlarını açıp seç; erişilebilirlik ağacında controls ve active descendant hedeflerinin mevcut option'ları gösterdiğini masaüstü UI'da doğrula.
- [x] FE-R04 kod dilimi: Son aramalar için listbox-option ilişkisi ve `aria-selected` eklendi; ok tuşları/Enter ile seçim ve Escape ile kapatma destekleniyor. Tekil silme düğmeleri listbox dışındaki isimli `fieldset` grubunda ve arama terimini içeren erişilebilir ad kullanıyor.
- [x] FE-R04 render testi: Gerçek bileşen render'ında son arama option'ları ArrowDown/Enter ile gezilip açılıyor; terim adını taşıyan silme düğmesi listbox dışında kalıyor. Silme yazımı başarılı olunca seçenek kaldırılıyor; yazım hatasında liste korunup alert gösteriliyor. Depolama sonucu test sınırında yalıtılmıştır.
- [ ] FE-R04 runtime: Windows masaüstü WebView2’de klavye-only gezinmeyi ve ekran okuyucu ağacında listbox, seçili option ve terim adı içeren silme düğmelerini doğrula. İzole profilde sorgu/yok-sonuç popup'ı açılıp Escape ile kapandı; son arama verisi bulunmadığından option seçimi ve silme denetlenemedi.
- [x] Aktif uygulama yüzeylerinde iç içe `<main>` landmark'ları kaldırıldı: Settings ve Görsel stüdyosu içerikleri başlıklarıyla ilişkili section'lara dönüştürüldü. Kaynak regresyon testi eklendi.
- [ ] Ayarlar, Görsel kütüphanesi ve canvas yüzeylerini masaüstünde açıp erişilebilirlik ağacında yalnızca uygulama kabuğunun ana landmark'ı bulunduğunu doğrula.
- [x] FE-14: Provider API key ve proje MCP editörleri native form submit kullanıyor; Enter ile kaydetme mümkün, doğrulama akışları korunuyor. Regresyon testi eklendi.
- [ ] FE-14: API key ve proje MCP formlarında Enter ile gerçek UI submit akışını masaüstünde doğrula.
- [x] FE-15: Custom provider, proje oluşturma/MCP ve arşiv passphrase doğrulamaları ilgili alanlara `aria-invalid`/`aria-describedby`/alert ilişkisiyle bağlandı; submit sonrası ilgili ilk geçersiz alan odaklanıyor. Arşiv hatalarının dialog dışında yinelenen alert'i kaldırıldı; kaynak regresyon testi eklendi.
- [ ] FE-15: Custom provider, proje oluşturma/MCP ve arşiv passphrase hatalarının görünümünü, ekran okuyucu duyurusunu ve ilk geçersiz alana odağı gerçek masaüstü arayüzünde doğrula.
- [x] FE-16: Yanlış `modelsLoading`/`historyLoading` kullanımları düzeltildi; OAuth, sağlayıcı credential durumu, ajanlar, MCP credential durumu, prompt şablonları, proje içeriği ve istatistikler doğru domain etiketiyle yükleniyor. Beş locale tutarlı; kaynak regresyon testi eklendi.
- [x] FE-17 kod dilimi: Prompt şablonu okuması bozuk ham kaydı koruyor; storage ve schema hataları ayrılıyor. Retry, kullanıcı tetiklemeli ham export ve onaylı kalıcı reset eklendi. Kaydetme/silme artık durable sonucu bekleyip hata halinde false success göstermiyor; beş locale ve kaynak regresyon testleri eklendi.
- [ ] FE-17 runtime: Bozuk ham kayıt, indirilen dosya içeriği ve kalıcı silme başarısını/başarısızlığını Windows masaüstü WebView2'de doğrula.
- [x] `SVC-13/14/18`: Bozuk design canvas için ham veri kurtarma, onaylı reset ve kalıcı silme hatasında korunma; security scan silme sonrasında cleanup-pending uzlaştırması; scan operation iptalini tek deadline altında bekleme sözleşmeleri ve regresyonları tamamlandı.
- [x] `SVC-12/15/16/17/20`: Archive restore süreç çökmesi kalıntıları journal yerine başlangıç/restore orphan reconciliation ile toparlanıyor; archive export/inspect/restore status, cancellation ve progress; attachment index rebuild cancellation/status; worktree liste cancellation/toplam deadline ve 20 kayıt sınırı uygulandı ve regresyonlarla doğrulandı.
- [x] `SVC-19/21`: Eşzamanlı preference patch yazımları store başına sıraya alınıp güncel state'ten birleştiriliyor; notification saat taslakları yalnız geçerli `HH:mm` değerinde blur sırasında kaydediliyor. Patch merge regresyonları ve izole WebView2 saat girişi doğrulandı.
- [x] Merkezi CLI `check` hata toplama, local/CI Cargo feature ve lock eşleşmesi, task orchestration testleri ve FSD checker'ın tarama hatasında başarısız olması tamamlandı; uygulama ve odaklı test doğrulamaları Faz 10.2.2'de kaydedildi.
- [x] QA-09: Feature ve widget kaynaklarındaki `@app` bağımlılıkları kaldırıldı; OAuth/bildirim runtime'ları sahibi feature'lara taşındı. Checker alt→üst katman importlarını reddediyor, yalnız tanımlı feature-slice API'lerini kabul ediyor.
- [x] QA-07/11 kod dilimi: Sinyal aboneliği, Windows `taskkill /T /F` ve Unix process group sonlandırması, bounded kapanış, kapanışı doğrulayamama halinde profil koruması ve run-ID'li normal pencere kapanış işareti uygulandı. Geçici profil cleanup'ında `EPERM`/`EBUSY`/`ENOTEMPTY` için sınırlı tekrar ve hata sınıflarını kapsayan regresyon testleri eklendi. GUI normal kapatma ve gerçek PTY Ctrl+C sonrası UUID profil temizliği doğrulandı. İç `npm run dev` lifecycle mesajı, Tauri CLI `2.12.3` kaynak kodundaki normal `beforeDevCommand` process-tree cleanup davranışıyla uyumlu.
- [ ] QA-07/11 runtime kalan kabulü: Tauri crash'i, Vite/Tauri startup failure ve Ctrl+C exit-code aktarımını gerçek konsol/uygulama oturumunda ayrı doğrula; profili silme/koruma davranışı ve kullanıcı tanısı doğru olmalı.
- [x] `maintain-target --apply` ve `clean` için aktif build koruması, güvenli target seçimi ve görünür önizleme davranışı Faz 10.1'de uygulandı; bu araştırma sırasında gerçek cleanup komutları çalıştırılmadı.
- [x] Tek npm workspace/install otoritesi olarak kök workspace ve `package-lock.json` seçildi; eski iç lockfile kaldırıldı, CI cache yolu ve bootstrap köke hizalandı. Temiz `npm ci` ve izole dependency ağacındaki ilk Vite başlangıcı başarılı; gerçek UI rota yürütmesi ve paketlenmiş uygulama karşılaştırması ayrı takipte.
- [x] FE-18 kod dilimi: Son aramalar renderer `localStorage` yerine `unifiedStorage` üzerinden kalıcı saklanıyor; legacy `openchat:recent_searches` anahtarı korunuyor. En fazla 5 arama ve 256 karakter sınırı, cihaz içi saklama açıklaması, tek tek/toplu silme, retry ve hata geri bildirimi eklendi.
- [ ] FE-18 runtime: İzole masaüstü profilinde eski localStorage anahtarının migration sonrası görünmesini, yeni aramanın yeniden açılışta kalmasını, tekil/toplu silmeyi ve SQLite hata durumunda listenin korunup mesaj gösterilmesini doğrula.
- [x] FE-19 kod dilimi: Model indirme ilerleme bölgesi ve çubuğu artık “model indirme ilerlemesi” olarak adlandırılıyor; bilinen indirme miktarı `aria-valuetext` ile yerelleştirilmiş indirilen/toplam değerini taşıyor. İlerleme miktarı bilinmiyorsa uydurma değer metni eklenmiyor.
- [ ] FE-19 runtime: Gerçek bir model indirmesinde Windows WebView2 erişilebilirlik ağacındaki ilerleme adını ve byte değer metninin ilerlemeyle güncellenmesini doğrula.
- [x] `src/app/AppManager.tsx` consumer/import graph'ını tarama; kaynakta aktif consumer bulunmadı, çalışan giriş `bootstrap/main.tsx` üzerinden `bootstrap/App.tsx` yükleyip render ediyor.
- [x] `QA-12`: `AppManager` dosyası ve barrel export'ları tüketici/import taramasından sonra kaldırıldı; çalışan giriş `bootstrap/main.tsx` üzerinden `bootstrap/App.tsx` render ediyor.
- [x] Aynı makinedeki browser'dan Vite loopback URL'sini aç; uygulama kabuğuna giden gezinme artık Tauri servisleri başlatılmadan yerelleştirilmiş masaüstü gereksinimi ekranında sonlanıyor. Bu renderer guard'ı HTTP portunu kapatmaz veya LAN sınırı kanıtı sayılmaz.
- [x] Vite HTTP CORS sınırını ölç ve daralt: varsayılan ayar loopback `localhost` originlerini genişçe kabul ediyordu; artık izin verilen CORS origin'i yalnız `http://127.0.0.1:5173`. Diğer originlere GET yine HTTP 200 dönebilir, fakat `Access-Control-Allow-Origin` sabit dev URL'siyle eşleşmez. Bu raw-header ölçümüdür, browser enforcement veya port erişimini engelleme kanıtı değildir.
- [x] `CROSS-02` Vite Host başlığı sınırı: gerçek HTTP sunucusunda `127.0.0.1` Host'u 200, izinsiz `.invalid` DNS host'u 403 döndü. Vite 8.3.5 bağlandığı host'u otomatik izin listesine eklediği için ayrıca `allowedHosts` ayarı eklenmedi; aynı makineden loopback portuna erişimi bu engellemez.
- [ ] Gerçek browser'da farklı HTTPS Origin/PNA, DNS rebinding, HMR ve LAN erişimini ölç; paketlenmiş uygulamanın dinleyen port envanterini çıkar.
- [x] Yerel llama.cpp başlatma argümanlarını ve sabitlenmiş upstream `b11349` CORS/auth varsayılanlarını kaynak koddan incele; statik riski raporla.
- [x] `CROSS-06` managed llama.cpp kod dilimi: Her yönetilen başlatmada yeni UUIDv4 API anahtarı üret, sabitlenmiş `b11349`'e komut satırı yerine `LLAMA_API_KEY` child environment üzerinden ver, mevcut local-service bearer-auth akışını kullan ve kullanılmayan llama.cpp Web UI'ını kapat. Argüman, child environment, özellik isteği ve sohbet isteği testleri merkezi check'te geçti.
- [ ] `CROSS-06` runtime kabulü: Gerçek pinned `b11349` ve GGUF modeliyle eksik/yanlış anahtarın inference API'sinden `401` aldığını, OpenChat'in özellik/sohbet isteklerinin çalıştığını ve anahtarın renderer'a sızmadığını doğrula. Testte yalnız `GET /health` endpointinin upstream gereği public kaldığını hesaba kat.
- [ ] Farklı browser Origin'den aktif model portuna istek, port keşfi, tarayıcının local-network policy davranışı ve boş port seçimi/child başlatma yarışını kontrollü runtime incele.
- [x] Yerel check kapsamını Windows ve Linux CI'deki ortak frontend/Rust doğrulama adımlarıyla hizala; Tauri workspace kontrolünden önce host sidecar hazırlanıyor.
- [x] Araştırmada doğrulanan ve kaynak düzeyinde düzeltilebilen kullanıcı akışı/servis kusurları ilgili kod dilimlerinde regresyon senaryolarına bağlandı; gerçek WebView2, installer, veri içeren profil ve güç-kaybı kabulleri aşağıdaki runtime maddelerinde açık.
- [ ] NSIS installerı eksik/eski WebView2 runtime ve offline koşullarında kurup aç; sidecar ile ilk profil başlangıcını doğrula.
- [ ] Temiz dependency ağacında Vite, Görsel stüdyosu entry modülünü HTTP 200 ile dönüştürdü; browser'da lazy route'u gerçekten yürütüp önceki dinamik import hatasını ve paketlenmiş build'i karşılaştır.

- [x] Her bulguda önem ve güven düzeyini, etkilenen akışı, dosya/satır kanıtını, tetikleme koşulunu, beklenen ve mevcut davranışı, test kapsamını ve takip önerisini kaydet.
- [x] Doğrulanmış kusurları olası risklerden ve kullanılabilirlik gözlemlerinden ayrı tut.
- [x] Araştırma notlarını yalnızca gitignore kapsamındaki `.reports/openchat-deep-review-2026-11-04.md` dosyasında tut.
- [x] Kaynakta incelenebilir TODO maddelerini rapordaki FE/QA/T-HOST/SVC/CROSS bulgularıyla çapraz eşle; kod düzeltmesi ve ayrıca izin gerektiren runtime/kurulum doğrulamaları açık takip işi olarak bırak.
- [x] Bu turda kaynakta incelenebilen başlıkları ve yeni QA-09/QA-13 import-checker bulgularını rapora ekle; uygulama kodu, test, build, installer ve proje ayarlarına dokunma.
- [ ] **Runtime doğrulaması bekliyor:** Son salt-okunur kontrolde 5173, 1455, 1457 portlarında listener ve OpenChat/llama süreci görülmedi. Farklı-Origin/PNA/DNS rebinding/HMR/LAN ve aktif llama.cpp portu testleri için uygulama/model süreci gerekir.
- [ ] **Kurulum doğrulaması bekliyor:** WebView2 eksik/eski/offline senaryoları ile paketlenmiş uygulama ve sidecar ilk açılışı bu araştırmada kurulmadı/çalıştırılmadı; yalnızca installer yapılandırması ve CI akışı kaynakta incelendi.

---

## Linux sürümü ve masaüstü bildirimleri — entegrasyon tamamlandı

- [x] Windows ve Linux için yanıt bildirimi entegrasyonu tamamlandı; Windows WinRT/AppUserModelID akışı ve Linux masaüstü bildirim desteği mevcut.
- [x] Linux x86_64 için Debian ve AppImage paketleri yerel `build` komutuyla oluşturuluyor; doğrulama öncesinde host sidecar hazırlanıyor.
- [x] `.github/workflows/desktop-ci.yml` Windows ve Linux üzerinde ortak doğrulamayı, yerel paket build'lerini ve artefakt yüklemelerini çalıştırıyor.
- [x] `v*` tag release'i iki platform job'ına bağlı; Windows NSIS, Linux `.deb` ve AppImage artefaktlarından biri eksikse release yayınlanmıyor.
- [x] CI incelemesinde bulunan Windows Tauri workspace hatası düzeltildi: `check` ve `check-types`, Tauri workspace'i doğrulamadan önce host sidecar'ı hazırlar.
- [x] Dependabot npm güncellemeleri kök workspace ve kök `package-lock.json` üzerinden yürütülecek şekilde hizalandı.
- [ ] Bu değişiklikler GitHub'a gönderildikten sonraki Desktop CI koşusunda Linux/Windows job sonuçlarını ve oluşturulan release dosyalarını doğrula.
- [ ] Linux `.deb` ve AppImage paketlerini desteklenen temiz Linux ortamında kurup Secret Service, bildirim, autostart ve ilk sidecar başlangıcını doğrula.

---

## Faz 10 — Bulguların sıralı entegrasyon planı

> **Amaç:** Araştırmadaki bulguları küçük, geri alınabilir ve ölçülebilir değişiklik dilimleriyle kapatmak. Her dilim kendi davranış testi ve kanıtıyla tamamlanır; test/build/installer doğrulaması ilgili kod değişikliği başladığında yapılır. Faz 9 araştırma kapsamı kapandı; bu faz uygulama planıdır.

### 10.1 Build ve cleanup güvenliği

- [x] `QA-04/05`: `maintain-target` ve `clean` için workspace görev kilidi, aktif Cargo/rustc koruması, sabit cleanup hedefleri ve silme öncesi görünür önizleme eklendi. `maintain-target` artık eşik altındayken pruning yapmıyor; tam Cargo temizliğinde tüm hedefler önce `cargo clean --dry-run` ile doğrulanıyor.
- [x] Kabul: aktif build koruması, eşik altı no-op, hedeflerin önceden listelenmesi ve path sınırları davranış testleriyle doğrulandı. Odaklı 17 test ve frontend toplam 143 test geçti; gerçek `--apply` çalıştırılmadı.

### 10.2 Bağımlılık kurulumu ve doğrulama harness'i

- [x] `CROSS-01/QA-08` uygulaması: kök npm workspace ve kök `package-lock.json` otorite seçildi; eski iç lockfile kaldırıldı; CI cache ve bootstrap bu köke bağlandı. Bootstrap, tüm workspace ağacını `npm ls` ile doğrular ve eksikse `npm ci` çalıştırır. Davranışlar gerçek install yapmayan testlerle doğrulandı.
- [x] `QA-08` temiz kurulum: mevcut `node_modules` ağaçlarına dokunmadan izole boş npm workspace'te `npm ci` ile 159 paket kuruldu ve tüm frontend workspace dependency ağacı `npm ls` ile doğrulandı.
- [x] `QA-08` ilk dev başlangıcı: izole temiz dependency ağacında `npm run dev -- --force` ile Vite 8.3.5 başladı; root, app bootstrap ve Görsel stüdyosu modülleri HTTP 200 ile dönüştü; optimizer önbelleği oluştu. Browser/Tauri uçtan uca yürütmesi ayrıca bekliyor.
- [x] `QA-02/03/06`: Merkezi `check` frontend test/locale/import/build ve locked Rust format/check/test adımlarını çalıştıracak şekilde genişletildi; hata veren adımdan sonra kalan adımların da denenip özetlenmesi için test eklendi. CI ortak doğrulama adımları için merkezi CLI'ı çağırıyor. Odaklı doğrulama geçti; tam Cargo çalışması hedef alanı sınırı nedeniyle bu turda çalıştırılmadı.
- [x] `QA-01`: Mesaj gruplama ve gün ayırma testleri üretimde kullanılan saf fonksiyonlara bağlandı. 10.000 mesaj benchmark'ı sentetik fixture için tanı ölçümü verir; React render/scroll ve gerçek kullanıcı workload'u ölçmediği açıkça yazılır. Donanıma bağlı sert süre eşiği kaldırıldı.
- [x] `QA-09`: Repo mimari kuralıyla çelişen feature/widget → app izinleri kaldırıldı; app-runtime facade'ları alt feature API'lerine yönlendirildi; çapraz feature sınırları public slice entrypoint'lerine alındı.
- [x] `QA-13`: Import checker re-export'ları tarıyor, dosya/ağaç okuma hatalarını hata sonucu yapıyor; statik import, type-only re-export, okuma ve enumerasyon hataları için test eklendi.
- [x] `QA-14`: ChatGPT account testlerinin geçici veritabanı dizinleri PID ve süreç içi sayaç yerine UUID ile tekil oluşturuluyor; önceki kesilmiş test koşusundan kalan dizinin yeniden kullanımına bağlı tablo çakışması kapatıldı.
- [x] Kısmi kabul: check adımları ayrı isimlerle raporlanır ve runner hatasında diğerleri de denenir; local/CI ortak komutları hizalı; frontend build, test, locale/import kontrolleri ve checker testleri geçti.
- [x] Cargo check/test disk sınırı kabulü: Merkezi Windows `npm run openchat -- check` hedef kullanımı 15.86 GiB / 20 GiB iken başarıyla tamamlandı.
- [ ] Kalan kabul: sidecar sırası düzeltmesinden sonraki Windows CI koşusu ve browser'da Görsel stüdyosu lazy route ile paketlenmiş build karşılaştırması.

### 10.3 CLI, Tauri ve sidecar yaşam döngüsü

- [x] `QA-07/11` kod dilimi: Interrupt sonrası 1,5 sn genel graceful bekleme ve Tauri için Windows'ta 10 sn bekleme, Windows process-tree sonlandırma veya Unix process-group sinyalleme eklendi. Test profili yolunu kullanan süreçlere ek 2 sn kapanış payı verilir; kalan eşleşen süreç ağaçları sonlandırılır ve 30 sn içinde kapanmadığı doğrulanır. Sorgu/sonlandırma başarısızsa profil korunur. Normal test penceresi kapanışı Tauri'nin kullanıcı kaynaklı exit isteği, başarılı sidecar kapanışı ve eşleşen tek kullanımlık run marker'ı ile tanınıyor; `0` ve gözlenen Windows `4294967295` yalnız bu marker ile kabul ediliyor.
- [x] `QA-07` cleanup dayanıklılığı: Doğrulanmış UUID geçici profilini kaldırırken `EPERM`, `EBUSY` ve `ENOTEMPTY` hatalarında toplam yaklaşık 15,85 saniyelik sınırlı tekrar eklendi; temizlenebilir geçici kilit, kalıcı kilit ve kalıcı erişim reddi testleri kapsıyor.
- [x] `QA-11` GUI runtime kabulü: Normal pencere kapatma run-ID marker kontrolünden geçti; wrapper exit `0`, UUID profil temizliği ve OpenChat süreç kapanışı gözlendi. İç `npm run dev` lifecycle çıktısı Tauri `2.12.3` normal before-dev process-tree cleanup yoluyla açıklandı.
- [x] `QA-07` Ctrl+C runtime kabulü: Doğrudan Node CLI ile izole profilde gerçek PTY Ctrl+C denendi; öncesinde Vite `127.0.0.1:5173` üzerinden `HTTP 200` verdi. Ctrl+C sonrası profil yoluna referans veren Windows süreç kalmadı ve `--cleanup-profile` klasörü otomatik kaldırdı. Oturum aracı çıkış kodunu `1` gösterdi; Node'un beklenen `130` koduna eşlemesini ayrıca doğrula.
- [x] `QA-07` doğrulama: `npm run openchat -- check` başarılı; frontend 205/205, local-service 985 geçti ve 21 ignored, integration 3 geçti ve 1 ignored. `maintain-target` 15,67 GiB / 20 GiB ölçtü; cleanup gerekmedi.
- [ ] `QA-07` hata runtime kabulü: Gerçek Tauri crash'i ve Vite/Tauri startup failure yollarında marker üretilmediğini, profili temizleme davranışını ve kullanıcı tanısını ayrı doğrula.
- [x] `T-HOST-01` kod dilimi: Kaydedilen normal/maximized geometri açılışta geri yükleniyor; boyut/konum monitör çalışma alanına sığıyor, artık bulunmayan monitör birincil monitöre taşınıyor. Geometri hesaplama için altı Rust testi eklendi.
- [x] `T-HOST-01` runtime kısmi kabulü: Aynı Windows ekranında test profiline kayıtlı normal geometri gerçek pencerede yeniden açıldı; büyütme/geri yükleme sonrası geometri state dosyasına kaydedildi.
- [ ] `T-HOST-01` kalan runtime kabulü: Farklı çözünürlük/DPI ve monitör çıkarma sonrası konum, boyut ve maximize durumunu doğrula.
- [x] `T-HOST-02` kod dilimi: Ana Tauri penceresi gizli başlatılıyor; normal React ekranı ve startup hata ekranı `flushSync(render)` sonrasında ortak `show()` yolunu kullanıyor. `startupWindowVisibility.test.mjs` host görünürlüğü, render/show sırasını ve benchmark görünürlük event'ini sabitliyor.
- [ ] `T-HOST-02` runtime kabulü: Gerçek Windows penceresinde soğuk/yavaş başlangıç ve startup hata ekranında boş/unstyled frame olmadığını gözle doğrula.
- [x] `T-HOST-03` kod dilimi: WebView2 hata sayacı iki ardışık otomatik reload'dan sonra recovery gösteriyor; yalnız ana pencerenin React görünümü render edilip pencere gösterildikten sonra gelen readiness bildirimi sayacı sıfırlıyor. Sayaç doyumlu artıyor; seri, reset ve taşma için Rust testleri eklendi.
- [ ] `T-HOST-03` runtime kabulü: Gerçek Windows WebView2 renderer çökmesini en az üç kez ve araya başarılı renderer hazır olma durumu koyarak tetikle; iki reload, recovery ve sayacın sıfırlanmasını gözle doğrula.
- [x] `T-HOST-04` kod dilimi: Recovery ekranı en son doğrulanmış OpenChat URL'sine klavyeyle kullanılabilen bağlantı sunuyor; yalnız Tauri/loopback adresleri kabul ediliyor ve HTML escape ediliyor. URL yoksa güvenli yeniden başlatma yönergesi gösteriliyor. Recovery sayfası ana pencereyi ve standart başlık/kapatma kontrollerini açıyor; React hazır olunca özel başlık çubuğu geri geliyor. Metinler beş desteklenen dilde.
- [ ] `T-HOST-04` runtime kabulü: Üçüncü gerçek WebView2 renderer çökmesinde recovery penceresinin görünmesini, bağlantının app URL'sine dönmesini, frontend açılışını ve local-service handshake'ini doğrula.
- [x] `T-HOST-05` kod dilimi: Native, renderer ve permission pipe yazımları tek FIFO worker'a taşındı; kuyruk 8 bekleyen mesaj ve 32 MiB toplam frame alanıyla sınırlı. I/O sırasında process/native-response/permission kilitleri tutulmuyor. 10 sn yazım timeout'u sidecar process group'unu kesiyor; kuyruk doluluğu açık backpressure hatası veriyor. FIFO, byte/adet sınırı, ack timeout'u ve Windows process-group kesmesi test edildi.
- [ ] `T-HOST-05` runtime kabulü: Paketlenmiş Windows sidecar'ın stdin'ini durdurup kuyruk doluluğunu, 10 sn timeout sonrası tüm process tree'nin kapanmasını, bekleyen native/permission isteklerinin temizlenmesini ve yeni generation ile tekrar başlatılmasını doğrula.
- [ ] `T-HOST-06` native dialog yaşam döngüsü: Kullanıcı dialog'u sonuçlandırana kadar bekleme beklenen davranış. Mevcut plugin gerçek dialog iptal handle'ı sunmadığı için sabit timeout ekleme; Windows'ta cancel, ana pencere kapanışı ve uygulama kapanışında invoke/archive cleanup davranışını runtime'da doğrula. Gerçek takılma görülürse timeout'tan önce iptal edilebilir dialog entegrasyonunu seç.
- [x] `T-HOST-07` kod dilimi: `Moved`/`Resized` event'leri geometriyi bellekte güncelliyor; tek background writer son event'ten 300 ms sonra coalesced atomic save yapıyor. Normal boyut/konum maximized durumunda korunuyor; minimize edilmiş pencere atlanıyor. Kapanış en son geometriyi worker'a flush edip en fazla 2 sn bekliyor. `dev-test-profile` writer ve restore akışı state'i yalnızca geçici profil kökündeki `window-state.json` dosyasına yazar; testler dış/iç boyut dönüşümünü kapsar.
- [x] `T-HOST-07` runtime kısmi kabulü: Gerçek Windows penceresi geçici profile seed edilen geometriyle açıldı; büyütme/geri yükleme ve normal kapanış geometriyi kaydetti. İkinci açılış/kapanışta kayıt `1132×828` olarak sabit kaldı. Tauri'nin `set_size` iç boyutu, `outer_size` dış boyutu döndürdüğünden önceki akışta her açılışta `16×39` px artış oluşuyordu; restore sırasında mevcut çerçeve farkı çıkarılarak düzeltildi.
- [ ] `T-HOST-07` kalan runtime kabulü: Hızlı sürekli drag/resize sırasında UI tepkisini ve yavaş/erişilemeyen disk durumunda 2 sn kapanış deadline'ını doğrula.
- [x] `T-HOST-08` kod dilimi: Renderer komutu yalnız sabit app-setting event'ini yayınlıyor; payload tam `key`/`value_json` alanlarından oluşuyor, fazladan alanlar reddediliyor, alanlar 128 B ve 2 MiB ile sınırlandırılıyor. Testler şekil, aşım ve izin verilen üst sınırı kapsıyor.
- [ ] `T-HOST-08` runtime kabulü: Gerçek iki pencerede ayar değişikliğinin alındığını ve bozuk/limit aşan payload'ın yayınlanmadığını doğrula.
- [x] `T-HOST-08` kaynak araştırması: Kilitli `tauri 2.12.4` / `tauri-runtime-wry 2.12.3` / `wry 0.57.0` akışında WebView2 mesajı önce `String` olarak alınıyor, Tauri invoke zarfı `serde_json::Value` olarak çözüldükten sonra app handler çalışıyor; genel gövde byte sınırı veya uygulama konfigürasyonu/hook'u bulunmadı. Tauri `tracing` feature'ı mevcut build'de kapalı; açılırsa upstream IPC trace span'ının tüm gövdeyi taşıdığı ayrıca dikkate alınmalı.
- [ ] `T-HOST-08` framework/runtime takibi: WebView2'nin belgelenmiş/ölçülmüş gerçek mesaj boyutu ve kaynak sınırını belirle; Tauri/Wry'e bounded ingress desteği eklenirse yükseltmeyi tekrar değerlendir. Uygulama komutlarındaki alan sınırlarını invoke zarfı sınırı gibi sunma.
- [x] `T-HOST-09` kod dilimi: Pencere state yazma ve worker-start hataları path/OS hata metni basmadan sabit aşama kodlarıyla raporlanıyor. Worker başarısız revision'ı tamamlanmış deneme olarak işaretleyip spin-retry yapmıyor; flush ack gerçek kayıt sonucunu taşıyor. Okunabilir kayıt yoksa pencerenin maximize açıldığı module doc'ta açıklandı.
- [ ] `T-HOST-09` runtime kabulü: Gerçek Windows'ta yazılamayan/yavaş app-data diski, kapanış deadline'ı ve eksik/bozuk state dosyasıyla açılış davranışını doğrula; tanı çıktısında path veya OS hata metni olmadığını gözle.
- [ ] Runtime kabulü: Gerçek Tauri/CLI oturumunda normal kapanış, interrupt, child kapanışı, doğrulanamayan kapanışta profilin korunması, native dialog iptali ve IPC arızası; yukarıda uygulanan sınırlar ve güvenli sonuçlarla uyuşmalı.

### 10.4 Yerel servis ve veri bütünlüğü

- [x] `SVC-01`: Normal chat, provider yanıtını ancak run sonucu kalıcılaştırıldıktan sonra başarılı döndürüyor; run okunamaz veya yazılamazsa `storage_unavailable` dönüyor ve güvenli diagnostic kaydediliyor. Başarı, missing-run ve SQLite write-failure yolları test edildi.
- [x] `SVC-02` hata yolu: Yeni yayımlanan attachment dizini, SQLite commit hatasında yalnızca DB transaction'ı sonrasında aynı mesaj satırı yoksa temizleniyor; kalıcı satır varsa eşzamanlı kayıt verisini koruyor. Hedef dizin staging sırasında mevcutken eşzamanlı silinmişse DB transaction'ı açıldıktan sonra attachment tekrar stage ediliyor. Başarılı kayıt, ertelenmiş foreign-key commit hatası ve mevcut mesaj satırını koruma testleri geçti.
- [x] `SVC-02` süreç-crash toparlaması: Açılışta yalnız tam biçimi doğrulanmış `.stage-<güvenli-mesaj-id>-<UUID>` ek klasörleri containment ve symlink kontrollerinden sonra siliniyor. Staging taraması servis sürecinde `OnceLock` ile bir kez çalışıyor; normal orphan temizliği ve iptal edilebilir archive temizliği staging'i koruyor. Bozuk adlar silinmeden hata/diagnostic bırakıyor; live ekler korunuyor.
- [x] `SVC-02` güç-kaybı kod dilimi: Ek taşıyan mesaj transaction'ı yalnız kendi süresince SQLite `synchronous=FULL` kullanıyor, işlemden sonra varsayılan `NORMAL` ayarına dönüyor. Unix'te yeni üst klasör girdileri, stage klasörü ve yayınlanan rename'in üst klasörü sync ediliyor; Windows'ta stage dizini `MOVEFILE_WRITE_THROUGH` ile yayınlanıyor. Eşleşen testler geçti; bu, gerçek güç-kesintisi kanıtı değildir.
- [ ] `SVC-02` güç-kaybı runtime kabulü: Paketlenmiş Windows ve desteklenen Unix profilinde ani süreç/VM kapatma veya disk-cache fault injection ile DB satırı ve attachment dosyasının birlikte toparlandığını doğrula. Windows'ta yeni attachments/conversation klasörlerinin güç-kaybı dayanıklılığı için belgelenmiş klasör flush garantisini ayrıca doğrula; attachment kaydetme p50/p95 gecikmesini normal kayıtla karşılaştır.
- [x] `SVC-03`: Ekler kökü gerçekten yoksa temizlik no-op; bozuk/erişilemeyen kökte DB silmesi `attachmentCleanupPending` ile başarılı dönüyor. Tek sohbet silme ve tüm geçmişi temizleme arayüz durumunu güncelliyor, yerelleştirilmiş uyarı gösteriyor; açılışta DB'de karşılığı olmayan sohbet dizinleri ve geçerli attachment envelope'ı bulunmayan mesaj ek dizinleri temizleniyor, canlı ekler korunuyor. Eksik/bozuk kök, kısmi silme ve live/orphan ayrımı test edildi.
- [x] `SVC-04`: Mesaj kaydı, mevcut uzantı/MIME metadata doğrulayıcısını dosya yazımından önce çalıştırıyor; uyuşmayan MIME ve desteklenmeyen uzantı `invalid_request_params` ile reddediliyor. DB ve dosya sisteminde kayıt oluşmadığını doğrulayan RPC testi eklendi.
- [x] `SVC-05`: Persisted history decoder, GIF MIME'ını mevcut imza doğrulamasıyla birlikte kabul ediyor; RPC ile kaydedilip SQLite'tan okunan GIF'in history decode round-trip testi eklendi.
- [x] `SVC-06`: Attachment envelope'ı plain içerikle değiştiren message upsert, DB commit'i sonrası güvenli ve yarışa dayanıklı mesaj klasörü temizliği yapıyor; temizlik bekliyorsa kayıt başarılı ve `attachmentCleanupPending` açıkça dönüyor. Açılış kurtarması orphan mesaj klasörlerini tekrar deniyor.
- [x] `SVC-07`: Normal assistant save ve tool checkpoint, attachment envelope merge'ünden sonraki son içerik byte boyutunu aynı 4 MB validator ile denetliyor; sınırı aşan kayıt `SQLITE_TOOBIG` ile reddediliyor ve önceki içerik korunuyor.
- [x] `SVC-08`: Mesaj geçmişi RPC'si, `(created_at, id)` sıralamasında 100 kayıtlık geriye doğru cursor sayfaları ve `hasMore` döndürüyor. Aktif sohbet son sayfadan açılıyor; kullanıcı daha eski mesajları isterse yükleyebiliyor. Tam geçmiş gerektiren mevcut tüketiciler sayfaları kronolojik sırayla birleştiriyor. 205 eşit zaman damgalı mesaj testi üç sayfada kayıpsız ve kararlı sıralamayı doğruladı; merkezi check geçti.
- [x] `SVC-09`: Düz mesaj attachment/escape prefix'iyle başlıyorsa servis yeni metin escape prefix'iyle saklıyor; servis ve React decoder özgün metni geri çıkarıyor. Eski bozuk JSON prefix içeriği metin olarak okunabiliyor; assistant checkpoint/final kayıtları da aynı kaçış kuralını kullanıyor. RPC ve decoder round-trip testleri ile merkezi check geçti.
- [x] `SVC-10`: Önceden encode edilmiş attachment envelope'ındaki metadata ile ayrı byte payload'ı `id`, ad, MIME, boyut ve tür bakımından eşleşmeden servis dosya staging'i veya DB yazımı başlatmıyor. Tam eşleşen akış ve ID/ad uyuşmazlıklarında dosya/mesaj bırakılmaması test edildi; merkezi check geçti.
- [x] `SVC-11`: Silinmiş conversation için `chat.send` artık `conversation_not_found` döndürüyor. Rota okuması eksik konuşmayı erken reddediyor; run kaydı sırasında oluşan foreign-key kısıtı da pending-run çatışmasından ayrıştırılıyor. Yinelenen run ID çatışması korunuyor; regresyon testleri ve merkezi check geçti.
- [x] `SVC-12`: Archive restore, başlamadan orphan attachment klasörlerini uzlaştırıyor; temizleme hatası varsa import'u başlatmıyor. Servis başlangıcında ve yeni restore öncesinde abandoned `restore-*` staging klasörleri containment kontrolüyle temizleniyor; aynı süreçteki aktif klasörler korunuyor. Yarım kalmış dosya/stage kalıntısından restore retry testi ve merkezi check geçti.
- [x] `SVC-13`: Security scan silme, DB commit'inden sonra `{ deleted: true, attachmentCleanupPending }` döndürüyor. UI silinen taramayı listeden çıkarıyor; dosya temizliği bekliyorsa kullanıcıya yerelleştirilmiş durum mesajı gösteriliyor. Açılıştaki orphan attachment uzlaştırması tekrar deneyebiliyor; hata ve başarılı temizlik yolları test edildi, merkezi check geçti.
- [x] `SVC-14`: Security scan silerken cancellation ve operation settlement aynı 10 saniyelik deadline içinde bekleniyor; bekleme aşılırsa kayıt silinmiyor ve kullanıcıya tekrar deneyebileceği açık hata gösteriliyor.
- [x] `SVC-15`: Worktree listeleme, Git çağrıları boyunca cancellation alıyor ve 25 saniyelik ortak backend deadline'ı kullanıyor; 30 saniyelik RPC timeout'undan önce typed timeout hatası döndürüyor. Boş ve dolu başarı yanıtları dönmeden önce deadline/cancellation yeniden kontrol ediliyor. UI timeout'u ayırt edip yenileme öneriyor.
- [x] `SVC-16`: Worktree listesi sabit ID sıralı ilk 20 kaydı, tarama sırasından bağımsız ve sınırlı bellekle seçiyor. Backend create, aynı proje için süreç içi ve OS dosya kilidi altında sayım/oluşturma yaparak eşzamanlı isteklerin 20 sınırını aşmasını engelliyor; limit dolu hatası yerelleştirilmiş, kırpılma mesajı görünmeyen sonraki kayıtları kaldırma yoluyla yönetmeyi açıklıyor. Kararlı seçim, limit ve eşzamanlı oluşturma testleri ile merkezi check geçti.
- [x] `SVC-17`: Attachment text-index rebuild ve clear için süreç içi single-flight, ölçülebilir durum/ilerleme, rebuild iptali ve timeout sonrası status reconciliation ekle; iptal/hata halinde önceki indeks korunmalı.
- [x] `SVC-20`: Conversation archive export/inspect/restore işlemlerinde cancellation, timeout, progress/status ve yeniden deneme davranışını hizala. RPC cancellation, single-flight status, byte/item progress, 30 dakikalık kullanıcı iptali ve final export/restore commit gate eklendi; restore öncesi orphan temizliği de aktif operasyon kapsamında.
- [x] Kabul: iptal kazandığında export kalıntısı temizlenir ve restore DB/dosyaları commit öncesi geri alınır; timeout veya renderer yeniden açılışı sonrası aktif işlem status üzerinden gözlenip güvenle iptal edilebilir; export'ta bilinmeyen toplam sırasında işlenen öğe sayısı gösterilir.

### 10.5 Dış erişim ve yerel model sınırı

- [x] `CROSS-02` kullanılabilirlik dilimi: Tauri dışı başlangıç, storage/sidecar/overlay akışından önce erişilebilir ve beş dilde yerelleştirilmiş masaüstü gereksinimi ekranı gösteriyor. Runtime'da aynı Vite URL'si tarayıcıda bu ekranı, Tauri penceresinde normal sohbeti gösterdi. Guard yalnız UI davranışını değiştirir; HTTP isteğini, kaynakların indirilmesini veya loopback portunu engellemez.
- [x] `CROSS-02` CORS dilimi: Vite'ın varsayılan geniş localhost-origin regex'i yerine `tauri.conf.json` içindeki sabit dev origin'i (`http://127.0.0.1:5173`) CORS izni oldu. Config regresyon testi ve raw HTTP GET/preflight başlıkları doğrulandı. Dış origin yanıtı okuyamaz; doğrudan GET/gezinti hâlâ çalışır. Tauri WebView2 sohbeti normal yüklendi.
- [x] `CROSS-06` managed model API kod dilimi: Her başlatmada benzersiz UUIDv4 anahtarı `LLAMA_API_KEY` child environment üzerinden veriliyor; anahtar süreç argümanlarına yazılmıyor. Protected özellik/sohbet istekleri bearer auth kullanıyor ve gereksiz llama.cpp Web UI kapalı.
- [ ] `CROSS-02/03/06` runtime: Farklı Origin, CORS preflight, PNA, DNS rebinding, HMR ve LAN davranışlarını kontrollü ortamda ölç; aktif llama.cpp portunda yetkisiz isteğin reddini, port keşfini, boş port seçimi/child başlatma yarışını ve paketlenmiş dinleyici envanterini doğrula.
- [x] `CROSS-06` managed model API için kaynak temelli ilk erişim sınırı seçildi: local-service tarafından saklanan bearer anahtarı ve kapalı Web UI. CORS/PNA ve browser-enforced erişim sınırları runtime kabulünde kalıyor.
- [ ] Runtime kabulü: Hangi yerel/dış originin hangi endpoint'e erişebildiği kanıtla kayıtlı olmalı; aynı makine testi LAN güvenliği kanıtı sayılmamalı; paketlenmiş sürümde beklenmeyen listener kalmamalı.

### 10.6 Kullanıcı akışları, erişilebilirlik ve kurtarma

- [x] `FE-01..19` kod/regresyon dilimleri: Sohbet, MCP filtreleri, provider/proje formları, kısayollar, odak/combobox, loading metinleri, prompt retry, recent-search retention ve indirme progress etiketlerine ait kaynak düzeltmeleri ile otomatik regresyonlar tamamlandı. Gerçek masaüstü akışları ve erişilebilirlik kabulleri aşağıda ayrı takip ediliyor.
- [x] `FE-R02` kod dilimi: Özel klasör tercih okuyucusu geçerli, bozuk ve okunamayan veriyi ayırıyor; bozuk ham değer korunup dışa aktarılabiliyor. Bozuk/okunamayan veri varken klasör CRUD ve konuşma konumu değişiklikleri engelleniyor; yazıcı da saklanan veriyi yeniden doğruluyor. Decoder ve React DOM regresyonları geçti.
- [ ] `FE-R02` runtime: İzole Windows WebView2 profilinde bozuk JSON ve kısmi geçersiz kayıtla klasör uyarısını, kurtarma dosyasının ham veriyi aynen taşımasını ve klasör/konuşma değişikliklerinin saklanan değeri koruduğunu doğrula.
- [x] `FE-20` kod dilimi: `Slider` `Field`'dan yalnızca native input'a ait `id` ve ARIA alanlarını alıyor; dahili `labelId` artık DOM'a yayılmıyor. Regresyon testi prop sızıntısını ve `Field` etiketinin `htmlFor`/`id` bağını kapsıyor.
- [x] `FE-20` runtime kısmi kabulü: İzole WebView2 bildirim ekranında kaydırıcının erişilebilir adı `Ses düzeyi`, değeri `%70` olarak göründü.
- [ ] `FE-20` kalan runtime kabulü: WebView2 geliştirme konsolunda React `labelId` uyarısının artık oluşmadığını doğrudan doğrula.
- [x] `FE-21` kod dilimi: Başlık çubuğuna gelişmiş tarihçe araması bağlandı; konuşma/mesaj/proje türü, sağlayıcı, model, proje, arşiv durumu, sıralama ve dahil bitiş tarihli aralık filtreleri eklendi. Boş filtreler RPC isteğinden çıkarılıyor; yerel takvim günleri servis zaman sınırlarına uyarlanıyor; preset kaydetme/silme UI state'ini ancak SQLite kalıcı yazımı başarılı olduktan sonra güncelliyor.
- [x] `FE-21` kalıcı yazım hatası: Kayıtlı arama ekleme/silme akışları `setPersistedItem()` bekliyor; yazım hatasında mevcut seçim/liste korunuyor, ekleme ve silme için ayrı yerelleştirilmiş hata gösteriliyor. Regresyon testleri kalıcı yazım beklemesini ve iki hata yolunu kapsıyor.
- [x] `FE-21` boş profil runtime kabulü: İzole Windows WebView2'de filtreler yüklendi ve `OpenChat` sorgusu RPC hatası olmadan beklenen boş sonucu gösterdi.
- [ ] `FE-21` veri içeren profil runtime kabulü: Gerçek konuşma/mesaj/proje kayıtlarında arama sonucu seçimini, filtreleri ve hedef yüzeye geçişi doğrula; izole profilde bu veriler bulunmuyordu.
- [ ] `FE-21` kayıtlı arama runtime kabulü: En az iki farklı sorgu/filtre preset'i arasında normal fare ve klavye kullanımıyla geçiş yap; tüm alanların doğru geri yüklendiğini doğrula. İzole WebView2 erişilebilirlik otomasyonunda kayıt/listeme görüldü, seçim sonucu tutarlı doğrulanamadı.
- [x] `FE-22`: ResizeObserver döngü bildirimleri `useVirtualMessageList`, `DatePicker`, `ColorPicker`, `SettingsPage`, `ConversationPane`, `ConversationSidebar` ve `DesignImageGallery` üzerinde `requestAnimationFrame` ve frame-coalescing ile korundu; `main.tsx` içine W3C spesifikasyonuna uygun zararsız döngü bildirimi filtresi eklendi.
- [x] `FE-22` kaynak taraması: ResizeObserver kullanan yanıtların boyut/konum durumunu veya sanal liste ölçülerini güncellediği doğrulandı; tarama bildirilen WebView2 hatasını tek bir observer'a bağlamadı. Runtime attribution ve kullanıcı etkisi açık.
- [x] `SVC-18`: Bozuk kanvas JSON/schema/size-limit durumlarını ayır; ham veriyi dışa aktarma ve kullanıcı onaylı yeni boş kanvas akışı ekle. Silme hatası özgün veriyi korur.
- [x] `SVC-19`: Tasarım, sohbet görünümü ve asistan yürütme tercih yazımlarını store başına seri kuyruğa al; sıradaki işlemin en güncel state'i temel alıp farklı alan patch'lerini birleştirdiğini test et.
- [x] `SVC-21`: Sessiz saatler native time alanları düzenleme sırasında taslak tutuyor ve yalnız geçerli `HH:mm` değerini odak kaybında kaydediyor; regresyon testleri ve izole WebView2 giriş/kayıt akışı doğrulandı.
- [x] `SVC-22`: `conversations` history araması artık yerel proje sonuçlarını eklemiyor; `projects` filtresi proje sonuçlarını koruyor ve iki davranış regresyon testleriyle doğrulandı.
- [ ] `T-HOST-01..04/07`: Kalan host runtime kabulleri için ürün/uygulama kararlarını ver; farklı DPI/monitör, startup hata/soğuk başlangıç, gerçek WebView2 crash/recovery ve hızlı resize/yavaş disk davranışlarını doğrula.
- [ ] WebView2 runtime kabulü: Klavye ve ekran okuyucu akışları; loading/empty/error/retry/cancel; hatalı ve uzun girdiler; pencere boyutu ve yeniden açılış durumları gerçek masaüstü yüzeylerinde doğrulanır.

### 10.7 Paketlenmiş ilk açılış ve release doğrulaması

- [ ] `CROSS-05/QA-10`: NSIS kurulumunu WebView2 güncel, eksik, eski ve çevrimdışı koşullarında; uygulama, sidecar ve ilk profil başlangıcıyla doğrula. Release host Cargo lock davranışını kanıtla.
- [ ] Linux paket runtime kabulü: CI'den gelen `.deb` ve AppImage paketlerini temiz, desteklenen Linux ortamında kurup Secret Service, bildirim, XDG autostart ve ilk profil/sidecar başlangıcını doğrula.
- [ ] Paket runtime kabulü: Kurulum gereksinimleri/çevrimdışı davranış açıkça gösterilir; ilk açılış, servis readiness ve hata kurtarma paketten çalıştırılır; desteklenen CI installer için smoke doğrulaması sağlar.

### Uygulama sırası ve entegrasyon kapısı

1. `10.1` cleanup güvenliği
2. `10.2` temiz kurulum ve merkezi doğrulama
3. `10.3` process/window yaşam döngüsü
4. `10.4` storage ve servis tutarlılığı
5. `10.5` yerel ağ/model sınırı
6. `10.6` kullanıcı akışları ve erişilebilirlik
7. `10.7` paketlenmiş ilk açılış

Her aşamada değişikliği küçük bir dikey dilim halinde uygula; ilgili regresyon testini ekle, hedefli kontrolleri çalıştır, ardından merkezi `check`/CI kapsamını doğrula. Aşama ancak kabul maddeleri kanıtlandığında kapanır. Kurulum, WebView2 ve ağ kanıtı olmadan statik bulguları runtime çözümü olarak işaretleme.

---

## Faz 11 — Çoklu Model Kıyaslama ve Sesli Etkileşim (Planlanan)

### 11.1 Yan Yana Model Desteği (Split Chat / Multi-Model Execution)
- [ ] Aynı istemin (prompt) seçilen 2-3 farklı modele (bulut ve yerel dahil) eşzamanlı gönderilmesi.
- [ ] Yan yana dikey bölünmüş sohbet görünümü (Split View), bağımsız akış (stream) yönetimi.
- [ ] Çıktı karşılaştırma metrikleri: İlk token gecikmesi (TTFT), saniye başına token hızı, toplam maliyet ve harcanan token kıyaslaması.
- [ ] Kıyaslanan yanıtlar arasından tek tıkla tercih edilen yanıtı seçip konuşmaya o daldan devam edebilme.

### 11.2 Sesli Etkileşim (Voice Interaction: STT & TTS)
- [ ] Yerel veya tarayıcı API'si üzerinden sesli giriş (Speech-to-Text / yerel Whisper veya Web Speech API).
- [ ] Model yanıtlarının otomatik veya isteğe bağlı seslendirilmesi (Text-to-Speech / sistem sesi veya yerel ses sentezleme).
- [ ] Ses kaydı sırasında canlı dalga boyu/görsel geri bildirim, sessizlik algılama (VAD) ve ses kısayolları.
- [ ] Ses verilerinin yerel gizlilik ilkelerine uygunluğu ve isteğe bağlı açılıp kapatılabilmesi.

---

## Faz 12 — Ajan Kalitesi ve Güvenilirlik (Planlanan)

### 12.1 Dayanıklı Çalıştırmalar (Durable Runs)
- [ ] Kalıcı çalıştırma durumu (persistent run state) ve SQLite üzerinde durum takibi.
- [ ] Çalıştırmaları duraklatma (pause), yeniden başlatma (restart) ve kullanıcı iptali (cancellation).
- [ ] Uygulama veya süreç çökmesinde kesilen çalıştırmaların güvenle kurtarılması (interrupted-run recovery).

### 12.2 Mesaj Dallanması ve Yanıt Sürümleri (Message Branches)
- [ ] Aynı konuşma içinde mesaj düzenlendiğinde yeni dal oluşturma ve konuşma içi yanıt sürümleri arasında gezinme (1/2 okları).
- [ ] Farklı dallar arasında aktif dalı seçebilme ve seçilen daldan konuşmaya devam edebilme.

### 12.3 Kapsamlı Alt Ajan Mimarisi (Subagents)
- [ ] Üst çalıştırmadan alt çalıştırmaya bağlam aktarımı ve izinli araç listesi (tool allowlist) zorlaması.
- [ ] Alt ajanlar için eşzamanlılık (concurrency), zaman aşımı (timeout) ve token/kullanım bütçesi sınırları.
- [ ] Alt ajan ilerleme durumunun ana konuşma akışında ve yan panelde eşzamanlı takibi.

### 12.4 Yeniden Kullanılabilir İş Akışları (Reusable Workflows)
- [ ] Parametrik şablonlar (parameterized recipes): başlangıç istemi, bağlam, sağlayıcı ve araç seti tanımları.
- [ ] Şablonları kaydetme, düzenleme, dışa/içe aktarma ve tek tıkla yeni konuşmada başlatabilme.

---

## Faz 13 — Sağlayıcı ve Yerel Çıkarım Genişletmeleri (Planlanan)

### 13.1 Ollama ve LM Studio Entegrasyonu
- [ ] Ollama ve LM Studio için ayrılmış yerel bağlantı sihirbazı.
- [ ] Yerel portların otomatik tespiti ve sağlık kontrolleri (health checks).
- [ ] Yerel sunuculardan tek tıkla model listesini çekip kataloğa otomatik ekleme.

### 13.2 Bulut Sağlayıcı Genişletmeleri
- [ ] Amazon Bedrock: Converse ve ConverseStream API desteği, bölgesel yapılandırma ve AWS kimlik doğrulama.
- [ ] Microsoft AI Foundry: deployment tanımlayıcıları ve model bazlı API uyumluluk desteği.

### 13.3 Yerel Çıkarım ve Model Yönetimi
- [ ] macOS için llama.cpp paketleme, kurulum, model yönetimi ve çalışma zamanı yaşam döngüsü.
- [ ] Yerel model kataloğu geliştirmeleri: dosya boyutu kontrolü, indirme ilerlemesi, SHA-256 bütünlük doğrulaması ve yarıda kalan indirmeleri devam ettirme.
- [ ] Bağlam penceresi ve model yetenek metaverileri: görsel (vision), akıl yürütme (reasoning) ve araç çağırma (tools) desteklerinin model kartında gösterimi.

---

## Faz 14 — Erişilebilirlik ve Sağlayıcı Kullanım Doğrulamaları

### 14.1 Canlı Hesap Kullanım Takibi (Provider Usage Coverage)
- [ ] Sağlayıcıların bildirdiği ek kullanım alanlarının (akıl yürütme tokenları, önbellek okuma/yazma) yerel kullanım defterine (`ledger`) eksiksiz eşlenmesi.
- [ ] Canlı sağlayıcı hesapları (OpenAI, Claude vb.) üzerinden kullanım istatistiklerinin ve maliyet hesaplamalarının uçtan uca doğrulanması (release gate).

### 14.2 Erişilebilirlik ve Klavye Navigasyonu (A-013)
- [ ] Klavye ile tam gezinme, görünür odak (focus visibility), yüksek kontrast ve azaltılmış hareket (reduced motion) kontrolleri.
- [ ] Metin ölçekleme (%125–%200), dar ekran düzenleri ve uzun çeviri metinleri uyumluluğu.
- [ ] Yerel Windows UI Automation ve ekran okuyucu (NVDA/Narrator) doğrulamaları.

---

## Faz 15 — Senkronizasyon, Dağıtım ve Çapraz Platform

### 15.1 Şifreli Senkronizasyon (Encrypted Sync)
- [ ] Cihazlar arası uçtan uca şifreli veri senkronizasyonu (opt-in).
- [ ] Cihaz anahtarları yönetimi, çevrimdışı çalışma desteği ve eşitleme çakışma çözümü (conflict resolution).

### 15.2 İmzalı Dağıtım ve Güvenlik (Signed Distribution)
- [ ] Windows Authenticode kod imzalama entegrasyonu.
- [ ] Yazılım malzeme listesi (SBOM) üretimi ve yayınlanması.
- [ ] Kararlı (stable) ve önizleme (preview) güncelleme kanalları; güncelleme paketi bütünlük doğrulaması.

### 15.3 macOS Masaüstü Desteği
- [ ] macOS x86_64 ve ARM64 (Apple Silicon) derleme ve paketleme (DMG / app bundle).
- [ ] macOS Keychain üzerinden güvenli kimlik/parola saklama entegrasyonu.
- [ ] macOS standart uygulama veri dizinleri ve bildirim entegrasyonu.

---

## Faz 16 — Ertelenen Gelecek İşler (Deferred)

- [ ] Yaşam döngüsü kancalarına (lifecycle hooks) sahip gelişmiş iş akışları.
- [ ] Mobil platform desteği (Android ve iOS istemcileri).
- [ ] Genel soyut yetkilendirilmiş ajan çerçevesi (generalized delegated-agent architecture).
- [ ] Ek yerel çıkarım motorlarının (MLC-LLM, ONNX Runtime vb.) değerlendirilmesi ve entegrasyonu.
