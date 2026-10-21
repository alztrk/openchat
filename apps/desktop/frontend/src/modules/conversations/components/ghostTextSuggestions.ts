import type { ChatMessage } from '@/modules/conversations/data/chatStorage';

export interface GhostTextCandidate {
  key:
    | 'ghostSuggestionRunTests'
    | 'ghostSuggestionApplyChanges'
    | 'ghostSuggestionExplainDetails'
    | 'ghostSuggestionFixError'
    | 'ghostSuggestionContinueNext';
  fallbackText: string;
}

/**
 * Evaluates the conversation messages and extracts a high-probability follow-up candidate
 * based on the most recent completed assistant response.
 */
export function detectGhostTextSuggestion(
  messages: readonly ChatMessage[] | null | undefined,
): GhostTextCandidate | null {
  if (!messages || messages.length === 0) return null;

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    if (!msg || msg.role !== 'assistant') continue;
    if (msg.status !== 'completed') continue;

    const content = msg.content.trim();
    if (!content) continue;

    const lower = content.toLowerCase();

    // 1. Errors, exceptions, or failures
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

    // 2. Tests, validation suites, test execution
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

    // 3. Proposed changes, plans, numbered steps, code implementations
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

    // 4. Next steps, continuation, progression
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

    // 5. Default detailed explanation
    return {
      key: 'ghostSuggestionExplainDetails',
      fallbackText: 'Daha fazla detay açıkla',
    };
  }

  return null;
}
