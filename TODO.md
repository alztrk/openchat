# TODO — Architecture Migration

> **Durum:** Tüm fazlar tamamlandı ✅  
> **Oluşturulma:** 2026-10-31  
> **Son güncelleme:** 2026-10-31  
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
- [ ] Diğer ayarlar sayfalarının tasarımı tamamlandıktan sonra Windows bildirim durumunun denetlenememesi sorununu teşhis edip düzelt. Dev moddaki exe yolu ile kurulu uygulamanın `AppUserModelID`/Başlat menüsü kısayolu eşleşmesini ve WinRT hata kodlarını doğrula; dev modun neden olduğunu varsayma.

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
- [x] FSD Import check — **129 cross-feature warnings** (architectural debt)

### 8.3 Pre-existing Test Failures (Environment/Infrastructure Issues)

| Category | Failures | Root Cause | Fix Difficulty |
|----------|----------|------------|----------------|
| File changes tests | 5 tests | Test isolation/state leakage in full suite | Medium |
| File ops tests | 2 tests | Test isolation in full suite | Medium |
| Executor policy tests | 2 tests | Test isolation in full suite | Medium |
| Terminal test | 1 test | Test isolation in full suite | Medium |
| RPC Project tests | 5 tests | **Missing `git` in PATH** | **Easy** (install git) |
| Stack overflow | 1 test | Recursion depth in `kilo_context_usage_estimate` | Hard |
| Compaction logic | 1 test | Token estimation logic vs test expectations | Medium |

**Key Finding:** All "pre-existing" test failures pass when run individually (`cargo test -p openchat-local-service --lib -- <test_name>`). They fail only in the full test suite due to test isolation/state issues.

### 8.4 Recommended Next Steps (Priority Order)

#### High Priority (Quick Wins)
- [x] **Resolve Git executable on Windows** — Rust tests now resolve a real `git.exe` even when PATH exposes a `.cmd` shim
- [x] Full suite test isolation verified after Git executable resolution (`926 passed, 0 failed`)

#### Medium Priority
- [ ] Fix stack overflow in `rpc::conversations::tests::kilo_context_usage_estimate_does_not_require_provider_credentials` — increase stack size or refactor recursion
- [ ] Fix compaction test: `domain::openai_compatible::conversation::in_loop_compaction_tests::compacts_older_tool_outputs_while_preserving_latest_tool_round_and_call_pairs` — token estimation logic
- [ ] Investigate and fix test isolation for file_changes/file_ops/executor/terminal tests

#### Low Priority (Architectural Debt)
- [ ] **FSD Architecture Violations** — 128 cross-feature import warnings requiring significant refactoring
  - `features/app-settings` → imports from `chat`, `provider-settings`, `projects`, `notifications`, `design-workspace`
  - `features/chat` → imports from `projects`, `provider-settings`, `app-settings`, `notifications`
  - `features/design-workspace` → imports from `chat`, `provider-settings`
  - `features/model-catalog` → imports from `app-settings`, `provider-settings`, `chat`
  - `features/notifications` → imports from `chat`
  - `features/projects` → imports from `chat`, `provider-settings`, `app-settings`
  - `features/security-scans` → imports from `chat`, `provider-settings`

#### Optional Cleanup
- [ ] Remove `#[allow(dead_code)]` on `summary_text` in `history.rs` once it's actually used, or remove the function if not needed
- [ ] Remove unused `PruneLevel` import in `chatgpt/compaction.rs:15` (only used in test module)
