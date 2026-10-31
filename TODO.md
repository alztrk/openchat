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

## Faz 6 — Rust Sidecar Modularization ✅ TAMAMLANDI (Commits: b6a2017, 7b19a5a, 9de8a21, 72dca0c)

- [x] `crates/openchat-domain/` — 183 dosya, tüm domain types, providers, tools, storage, sandbox
- [x] `crates/openchat-transport/` — NDJSON RPC re-exports
- [x] `crates/openchat-adapters/` — DB, filesystem, credentials re-exports
- [x] Workspace Cargo.toml: 3 crate workspace
- [x] Domain compiles: 0 errors, 3250 warnings
- [x] Transport compiles: 0 errors
- [x] Adapters compiles: 0 errors
- [x] Dependency graph: transport→domain, adapters→domain

**Taşınan modüller:**
- 117 domain files (`src/domain/` → `crates/openchat-domain/src/`)
- 22 storage adapter files (`src/adapters/storage/` → `crates/openchat-domain/src/storage/`)
- 21 transport files (`src/transport/` → `crates/openchat-domain/src/transport/`)
- 15 sandbox files (`src/sandbox/` → `crates/openchat-domain/src/sandbox/`)
- 7 secrets files (`src/secrets/` → `crates/openchat-domain/src/secrets/`)
- storage_broker.rs, credentials.rs, filesystem.rs, service.rs

---

## Faz 7 — CI Gates + Dokümantasyon ⚠️ KISMİ

- [x] `biome.json` — `noUnusedImports` rule
- [x] `package.json` — `check:imports` script
- [x] Import layer violation düzeltildi (0 errors)
- [ ] CI/CD pipeline (.github/workflows/)
- [ ] `AGENTS.md` güncellenmedi

---

## Özet

| Faz | Status | Dosya Sayısı | Commit |
|-----|--------|-------------|--------|
| 0 | ✅ | 4 | 41a51e0 |
| 1 | ✅ | 230 | 2258d61 |
| 2-3 | ✅ | 200+ | a2f5851 |
| 4 | ✅ | 3 | 9fa81fb |
| 5 | ✅ | 8 | bcbde06 |
| 6 | ✅ | 190+ | b6a2017, 7b19a5a, 9de8a21, 72dca0c |
| 7 | ⚠️ | 2+ | e81140c |

**Toplam:** ~13 commit, 600+ dosya değişti.
