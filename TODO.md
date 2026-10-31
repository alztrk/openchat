# TODO — Architecture Migration

> **Durum:** Faz 5 devam ediyor (Entity Layer)  
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

## Faz 2-3 — Chat Modülü + Diğer Modüller ✅ TAMAMLANDI (Commit: a2f5851)

**Amac:** `modules/conversations/` → `features/chat/` ve diğer modüller → `features/`

### 2.1 Directory
- [x] `features/chat/components/` — chat UI komponentleri
- [x] `features/chat/data/` — chat data files
- [x] `features/chat/state/` — Zustand stores (15 store → organized)
- [x] `features/chat/hooks/` — chat hooks
- [x] `features/chat/services/` — placeholder
- [x] `features/chat/adapters/` — placeholder
- [x] `features/chat/index.ts` — barrel export

### 2.2 Diğer Modüller
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
- [x] tsconfig.tsbuildinfo cache temizlendi

---

## Faz 5 — Entity Layer 🔄 DEVAM EDİYOR

**Amac:** Common tipleri `entities/` altına taşı

- [ ] `entities/conversation/` — ChatConversation, ChatMessage vb.
- [ ] `entities/model/` — ChatModel, ChatModelProvider
- [ ] `entities/provider/` — Provider types
- [ ] `entities/project/` — ChatProject vb.
- [ ] Tüm type import'larını `entities/` yap
- [ ] TypeScript check: clean

---

## Faz 6 — Rust Sidecar Modularization

**Amac:** `domain/` klasörünü Cargo workspace'a bölmek

- [ ] `crates/domain/` — domain types (minimal deps)
- [ ] `crates/usecases/` — use-case orchestrators
- [ ] `crates/transport/` — NDJSON RPC
- [ ] `crates/adapters/` — DB, sandbox, credentials
- [ ] Dependency graph: `transport → usecases → domain`, `adapters → domain`
- [ ] `cargo check` — tüm crate'lar build edebiliyor
- [ ] `cargo test` — tüm test'ler passing

---

## Faz 7 — CI Gates + Dokümantasyon

- [ ] Import lint rules (biome.json)
- [ ] `npm run openchat -- check-imports` script
- [ ] CI/CD gates
- [ ] `AGENTS.md` güncelle
