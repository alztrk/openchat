# TODO — Architecture Migration

> **Durum:** Faz 0 başladı  
> **Oluşturulma:** 2026-10-31  
> **Son güncelleme:** 2026-10-31  
> **Amaç:** OpenChat frontend ve Rust sidecar'ın endüstri standardına (FSD + Modular Monolith) incremental geçiş  

---

## Faz 0 — Hazırlık ✅ TAMAMLANDI

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

### Post-Faz 0 Checklist
- [x] TypeScript check — 0 hata
- [x] check-imports.mjs — 0 hata, 46 migration info

---

## Faz 1 — Shared Layer Temizliği 🔄 DEVAM EDİYOR

**Amac:** `design-system`, `platform`, `shared/utils`, `localization` → `@shared/` altına taşı.

### 1.1 Design System → shared/components
- [ ] `src/design-system/` → `src/shared/components/` (tüm dosyalar)
- [ ] Tüm `@/design-system/*` import'larını `@shared/components/*` yap

### 1.2 Platform → shared/lib/platform
- [ ] `src/platform/local-service/` → `src/shared/lib/platform/local-service/`
- [ ] `src/platform/storage/` → `src/shared/lib/platform/storage/`
- [ ] `src/platform/tauri/` → `src/shared/lib/platform/tauri/`
- [ ] Tüm `@/platform/*` import'larını `@shared/lib/platform/*` yap

### 1.3 Shared Utils → shared/lib/utils
- [ ] `src/shared/hooks/` → `src/shared/lib/hooks/`
- [ ] `src/shared/utils/` → `src/shared/lib/utils/`
- [ ] Tüm `@/shared/*` import'larını güncelle

### 1.4 Localization → shared/lib/i18n
- [ ] `src/localization/` → `src/shared/lib/i18n/`
- [ ] Tüm `@/localization/*` import'larını `@shared/lib/i18n/*` yap

### 1.5 Yeni shared/index.ts
- [ ] `src/shared/components/index.ts` (re-export)
- [ ] `src/shared/lib/platform/index.ts` (re-export)
- [ ] `src/shared/lib/hooks/index.ts` (re-export)
- [ ] `src/shared/lib/utils/index.ts` (re-export)
- [ ] `src/shared/lib/i18n/index.ts` (re-export)

---

## Faz 2 — Chat Modülü Vertical Slice

**Amac:** `modules/conversations/` → `features/chat/`

### 2.1 Directory
- [ ] `features/chat/components/` — chat UI komponentleri
- [ ] `features/chat/state/` — Zustand store (15 store → tek store)
- [ ] `features/chat/hooks/` — chat hooks
- [ ] `features/chat/services/` — RPC/API calls
- [ ] `features/chat/adapters/` — platform-specific
- [ ] `features/chat/types.ts`
- [ ] `features/chat/index.ts`

### 2.2 Store Consolidation
- [ ] 15 store → tek `state/chatStore.ts`

### 2.3 Components, Data, Hooks
- [ ] Tüm component import'ları güncelle
- [ ] Tüm data import'ları güncelle
- [ ] Tüm hook import'ları güncelle

### 2.4 Services Layer
- [ ] RPC çağrılarını `services/` altına topla

---

## Faz 3 — Diğer Modüller

- [ ] `modules/providers/` → `features/provider-settings/`
- [ ] `modules/settings/` → `features/app-settings/`
- [ ] `modules/design/` → `features/design-workspace/`
- [ ] `modules/model-library/` → `features/model-catalog/`
- [ ] `modules/projects/` → `features/projects/`
- [ ] `modules/notifications/` → `features/notifications/`
- [ ] `modules/security-scans/` → `features/security-scans/`

---

## Faz 4 — App.tsx Küçültme

- [ ] `app/AppRouter.tsx` — surface routing
- [ ] `app/AppManager.tsx` — global event management
- [ ] `app/AppLayout.tsx` — layout management
- [ ] `app/AppProviders.tsx` — global providers
- [ ] 1277 satır → ~500 satır

---

## Faz 5 — Entity Layer

- [ ] `entities/conversation/` — ChatConversation, ChatMessage vb.
- [ ] `entities/model/` — ChatModel, ChatModelProvider
- [ ] `entities/provider/` — Provider types
- [ ] `entities/project/` — ChatProject vb.
- [ ] Tüm type import'larını `entities/` yap

---

## Faz 6 — Rust Sidecar Modularization

- [ ] `crates/domain/` — domain types (minimal deps)
- [ ] `crates/usecases/` — use-case orchestrators
- [ ] `crates/transport/` — NDJSON RPC
- [ ] `crates/adapters/` — DB, sandbox, credentials

---

## Faz 7 — CI Gates + Dokümantasyon

- [ ] Import lint rules (biome.json)
- [ ] `npm run openchat -- check-imports` script
- [ ] CI/CD gates
- [ ] `AGENTS.md` güncelle
