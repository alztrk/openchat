# TODO — Architecture Migration

> **Durum:** Mimari geçiş fazları tamamlandı ✅; hata ve kullanılabilirlik araştırması devam ediyor.
> **Oluşturulma:** 2026-10-31
> **Son güncelleme:** 2026-11-04
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
- [x] `app/AppManager.tsx` — global event management
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
- [ ] MCP kategori filtrelerindeki `webSearch`, `devOpsCloud`, `memoryReasoning` anahtar uyumsuzluğunu düzelt ve filtre sonuçlarını doğrula.
- [ ] Yüzey geçişinde odak yönetimini; kısayol kaydı sırasındaki global kısayol yan etkisini; başlık çubuğu kısayol etiketini ve arama combobox erişilebilirliğini düzelt.
- [ ] Provider API key ve proje MCP editörlerine klavye ile submit davranışı ekle; doğrulama hatalarını ilgili alanlara bağla.
- [ ] Yanlış loading metinlerini domain'e göre düzelt; prompt template load hatasına güvenli retry/recovery ekle.
- [ ] Design canvas bozulma kurtarması, security scan delete/cleanup reconciliation ve cancel timeout davranışı için ürün sözleşmesi belirle.
- [ ] Archive restore crash recovery/journal ve archive cancellation/status; attachment index cancellation/progress; worktree list cancellation ve backend limiti konularını tasarla.
- [ ] Eşzamanlı tercih güncellemelerinde patch merge/invariant davranışını test et; notification saat girişini Windows WebView2'de doğrula.
- [x] Merkezi CLI `check` hata toplama, local/CI Cargo feature ve lock eşleşmesi, task orchestration testleri ve FSD checker'ın tarama hatasında başarısız olması tamamlandı; uygulama ve odaklı test doğrulamaları Faz 10.2.2'de kaydedildi.
- [x] QA-09: Feature ve widget kaynaklarındaki `@app` bağımlılıkları kaldırıldı; OAuth/bildirim runtime'ları sahibi feature'lara taşındı. Checker alt→üst katman importlarını reddediyor, yalnız tanımlı feature-slice API'lerini kabul ediyor.
- [ ] QA-07/11: Sinyal aboneliği, Windows `taskkill /T /F` ve Unix process group sonlandırması, bounded kapanış, kapanışı doğrulayamama halinde profil koruması ve run-ID'li normal pencere kapanış işareti uygulandı; 165 frontend ve 84 Tauri testi geçti. Gerçek GUI normal kapatma, terminal Ctrl+C ve crash/startup-failure runtime ayrımı hâlâ doğrulanmalı.
- [x] `maintain-target --apply` ve `clean` için aktif build koruması, güvenli target seçimi ve görünür önizleme davranışı Faz 10.1'de uygulandı; bu araştırma sırasında gerçek cleanup komutları çalıştırılmadı.
- [x] Tek npm workspace/install otoritesi olarak kök workspace ve `package-lock.json` seçildi; eski iç lockfile kaldırıldı, CI cache yolu ve bootstrap köke hizalandı. Temiz `npm ci` ve izole dependency ağacındaki ilk Vite başlangıcı başarılı; gerçek UI rota yürütmesi ve paketlenmiş uygulama karşılaştırması ayrı takipte.
- [ ] `recent searches` renderer `localStorage` saklama/retention kararını privacy sözleşmesiyle karşılaştır; hassas sorgu içerme olasılığını ve uzunluk sınırını değerlendir.
- [x] `src/app/AppManager.tsx` consumer/import graph'ını tarama; kaynakta aktif consumer bulunmadı, çalışan giriş `bootstrap/main.tsx` üzerinden `bootstrap/App.tsx` yükleyip render ediyor.
- [ ] `AppManager` gerçekten dead ise sonraki kod değişikliği sırasında kaldır; tutacaksan aktif entry point ile rolünü belgeleyip tekilleştir.
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
- [x] Kısmi kabul: check adımları ayrı isimlerle raporlanır ve runner hatasında diğerleri de denenir; local/CI ortak komutları hizalı; frontend build, test, locale/import kontrolleri ve checker testleri geçti.
- [ ] Kalan kabul: desteklenen Windows CI çalışması; Cargo check/test'in 20 GiB hedef sınırına uyarak tamamlanması; browser'da Görsel stüdyosu lazy route ve paketlenmiş build karşılaştırması.

### 10.3 CLI, Tauri ve sidecar yaşam döngüsü

- [x] `QA-07/11` kod dilimi: Interrupt sonrası 1,5 sn graceful bekleme, Windows process-tree sonlandırma veya Unix process-group sinyalleme, en çok 3 sn kapanış doğrulaması ve doğrulanamayan süreçte profili koruma eklendi. Normal test penceresi kapanışı Tauri'nin kullanıcı kaynaklı exit isteği, başarılı sidecar kapanışı ve eşleşen tek kullanımlık run marker'ı ile tanınıyor; `0` ve gözlenen Windows `4294967295` yalnız bu marker ile kabul ediliyor.
- [ ] `QA-07/11` runtime kabulü: Gerçek terminal Ctrl+C ile nested Tauri/Vite child tree kapanışını; GUI normal close ile exit kodu/profile cleanup eşleşmesini; crash ve startup failure yollarını ayrı ayrı doğrula.
- [x] `T-HOST-01` kod dilimi: Kaydedilen normal/maximized geometri açılışta geri yükleniyor; boyut/konum monitör çalışma alanına sığıyor, artık bulunmayan monitör birincil monitöre taşınıyor. Geometri hesaplama için altı Rust testi eklendi.
- [ ] `T-HOST-01` runtime kabulü: Farklı çözünürlük/DPI ve monitör çıkarma sonrası gerçek pencerede konum, boyut ve maximize durumunu doğrula.
- [x] `T-HOST-02` kod dilimi: Ana Tauri penceresi gizli başlatılıyor; normal React ekranı ve startup hata ekranı `flushSync(render)` sonrasında ortak `show()` yolunu kullanıyor. `startupWindowVisibility.test.mjs` host görünürlüğü, render/show sırasını ve benchmark görünürlük event'ini sabitliyor.
- [ ] `T-HOST-02` runtime kabulü: Gerçek Windows penceresinde soğuk/yavaş başlangıç ve startup hata ekranında boş/unstyled frame olmadığını gözle doğrula.
- [x] `T-HOST-03` kod dilimi: WebView2 hata sayacı iki ardışık otomatik reload'dan sonra recovery gösteriyor; yalnız ana pencerenin React görünümü render edilip pencere gösterildikten sonra gelen readiness bildirimi sayacı sıfırlıyor. Sayaç doyumlu artıyor; seri, reset ve taşma için Rust testleri eklendi.
- [ ] `T-HOST-03` runtime kabulü: Gerçek Windows WebView2 renderer çökmesini en az üç kez ve araya başarılı renderer hazır olma durumu koyarak tetikle; iki reload, recovery ve sayacın sıfırlanmasını gözle doğrula.
- [x] `T-HOST-04` kod dilimi: Recovery ekranı en son doğrulanmış OpenChat URL'sine klavyeyle kullanılabilen bağlantı sunuyor; yalnız Tauri/loopback adresleri kabul ediliyor ve HTML escape ediliyor. URL yoksa güvenli yeniden başlatma yönergesi gösteriliyor. Recovery sayfası ana pencereyi ve standart başlık/kapatma kontrollerini açıyor; React hazır olunca özel başlık çubuğu geri geliyor. Metinler beş desteklenen dilde.
- [ ] `T-HOST-04` runtime kabulü: Üçüncü gerçek WebView2 renderer çökmesinde recovery penceresinin görünmesini, bağlantının app URL'sine dönmesini, frontend açılışını ve local-service handshake'ini doğrula.
- [x] `T-HOST-05` kod dilimi: Native, renderer ve permission pipe yazımları tek FIFO worker'a taşındı; kuyruk 8 bekleyen mesaj ve 32 MiB toplam frame alanıyla sınırlı. I/O sırasında process/native-response/permission kilitleri tutulmuyor. 10 sn yazım timeout'u sidecar process group'unu kesiyor; kuyruk doluluğu açık backpressure hatası veriyor. FIFO, byte/adet sınırı, ack timeout'u ve Windows process-group kesmesi test edildi.
- [ ] `T-HOST-05` runtime kabulü: Paketlenmiş Windows sidecar'ın stdin'ini durdurup kuyruk doluluğunu, 10 sn timeout sonrası tüm process tree'nin kapanmasını, bekleyen native/permission isteklerinin temizlenmesini ve yeni generation ile tekrar başlatılmasını doğrula.
- [ ] `T-HOST-06` native dialog yaşam döngüsü: Kullanıcı dialog'u sonuçlandırana kadar bekleme beklenen davranış. Mevcut plugin gerçek dialog iptal handle'ı sunmadığı için sabit timeout ekleme; Windows'ta cancel, ana pencere kapanışı ve uygulama kapanışında invoke/archive cleanup davranışını runtime'da doğrula. Gerçek takılma görülürse timeout'tan önce iptal edilebilir dialog entegrasyonunu seç.
- [x] `T-HOST-07` kod dilimi: `Moved`/`Resized` event'leri geometriyi bellekte güncelliyor; tek background writer son event'ten 300 ms sonra coalesced atomic save yapıyor. Normal boyut/konum maximized durumunda korunuyor; minimize edilmiş pencere atlanıyor. Kapanış en son geometriyi worker'a flush edip en fazla 2 sn bekliyor.
- [ ] `T-HOST-07` runtime kabulü: Gerçek Windows penceresinde hızlı drag/resize sırasında UI tepkisini ve diskte son geometriyi doğrula; yavaş/erişilemeyen disk durumunda 2 sn kapanış deadline'ını ve yeniden açılış davranışını gözle.
- [x] `T-HOST-08` kod dilimi: Renderer komutu yalnız sabit app-setting event'ini yayınlıyor; payload tam `key`/`value_json` alanlarından oluşuyor, fazladan alanlar reddediliyor, alanlar 128 B ve 2 MiB ile sınırlandırılıyor. Testler şekil, aşım ve izin verilen üst sınırı kapsıyor.
- [ ] `T-HOST-08` runtime kabulü: Gerçek iki pencerede ayar değişikliğinin alındığını ve bozuk/limit aşan payload'ın yayınlanmadığını doğrula.
- [ ] `T-HOST-08` framework sınırı: Kilitli Tauri 2.12.4 invoke gövdesini komuta ulaşmadan önce genel byte limiti olmadan `serde_json::Value` olarak parse ediyor. Bu komutun typed payload sınırı fan-out'ı kısıtlar; parse öncesi bellek üst sınırı için framework düzeyinde destek/başka host sınırı araştır.
- [ ] `T-HOST-09` pencere state kayıt hatalarını path/secret sızdırmadan gözlemlenebilir yap; açılışta varsayılan geometriye düşme davranışını dokümante et.
- [ ] Kabul: doğrulanmış normal kapanış başarıyla raporlanır; interrupt ve child kapanışı bounded olur; süreç ağacı kapandığı doğrulanınca geçici profil temizlenir, kapanış doğrulanamazsa çalışan süreçten veri silmemek için profil korunur ve yolu raporlanır; dialog iptali ve IPC arızaları doğru, güvenli sonuç verir.

### 10.4 Yerel servis ve veri bütünlüğü

- [ ] `SVC-01..11`: Run durumunun persist edilmesi, conversation bulunamadı sözleşmesi, attachment dosyası/DB atomikliği ve temizliği, MIME/GIF tutarlılığı, upsert, envelope limiti/prefix'i ve uzun transcript pagination'ını konu bazında düzelt.
- [ ] `SVC-12/13`: Archive restore ve security scan silme için crash-safe cleanup/reconciliation sözleşmesi belirle; DB ile dosya sistemi sonuçlarını tekrar denenebilir hale getir.
- [ ] `SVC-14..17/20`: Operation cancel, timeout, progress/status ve yeniden deneme davranışını security scan, worktree listeleme, attachment index ve archive işlemlerinde hizala.
- [ ] `SVC-16`: Worktree limitini backend'de deterministik sırayla uygula; gizli kalan kayıtları kullanıcıya bildir veya yönetilebilir kıl.
- [ ] Kabul: dosya/DB tutarlılığı her hata noktasında korunur veya kalıcı reconciliation kaydı kalır; timeout sonrası işin sürüp sürmediği ve iptal sonucu gözlenebilir; uzun listelerde kesilme açıkça bildirilir.

### 10.5 Dış erişim ve yerel model sınırı

- [ ] `CROSS-02/03/06`: Farklı Origin, CORS preflight, PNA, DNS rebinding, HMR ve LAN davranışlarını kontrollü ortamda ölç; aktif llama.cpp port keşfini, boş port seçimi/child başlatma yarışını ve paketlenmiş dinleyici envanterini doğrula.
- [ ] Ölçüm sonucuna göre Vite dev server ve yerel model API'si için en dar gerekli erişim sınırını seç; gerekiyorsa auth, origin kısıtı veya Tauri üzerinden kontrollü proxy tasarımını uygula.
- [ ] Kabul: hangi yerel/dış originin hangi endpoint'e erişebildiği kanıtla kayıtlıdır; aynı makine testi LAN güvenliği kanıtı olarak kullanılmaz; paketlenmiş sürümde beklenmeyen listener kalmaz.

### 10.6 Kullanıcı akışları, erişilebilirlik ve kurtarma

- [ ] `FE-01..19`: Sohbet, MCP filtreleri, provider/proje formları, kısayollar, odak/combobox, loading metinleri, prompt retry, recent-search retention ve indirme progress etiketlerini tek kullanıcı akışı başına ele al.
- [ ] `SVC-18/19/21/22`, `QA-12`, `T-HOST-01..04/07`: Canvas kurtarma, tercih patch merge, saat girişi, kullanılmayan history helper ve `AppManager` rolü için ürün kararı ver; gerekli olan davranışı regresyon testleriyle doğrula.
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
