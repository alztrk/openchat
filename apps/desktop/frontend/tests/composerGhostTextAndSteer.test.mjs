import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

function detectGhostTextSuggestion(messages) {
  if (!messages || messages.length === 0) return null;

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    if (msg?.role !== 'assistant') continue;
    if (msg.status !== 'completed') continue;

    const content = msg.content.trim();
    if (!content) continue;

    const lower = content.toLowerCase();

    if (
      lower.includes('hata') ||
      lower.includes('error') ||
      lower.includes('failed') ||
      lower.includes('failure') ||
      lower.includes('exception') ||
      lower.includes('başarısız')
    ) {
      return {
        key: 'ghostSuggestionFixError',
        fallbackText: 'Bu hatayı düzelt ve tekrar dene',
      };
    }

    if (
      lower.includes('test') ||
      lower.includes('vitest') ||
      lower.includes('jest') ||
      lower.includes('pytest') ||
      lower.includes('cargo test') ||
      lower.includes('npm test') ||
      lower.includes('doğrula')
    ) {
      return {
        key: 'ghostSuggestionRunTests',
        fallbackText: 'Testleri çalıştır ve sonucu kontrol et',
      };
    }

    if (
      lower.includes('değişiklik') ||
      lower.includes('proposed') ||
      lower.includes('uygula') ||
      lower.includes('implement') ||
      lower.includes('plan') ||
      lower.includes('```') ||
      /(?:^|\n)\s*(?:1\.|-\s*\[\s*\])/m.test(content)
    ) {
      return {
        key: 'ghostSuggestionApplyChanges',
        fallbackText: 'Önerilen değişiklikleri uygula',
      };
    }

    if (
      lower.includes('sonraki') ||
      lower.includes('next step') ||
      lower.includes('devam') ||
      lower.includes('ilerle') ||
      lower.includes('hazır')
    ) {
      return {
        key: 'ghostSuggestionContinueNext',
        fallbackText: 'Sonraki adıma devam et',
      };
    }

    return {
      key: 'ghostSuggestionExplainDetails',
      fallbackText: 'Daha fazla detay açıkla',
    };
  }

  return null;
}

test('detectGhostTextSuggestion returns null for empty or incomplete messages', () => {
  assert.equal(detectGhostTextSuggestion([]), null);
  assert.equal(detectGhostTextSuggestion(null), null);
  assert.equal(detectGhostTextSuggestion([{ role: 'user', content: 'Merhaba', status: 'completed' }]), null);
  assert.equal(
    detectGhostTextSuggestion([{ role: 'assistant', content: 'Çalışıyorum...', status: 'streaming' }]),
    null,
  );
});

test('detectGhostTextSuggestion recognizes test execution context', () => {
  const messages = [
    { role: 'user', content: 'Değişiklikleri yaptın mı?', status: 'completed' },
    {
      role: 'assistant',
      content: 'Kod değişiklikleri tamamlandı. Şimdi npm test komutunu çalıştırabiliriz.',
      status: 'completed',
    },
  ];
  const suggestion = detectGhostTextSuggestion(messages);
  assert.ok(suggestion);
  assert.equal(suggestion.key, 'ghostSuggestionRunTests');
});

test('detectGhostTextSuggestion recognizes error fix context', () => {
  const messages = [
    { role: 'user', content: 'Derleme nasıl gitti?', status: 'completed' },
    {
      role: 'assistant',
      content: 'TypeScript derlemesi sırasında syntax error hatası ile karşılaşıldı.',
      status: 'completed',
    },
  ];
  const suggestion = detectGhostTextSuggestion(messages);
  assert.ok(suggestion);
  assert.equal(suggestion.key, 'ghostSuggestionFixError');
});

test('detectGhostTextSuggestion recognizes code or plan application context', () => {
  const messages = [
    { role: 'user', content: 'Bunu nasıl çözeriz?', status: 'completed' },
    {
      role: 'assistant',
      content: 'İşte yapacağımız plan:\n1. Dosyayı düzenle\n2. Bağımlılıkları kur',
      status: 'completed',
    },
  ];
  const suggestion = detectGhostTextSuggestion(messages);
  assert.ok(suggestion);
  assert.equal(suggestion.key, 'ghostSuggestionApplyChanges');
});

test('detectGhostTextSuggestion recognizes next step progression', () => {
  const messages = [
    { role: 'user', content: 'İlk adım bitti mi?', status: 'completed' },
    {
      role: 'assistant',
      content: 'İlk adım hazır, her şey sorunsuz ilerledi.',
      status: 'completed',
    },
  ];
  const suggestion = detectGhostTextSuggestion(messages);
  assert.ok(suggestion);
  assert.equal(suggestion.key, 'ghostSuggestionContinueNext');
});

test('detectGhostTextSuggestion defaults to explaining in more detail for general responses', () => {
  const messages = [
    { role: 'user', content: 'React hookları nedir?', status: 'completed' },
    {
      role: 'assistant',
      content: 'React hookları fonksiyonel bileşenlerde durum ve yaşam döngüsünü yönetmenizi sağlar.',
      status: 'completed',
    },
  ];
  const suggestion = detectGhostTextSuggestion(messages);
  assert.ok(suggestion);
  assert.equal(suggestion.key, 'ghostSuggestionExplainDetails');
});

test('all 5 locales define all required ghost-text and live-steer keys', () => {
  const locales = ['en', 'tr', 'de', 'es', 'fr'];
  const requiredKeys = [
    'composerGhostTextEnabled',
    'composerGhostTextEnabledDescription',
    'composerGhostTextModel',
    'composerGhostTextModelDescription',
    'composerGhostTextModelAuto',
    'composerSteerResponse',
    'ghostSuggestionRunTests',
    'ghostSuggestionApplyChanges',
    'ghostSuggestionExplainDetails',
    'ghostSuggestionFixError',
    'ghostSuggestionContinueNext',
  ];

  for (const locale of locales) {
    const filePath = path.resolve('src/localization/locales', `${locale}.json`);
    const content = JSON.parse(fs.readFileSync(filePath, 'utf-8'));

    for (const key of requiredKeys) {
      assert.ok(
        typeof content[key] === 'string' && content[key].trim().length > 0,
        `Locale "${locale}" is missing or has empty key: "${key}"`,
      );
    }
  }
});
