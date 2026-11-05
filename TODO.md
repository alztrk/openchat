# TODO — Architecture Migration

> **Durum:** Mimari geçiş fazları tamamlandı ✅; hata ve kullanılabilirlik araştırması devam ediyor.
> **Oluşturulma:** 2026-10-31
> **Son güncelleme:** 2026-11-05
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
- [x] CI/CD pipeline (`.github/workflows/windows-ci.yml`) — linter, test, locales, typecheck ve installer build
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
- [ ] Ayarlar sayfası yeniden tasarlanırken projeleri yönetmek için özel bir bölüm ekle; proje yapılandırma seçeneklerini bu bölümden sun.
- [x] Windows bildirim durumu ve test akışı: `OpenChat.Desktop` AppUserModelID, Start Menu shortcut ve WinRT status/activation akışları doğrulandı.

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
- [ ] FE-02 runtime: Daha önce kaydedilmiş bir sohbet ekini WebView2 masaüstünde kitaplıktan bulup seç; tür/model uyumsuzluğu ve ekleme hatası durumlarını da doğrula.
- [x] FE-03 kod dilimi: Proje, özel klasör ve Sohbetler bırakma hedeflerinde klasör ilişkisi servis taşıması başarılı olmadan değiştirilmiyor; başarısız mutation mevcut sidebar konumunu koruyor.
- [ ] FE-03 runtime: Proje/özel klasör/Sohbetler hedeflerine başarılı ve başarısız bırakmayı masaüstünde doğrula.
- [x] FE-04 kod dilimi: Özel klasör yazımı artık kalıcı depolama sonucunu bekliyor; UI yalnız başarıdan sonra güncelleniyor, hata halinde önceki düzen korunuyor ve kullanıcıya yerelleştirilmiş hata sunuluyor. Kayıt durumu erişilebilir biçimde duyuruluyor, klasör işlemleri tekilleştiriliyor; sohbet taşıması sonrası yazım hatasında önceki servis konumu telafi edilmeye çalışılıyor.
- [ ] FE-04 runtime: SQLite/servis yazım hatasını masaüstünde enjekte et; klasör oluşturma, yeniden adlandırma, silme ve taşımanın hatada korunmasını, retry ve yeniden açılışta kalıcılığı doğrula. Taşıma telafisi de başarısız olursa uyarı ve mevcut konumu kontrol etme akışını doğrula.
- [x] FE-05 kod dilimi: Başlangıç yüzeyi değişikliği, tercih okumasında zaten kullanılan `isAppSurface` doğrulayıcısına bağlandı; `security` seçeneği diğer yüzeyler gibi kaydediliyor. Regresyon testi ortak doğrulamayı ve kalıcılık öncesi state güncellenmemesini denetliyor.
- [ ] FE-05 runtime: Ayarlardan “Security scans” başlangıç yüzeyini seçip yeniden açılışta güvenlik taramaları ekranına geldiğini Windows masaüstünde doğrula.
- [x] FE-09: MCP kategori etiketleri ve filtreleri ortak kategori normalizasyonuna bağlandı; yedi kategori için etiket ve çapraz filtre eşleşme regresyonları eklendi.
- [ ] FE-09: MCP ekranındaki etiketleri ve filtre sonuçlarını değişiklik sonrası render edilmiş UI otomasyonuyla doğrula; bu oturumdaki Windows tıklama aracı iki yöntemde de hedefe ulaşamadı.
- [x] FE-10: Farklı yüzeye geçişte erişilebilir adı olan ana içerik odağı alıyor; ilk render’da odak değişmiyor ve aynı yüzeye basıldığında mevcut odak davranışı korunuyor. Regresyon testi eklendi.
- [ ] FE-10: Gerçek yüzey geçişi, geri/ileri geçmişi ve geçici sohbet onayından sonraki odağı render edilmiş masaüstü arayüzünde doğrula; Windows UI aracı bu oturumda hedefe tıklayamadı.
- [x] FE-11: Global kısayol dinleyicisi bubble aşamasına taşındı; kayıt modundaki capture listener tuş olayını önce engelliyor. Normal global dispatch ve listener cleanup fazı için regresyon testi eklendi.
- [x] FE-12: Komut paleti başlık çubuğu etiketi düzenlenmiş kısayol bağını gösteriyor; atanmamış durumda yerelleştirilmiş metin kullanıyor. Kaynak regresyon testi eklendi.
- [x] FE-13: Başlık çubuğu aramasında input, görünür recent/search popup ve aktif arama sonucu arasında ARIA ID ilişkileri kuruldu; expanded durumu gerçek popup görünürlüğünden türetiliyor. Kaynak regresyon testi eklendi.
- [ ] FE-13: Klavye/fare ile arama sonuçlarını açıp seç; erişilebilirlik ağacında controls ve active descendant hedeflerinin mevcut option'ları gösterdiğini masaüstü UI'da doğrula.
- [x] FE-R04 kod dilimi: Son aramalar için listbox-option ilişkisi ve `aria-selected` eklendi; ok tuşları/Enter ile seçim ve Escape ile kapatma destekleniyor. Tekil silme düğmeleri listbox dışındaki isimli `fieldset` grubunda ve arama terimini içeren erişilebilir ad kullanıyor.
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
- [ ] Design canvas bozulma kurtarması, security scan delete/cleanup reconciliation ve cancel timeout davranışı için ürün sözleşmesi belirle.
- [ ] Archive restore crash recovery/journal ve archive cancellation/status; attachment index cancellation/progress; worktree list cancellation ve backend limiti konularını tasarla.
- [ ] Eşzamanlı tercih güncellemelerinde patch merge/invariant davranışını test et; notification saat girişini Windows WebView2'de doğrula.
- [x] Merkezi CLI `check` hata toplama, local/CI Cargo feature ve lock eşleşmesi, task orchestration testleri ve FSD checker'ın tarama hatasında başarısız olması tamamlandı; uygulama ve odaklı test doğrulamaları Faz 10.2.2'de kaydedildi.
- [x] QA-09: Feature ve widget kaynaklarındaki `@app` bağımlılıkları kaldırıldı; OAuth/bildirim runtime'ları sahibi feature'lara taşındı. Checker alt→üst katman importlarını reddediyor, yalnız tanımlı feature-slice API'lerini kabul ediyor.
- [ ] QA-07/11: Sinyal aboneliği, Windows `taskkill /T /F` ve Unix process group sonlandırması, bounded kapanış, kapanışı doğrulayamama halinde profil koruması ve run-ID'li normal pencere kapanış işareti uygulandı. Geçici profil cleanup'ında `EPERM`/`EBUSY`/`ENOTEMPTY` için sınırlı tekrar ve hata sınıflarını kapsayan regresyon testleri eklendi. GUI normal kapatma ve gerçek PTY Ctrl+C sonrası UUID profil temizliği doğrulandı. İç `npm run dev` lifecycle mesajı, Tauri CLI `2.12.3` kaynak kodundaki normal `beforeDevCommand` process-tree cleanup davranışıyla uyumlu. Crash ve startup-failure ayrımı hâlâ doğrulanmalı.
- [x] `maintain-target --apply` ve `clean` için aktif build koruması, güvenli target seçimi ve görünür önizleme davranışı Faz 10.1'de uygulandı; bu araştırma sırasında gerçek cleanup komutları çalıştırılmadı.
- [x] Tek npm workspace/install otoritesi olarak kök workspace ve `package-lock.json` seçildi; eski iç lockfile kaldırıldı, CI cache yolu ve bootstrap köke hizalandı. Temiz `npm ci` ve izole dependency ağacındaki ilk Vite başlangıcı başarılı; gerçek UI rota yürütmesi ve paketlenmiş uygulama karşılaştırması ayrı takipte.
- [x] FE-18 kod dilimi: Son aramalar renderer `localStorage` yerine `unifiedStorage` üzerinden kalıcı saklanıyor; legacy `openchat:recent_searches` anahtarı korunuyor. En fazla 5 arama ve 256 karakter sınırı, cihaz içi saklama açıklaması, tek tek/toplu silme, retry ve hata geri bildirimi eklendi.
- [ ] FE-18 runtime: İzole masaüstü profilinde eski localStorage anahtarının migration sonrası görünmesini, yeni aramanın yeniden açılışta kalmasını, tekil/toplu silmeyi ve SQLite hata durumunda listenin korunup mesaj gösterilmesini doğrula.
- [x] FE-19 kod dilimi: Model indirme ilerleme bölgesi ve çubuğu artık “model indirme ilerlemesi” olarak adlandırılıyor; bilinen indirme miktarı `aria-valuetext` ile yerelleştirilmiş indirilen/toplam değerini taşıyor. İlerleme miktarı bilinmiyorsa uydurma değer metni eklenmiyor.
- [ ] FE-19 runtime: Gerçek bir model indirmesinde Windows WebView2 erişilebilirlik ağacındaki ilerleme adını ve byte değer metninin ilerlemeyle güncellenmesini doğrula.
- [x] `src/app/AppManager.tsx` consumer/import graph'ını tarama; kaynakta aktif consumer bulunmadı, çalışan giriş `bootstrap/main.tsx` üzerinden `bootstrap/App.tsx` yükleyip render ediyor.
- [x] `QA-12`: `AppManager` dosyası ve barrel export'ları tüketici/import taramasından sonra kaldırıldı; çalışan giriş `bootstrap/main.tsx` üzerinden `bootstrap/App.tsx` render ediyor.
- [x] Aynı makinedeki browser'dan Vite loopback URL'sini aç; uygulama kabuğu geliyor, Tauri RPC kullanan alanlar loading/error durumunda kalıyor.
- [x] Vite HTTP yanıtında dış `Origin` için CORS izin başlığının olmadığını, `localhost` loopback origininin ise izinli olduğunu doğrula; bu yalnız header probe'dur, browser enforcement testi değildir.
- [ ] Gerçek browser'da farklı HTTPS Origin/PNA, DNS rebinding, HMR ve LAN erişimini ölç; paketlenmiş uygulamanın dinleyen port envanterini çıkar.
- [x] Yerel llama.cpp başlatma argümanlarını ve sabitlenmiş upstream `b11349` CORS/auth varsayılanlarını kaynak koddan incele; statik riski raporla.
- [ ] Farklı browser Origin'den aktif model portuna istek, port keşfi, tarayıcının local-network policy davranışı ve boş port seçimi/child başlatma yarışını kontrollü runtime incele.
- [x] Yerel check kapsamını CI'deki ortak frontend/Rust doğrulama adımlarıyla hizala; Windows CI çalışması henüz gözlenmedi.
- [ ] Kalan doğrulanmış kullanıcı akışı ve servis kusurlarını ilgili düzeltme dilimlerinde regression senaryolarına bağla.
- [ ] NSIS installerı eksik/eski WebView2 runtime ve offline koşullarında kurup aç; sidecar ile ilk profil başlangıcını doğrula.
- [ ] Temiz dependency ağacında Vite, Görsel stüdyosu entry modülünü HTTP 200 ile dönüştürdü; browser'da lazy route'u gerçekten yürütüp önceki dinamik import hatasını ve paketlenmiş build'i karşılaştır.
- [ ] OpenChat'i OpenChat deposuna proje olarak ekleyip bir geliştirme görevini uygulamanın kendi sohbet, dosya araçları, terminal ve diff/revert akışıyla baştan sona yürüt; proje kökünün, izin kararlarının, komut çıktılarının ve oturum yeniden açma davranışının yeterli olduğunu doğrula.
- [ ] `AGENTS.md` içeriğinin OpenChat oturumuna otomatik aktarılmadığı kaynak bulgusunu gerçek kullanımda doğrula; proje talimatı ve `.openchat/rules/**/*.mdc` ile gerekli repo kurallarının eksiksiz uygulanıp uygulanmadığını belirle.
- [ ] OpenChat deposu için proje talimatı ve `npm run openchat -- check` gibi doğrulama komutlarını OpenChat'in proje ayarlarına tanımlamanın geliştirici akışını iyileştirip iyileştirmediğini kontrollü dogfooding denemesiyle değerlendir; bu araştırmada proje ayarı oluşturma.
- [ ] Dogfooding denemesinde seçilen sağlayıcı/modelin tool-calling desteğini, destek kapalıyken dosya ve terminal araçlarının davranışını ve kullanıcıya verilen açıklamayı doğrula.

- [x] Her bulguda önem ve güven düzeyini, etkilenen akışı, dosya/satır kanıtını, tetikleme koşulunu, beklenen ve mevcut davranışı, test kapsamını ve takip önerisini kaydet.
- [x] Doğrulanmış kusurları olası risklerden ve kullanılabilirlik gözlemlerinden ayrı tut.
- [x] Araştırma notlarını yalnızca gitignore kapsamındaki `.reports/openchat-deep-review-2026-11-04.md` dosyasında tut.
- [x] Kaynakta incelenebilir TODO maddelerini rapordaki FE/QA/T-HOST/SVC/CROSS/DOGFOOD bulgularıyla çapraz eşle; kod düzeltmesi ve ayrıca izin gerektiren runtime/kurulum doğrulamaları açık takip işi olarak bırak.
- [x] Bu turda kaynakta incelenebilen başlıkları ve yeni QA-09/QA-13 import-checker bulgularını rapora ekle; uygulama kodu, test, build, installer ve proje ayarlarına dokunma.
- [ ] **Runtime doğrulaması bekliyor:** Son salt-okunur kontrolde 5173, 1455, 1457 portlarında listener ve OpenChat/llama süreci görülmedi. Farklı-Origin/PNA/DNS rebinding/HMR/LAN ve aktif llama.cpp portu testleri için uygulama/model süreci gerekir.
- [ ] **Kurulum doğrulaması bekliyor:** WebView2 eksik/eski/offline senaryoları ile paketlenmiş uygulama ve sidecar ilk açılışı bu araştırmada kurulmadı/çalıştırılmadı; yalnızca installer yapılandırması ve CI akışı kaynakta incelendi.
- [ ] **Uçtan uca dogfooding bekliyor:** OpenChat sohbetiyle bu depoda dosya/terminal değişikliği yapıp kontrol ve diff/revert döngüsü yürütülmedi; sağlayıcı/model çağrısı ve yazma akışı gerektiriyor.

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
- [ ] Kalan kabul: desteklenen Windows CI çalışması; Cargo check/test'in 20 GiB hedef sınırına uyarak tamamlanması; browser'da Görsel stüdyosu lazy route ve paketlenmiş build karşılaştırması.

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
- [ ] `T-HOST-08` framework sınırı: Kilitli Tauri 2.12.4 invoke gövdesini komuta ulaşmadan önce genel byte limiti olmadan `serde_json::Value` olarak parse ediyor. Bu komutun typed payload sınırı fan-out'ı kısıtlar; parse öncesi bellek üst sınırı için framework düzeyinde destek/başka host sınırı araştır.
- [x] `T-HOST-09` kod dilimi: Pencere state yazma ve worker-start hataları path/OS hata metni basmadan sabit aşama kodlarıyla raporlanıyor. Worker başarısız revision'ı tamamlanmış deneme olarak işaretleyip spin-retry yapmıyor; flush ack gerçek kayıt sonucunu taşıyor. Okunabilir kayıt yoksa pencerenin maximize açıldığı module doc'ta açıklandı.
- [ ] `T-HOST-09` runtime kabulü: Gerçek Windows'ta yazılamayan/yavaş app-data diski, kapanış deadline'ı ve eksik/bozuk state dosyasıyla açılış davranışını doğrula; tanı çıktısında path veya OS hata metni olmadığını gözle.
- [ ] Kabul: doğrulanmış normal kapanış başarıyla raporlanır; interrupt ve child kapanışı bounded olur; süreç ağacı kapandığı doğrulanınca geçici profil temizlenir, kapanış doğrulanamazsa çalışan süreçten veri silmemek için profil korunur ve yolu raporlanır; dialog iptali ve IPC arızaları doğru, güvenli sonuç verir.

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

- [ ] `CROSS-02/03/06`: Farklı Origin, CORS preflight, PNA, DNS rebinding, HMR ve LAN davranışlarını kontrollü ortamda ölç; aktif llama.cpp port keşfini, boş port seçimi/child başlatma yarışını ve paketlenmiş dinleyici envanterini doğrula.
- [ ] Ölçüm sonucuna göre Vite dev server ve yerel model API'si için en dar gerekli erişim sınırını seç; gerekiyorsa auth, origin kısıtı veya Tauri üzerinden kontrollü proxy tasarımını uygula.
- [ ] Kabul: hangi yerel/dış originin hangi endpoint'e erişebildiği kanıtla kayıtlıdır; aynı makine testi LAN güvenliği kanıtı olarak kullanılmaz; paketlenmiş sürümde beklenmeyen listener kalmaz.

### 10.6 Kullanıcı akışları, erişilebilirlik ve kurtarma

- [ ] `FE-01..19`: Sohbet, MCP filtreleri, provider/proje formları, kısayollar, odak/combobox, loading metinleri, prompt retry, recent-search retention ve indirme progress etiketlerini tek kullanıcı akışı başına ele al.
- [x] `FE-20` kod dilimi: `Slider` `Field`'dan yalnızca native input'a ait `id` ve ARIA alanlarını alıyor; dahili `labelId` artık DOM'a yayılmıyor. Regresyon testi prop sızıntısını ve `Field` etiketinin `htmlFor`/`id` bağını kapsıyor.
- [x] `FE-20` runtime kısmi kabulü: İzole WebView2 bildirim ekranında kaydırıcının erişilebilir adı `Ses düzeyi`, değeri `%70` olarak göründü.
- [ ] `FE-20` kalan runtime kabulü: WebView2 geliştirme konsolunda React `labelId` uyarısının artık oluşmadığını doğrudan doğrula.
- [ ] `FE-21`: Tarihçe arama helper'ı, yerelleştirilmiş tarihçe arama etiketleri ve backend RPC'si mevcut ancak etkin bir UI tüketicisine bağlı değil; akışı ürün içinde etkinleştir veya kullanılmayan yüzeyleri kaldır.
- [x] `SVC-18`: Bozuk kanvas JSON/schema/size-limit durumlarını ayır; ham veriyi dışa aktarma ve kullanıcı onaylı yeni boş kanvas akışı ekle. Silme hatası özgün veriyi korur.
- [x] `SVC-19`: Tasarım, sohbet görünümü ve asistan yürütme tercih yazımlarını store başına seri kuyruğa al; sıradaki işlemin en güncel state'i temel alıp farklı alan patch'lerini birleştirdiğini test et.
- [x] `SVC-21`: Sessiz saatler native time alanları düzenleme sırasında taslak tutuyor ve yalnız geçerli `HH:mm` değerini odak kaybında kaydediyor; regresyon testleri ve izole WebView2 giriş/kayıt akışı doğrulandı.
- [x] `SVC-22`: `conversations` history araması artık yerel proje sonuçlarını eklemiyor; `projects` filtresi proje sonuçlarını koruyor ve iki davranış regresyon testleriyle doğrulandı.
- [ ] `T-HOST-01..04/07`: Kalan host runtime kabulleri için ürün/uygulama kararlarını ver; farklı DPI/monitör, startup hata/soğuk başlangıç, gerçek WebView2 crash/recovery ve hızlı resize/yavaş disk davranışlarını doğrula.
- [ ] Kabul: klavye ve ekran okuyucu akışları; loading/empty/error/retry/cancel; hatalı ve uzun girdiler; pencere boyutu ve yeniden açılış durumları ilgili yüzeylerde gözlenebilir testlerle kapsanır.

### 10.7 Paketlenmiş ilk açılış ve release doğrulaması

- [ ] `CROSS-05/QA-10`: NSIS kurulumunu WebView2 güncel, eksik, eski ve çevrimdışı koşullarında; uygulama, sidecar ve ilk profil başlangıcıyla doğrula. Release host Cargo lock davranışını kanıtla.
- [ ] Kabul: kurulum gereksinimleri/çevrimdışı davranış açıkça gösterilir; ilk açılış, servis readiness ve hata kurtarma paketten çalıştırılarak doğrulanır; CI installer üretmenin yanında desteklenen smoke doğrulamasını da yapar.

### 10.8 OpenChat ile OpenChat dogfooding

- [ ] `DOGFOOD-01/02`: Repo talimatlarının OpenChat'e aktarım yolunu ve `.openchat/instructions.md`, kurallar ve adlandırılmış doğrulama görevlerinin katkısını değerlendir; seçilen modeli tool-calling desteğiyle doğrula.
- [ ] İzole bir geliştirme görevinde repo kökünü seçme, talimatı anlama, izinli dosya düzenleme, terminal doğrulaması, diff inceleme/geri alma ve oturumu yeniden açıp devam etme adımlarını OpenChat içinde yürüt.
- [ ] Kabul: kullanıcı doğru proje kökünü ve değişiklikleri görebilir; araç desteği olmayan model açıkça anlaşılır; izinler işlem bazında uygulanır; check sonucu ve diff aynı oturumdan incelenebilir; yarıda kesilen görev güvenle devam ettirilebilir.

### Uygulama sırası ve entegrasyon kapısı

1. `10.1` cleanup güvenliği
2. `10.2` temiz kurulum ve merkezi doğrulama
3. `10.3` process/window yaşam döngüsü
4. `10.4` storage ve servis tutarlılığı
5. `10.5` yerel ağ/model sınırı
6. `10.6` kullanıcı akışları ve erişilebilirlik
7. `10.7` paketlenmiş ilk açılış
8. `10.8` gerçek OpenChat dogfooding

Her aşamada değişikliği küçük bir dikey dilim halinde uygula; ilgili regresyon testini ekle, hedefli kontrolleri çalıştır, ardından merkezi `check`/CI kapsamını doğrula. Aşama ancak kabul maddeleri kanıtlandığında kapanır. Kurulum, WebView2, ağ ve dogfooding kanıtı olmadan statik bulguları runtime çözümü olarak işaretleme.
