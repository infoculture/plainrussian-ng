// Оценка через Claude: официальный SDK, структурированный вывод по схеме Assessment.
import Anthropic from '@anthropic-ai/sdk';
import { betaZodOutputFormat } from '@anthropic-ai/sdk/helpers/beta/zod';
import { SYSTEM_PROMPT, userMessage } from './assessment.ts';
import { Assessment } from './schema.ts';
import { LlmError, type Request } from './providers.ts';

// Для этих моделей при отказе по соображениям безопасности API само повторит
// запрос на рекомендованной модели (fallbacks: 'default').
const SERVER_FALLBACK = new Set(['claude-fable-5-1', 'claude-opus-5-5', 'claude-opus-5', 'claude-sonnet-5-5']);

// Эти модели понимают output_config.effort; на старых параметр вызвал бы ошибку.
const EFFORT = new Set([
  'claude-fable-5-1', 'claude-fable-5', 'claude-opus-5-5', 'claude-opus-5', 'claude-opus-4-8',
  'claude-opus-4-7', 'claude-sonnet-5-5', 'claude-sonnet-5', 'claude-haiku-5-5',
]);

export async function assessWithAnthropic(
  text: string,
  req: Request,
  signal?: AbortSignal,
): Promise<Assessment> {
  // Ключ пользователя, запрос прямо из браузера: SDK добавит заголовок
  // anthropic-dangerous-direct-browser-access.
  const client = new Anthropic({ apiKey: req.apiKey, dangerouslyAllowBrowser: true, maxRetries: 1 });
  const fallback = SERVER_FALLBACK.has(req.model);
  try {
    const response = await client.beta.messages.parse(
      {
        model: req.model,
        max_tokens: 16000,
        ...(fallback ? { betas: ['server-side-fallback-2026-07-01'], fallbacks: 'default' as const } : {}),
        output_config: {
          // Оценка читаемости — не самая трудная задача, medium хватает.
          ...(EFFORT.has(req.model) ? { effort: 'medium' as const } : {}),
          format: betaZodOutputFormat(Assessment),
        },
        system: SYSTEM_PROMPT,
        messages: [{ role: 'user', content: userMessage(text) }],
      },
      { signal },
    );
    if (response.stop_reason === 'refusal') {
      throw new LlmError('Модель отказалась оценивать этот текст.');
    }
    if (response.stop_reason === 'max_tokens') {
      throw new LlmError('Ответ модели оборвался: текст слишком длинный для одного запроса.');
    }
    if (!response.parsed_output) {
      throw new LlmError('Модель ответила не по схеме. Попробуйте ещё раз.');
    }
    return response.parsed_output;
  } catch (e) {
    throw explain(e, req.model);
  }
}

function explain(e: unknown, model: string): unknown {
  if (e instanceof LlmError || e instanceof Anthropic.APIUserAbortError) return e;
  if (e instanceof Anthropic.AuthenticationError) return new LlmError('Ключ API не подошёл.');
  if (e instanceof Anthropic.PermissionDeniedError) return new LlmError('У ключа нет доступа к этой модели.');
  if (e instanceof Anthropic.NotFoundError) return new LlmError(`Модель «${model}» не найдена. Проверьте название.`);
  if (e instanceof Anthropic.RateLimitError) return new LlmError('Превышен лимит запросов. Подождите и повторите.');
  if (e instanceof Anthropic.BadRequestError) return new LlmError(`Запрос отклонён: ${e.message}`);
  if (e instanceof Anthropic.APIConnectionError) {
    return new LlmError('Не удалось соединиться с API Anthropic. Проверьте подключение к интернету.');
  }
  if (e instanceof Anthropic.APIError) return new LlmError(`Ошибка API Anthropic (${e.status}): ${e.message}`);
  return e;
}
