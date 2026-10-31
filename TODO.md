# TODO — Architecture Migration

> **Durum:** Tüm fazlar tamamlandı  
> **Oluşturulma:** 2026-10-31  
> **Son güncelleme:** 2026-10-31  
> **Amaç:** OpenChat frontend ve Rust sidecar'ın endüstri standardına (FSD + Modular Monolith) incremental geçiş  

---

## Faz 0 — Hazırlık ✅ TAMAMLANDI (Commit: 41a51e0)

### 0.1 TypeScript Config
- [x] `tsconfig.json` — 5 yeni alias eklendi (`@app/*`, `@features/*`, `@entities/*`, `@widgets/*`, `@shared/*`)
- [x] `vite.config.ts` — 5 yeni alias eklendi

### 0.2 Lint Rules
- [x] `scripts/check-imports.mjs` yazıldı (import layer violation detection)
- [x] ESM `__dirname` düzeltildi (`fileURLToPath`)

### 0.3 Directory Structure
- [x] `src/app/` oluşturuldu
- [x] `src/features/` oluşturuldu
- [x] `src/entities/` oluşturuldu
- [x] `src/widgets/` oluşturuldu
- [x] `src/shared/` (zaten vardı)

---

## Faz 1 — Shared Layer Temizliği ✅ TAMAMLANDI (Commit: 2258d61)

**Amac:** `design-system`, `platform`, `shared/utils`, `localization` → `@shared/` altına taşı.

- [x] `src/design-system/` → `src/shared/components/` (22 components + styles)
- [x] `src/platform/` → `src/shared/lib/platform/` (local-service, storage, tauri)
- [x] `src/shared/utils/` → `src/shared/lib/utils/` (6 files)
- [x] `src/shared/hooks/` → `src/shared/lib/hooks/` (1 file)
- [x] `src/localization/` → `src/shared/lib/i18n/` (i18n, locales, resources)
- [x] 131 import path güncellendi
- [x] TypeScript check: clean (0 errors)
- [x] Eski dizinler temizlendi (design-system/, platform/, localization/)

---

## Faz 2-3 — Modüller → Features Vertical Slice ✅ TAMAMLANDI (Commit: a2f5851)

**Amac:** `modules/conversations/` → `features/chat/` ve diğer modüller → `features/`

- [x] `modules/conversations/` → `features/chat/` (40+ components, 21 data, 14 stores, 5 hooks)
- [x] `modules/providers/` → `features/provider-settings/` (19 files)
- [x] `modules/settings/` → `features/app-settings/` (41 files)
- [x] `modules/design/` → `features/design-workspace/` (9 files)
- [x] `modules/model-library/` → `features/model-catalog/` (12 files)
- [x] `modules/projects/` → `features/projects/` (16 files)
- [x] `modules/notifications/` → `features/notifications/` (3 files)
- [x] `modules/security-scans/` → `features/security-scans/` (4 files)
- [x] Tüm imports güncellendi (200+ dosya)
- [x] TypeScript check: clean (0 errors)
- [x] Eski modules/ dizini silindi

---

## Faz 4 — App.tsx Küçültme ✅ TAMAMLANDI (Commit: 9fa81fb)

**Amac:** App.tsx'i parçalara böl (1277 → ~300 satır)

- [x] `app/AppRouter.tsx` — surface routing (~87 lines)
- [x] `app/AppManager.tsx` — global event management (~849 lines)
- [x] `app/AppLayout.tsx` — layout management (~180 lines)
- [x] TypeScript check: clean (0 errors)

---

## Faz 5 — Entity Layer ✅ TAMAMLANDI (Commit: bcbde06)

**Amac:** Common tipleri `entities/` altına taşı

- [x] `entities/conversation/` — ChatConversation, ChatMessage, ChatModel, ChatProject vb.
- [x] `entities/model/` — ChatModel, ChatModelProvider
- [x] `entities/provider/` — (placeholder, reserved)
- [x] `entities/project/` — ToolPermissionMode
- [x] `entities/index.ts` — barrel exports
- [x] TypeScript check: clean (0 errors)

---

## Faz 6 — Rust Sidecar Modularization ✅ TAMAMLANDI (Commit: b6a2017)

**Amac:** `domain/` klasörünü Cargo workspace'a bölmek

- [x] Workspace root Cargo.toml düzenlendi (members listesi güncellendi)
- [x] `crates/openchat-domain/` — domain types (serde, uuid, thiserror)
- [x] `crates/openchat-transport/` — NDJSON RPC protocol skeleton
- [x] `crates/openchat-adapters/` — DB, filesystem, credentials skeleton
- [x] Dependency graph: transport→domain, adapters→domain
- [x] `cargo check -p openchat-domain` — compiles clean (0 errors)
- [x] Transport ve adapters skeleton oluşturuldu

---

## Faz 7 — CI Gates + Dokümantasyon ✅ TAMAMLANDI (Commit: e81140c)

**Amac:** Import lint + biome rules + CI gates

- [x] `biome.json` — `noUnusedImports` rule eklendi
- [x] `package.json` — `check:imports` script eklendi
- [x] `scripts/check-imports.mjs` — ESM uyumlu (`fileURLToPath`)
- [x] Shared-layer violation düzeltildi (audioFeedback.ts → features/notifications/)
- [x] TypeScript check: clean (0 errors)
- [x] Import check: 0 errors, 355 cross-feature warnings (expected for modular monolith)

---

## Sonuç

**7/7 faz tamamlandı.** 13 commit ile:

| Phase | Files Changed | Description |
|-------|---------------|-------------|
| 0 | 4 | Path aliases, check-imports script, empty dirs |
| 1 | 230 | Shared layer consolidation (design-system, platform, i18n, utils, hooks) |
| 2-3 | 200+ | Modules → Features vertical slice migration |
| 4 | 3 | App.tsx splitting (AppRouter, AppManager, AppLayout) |
| 5 | 8 | Entity layer (type re-exports) |
| 6 | 9 | Cargo workspace with domain/transport/adapters crates |
| 7 | 12 | Lint rules, import fix, check:imports script |

**Import Layer Compliance:**
- 🔴 0 errors (shared/ → features/ violation fixed)
- 🟡 355 cross-feature warnings (expected for modular monolith)
- ℹ️ 0 migration info messages (all @modules → @features migrated)
