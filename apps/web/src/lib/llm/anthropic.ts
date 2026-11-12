// Запросы к Claude через официальный SDK: оценка — структурированным выводом по
// схеме Assessment, переписывание — обычным текстом.
import Anthropic from '@anthropic-ai/sdk';
import { betaZodOutputFormat } from '@anthropic-ai/sdk/helpers/beta/zod';
import { SYSTEM_PROMPT, userMessage } from './assessment.ts';
import { Assessment } from './schema.ts';
import { LlmError, type Request } from './providers.ts';
import type { Prompt } from './rewrite.ts';

// Для этих моделей при отказе по соображениям безопасности API само повторит
// запрос на рекомендованной модели (fallbacks: 'default').
const SERVER_FALLBACK = new Set(['claude-fable-5-1', 'claude-opus-5-5', 'claude-opus-5', 'claude-sonnet-5-5']);

// Эти модели понимают output_config.effort; на старых параметр вызвал бы ошибку.
const EFFORT = new Set([
  'claude-fable-5-1', 'claude-fable-5', 'claude-opus-5-5', 'claude-opus-5', 'claude-opus-4-8',
  'claude-opus-4-7', 'claude-sonnet-5-5', 'claude-sonnet-5', 'claude-haiku-5-5',
]);

function client(req: Request): Anthropic {
  // Ключ пользователя, запрос прямо из браузера: SDK добавит заголовок
  // anthropic-dangerous-direct-browser-access.
  return new Anthropic({ apiKey: req.apiKey, dangerouslyAllowBrowser: true, maxRetries: 1 });
}

/** Откат на другую модель при отказе — там, где API его поддерживает. */
function fallback(model: string) {
  return SERVER_FALLBACK.has(model)
    ? { betas: ['server-side-fallback-2026-07-01'], fallbacks: 'default' as const }
    : {};
}

/** Ни оценка читаемости, ни переписывание — не самые трудные задачи, medium хватает. */
function effort(model: string) {
  return EFFORT.has(model) ? { effort: 'medium' as const } : {};
}

/** Отказ и оборванный ответ — ошибки, понятные пользователю. `task` — «оценивать», «переписывать». */
function checkStop(stopReason: string | null, task: string): void {
  if (stopReason === 'refusal') throw new LlmError(`Модель отказалась ${task} этот текст.`);
  if (stopReason === 'max_tokens') {
    throw new LlmError('Ответ модели оборвался: текст слишком длинный для одного запроса.');
  }
}

export async function assessWithAnthropic(
  text: string,
  req: Request,
  signal?: AbortSignal,
): Promise<Assessment> {
  try {
    const response = await client(req).beta.messages.parse(
      {
        model: req.model,
        max_tokens: 16000,
        ...fallback(req.model),
        output_config: { ...effort(req.model), format: betaZodOutputFormat(Assessment) },
        system: SYSTEM_PROMPT,
        messages: [{ role: 'user', content: userMessage(text) }],
      },
      { signal },
    );
    checkStop(response.stop_reason, 'оценивать');
    if (!response.parsed_output) {
      throw new LlmError('Модель ответила не по схеме. Попробуйте ещё раз.');
    }
    return response.parsed_output;
  } catch (e) {
    throw explain(e, req.model);
  }
}

/**
 * Переписанный текст длиннее оценки, поэтому запрос потоковый: SDK не держит
 * долгий запрос с большим max_tokens без потока. Ответ собирается целиком.
 */
export async function rewriteWithAnthropic(prompt: Prompt, req: Request, signal?: AbortSignal): Promise<string> {
  try {
    const response = await client(req)
      .beta.messages.stream(
        {
          model: req.model,
          max_tokens: 64000,
          ...fallback(req.model),
          output_config: effort(req.model),
          system: prompt.system,
          messages: [{ role: 'user', content: prompt.user }],
        },
        { signal },
      )
      .finalMessage();
    checkStop(response.stop_reason, 'переписывать');
    return response.content.map((block) => (block.type === 'text' ? block.text : '')).join('');
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
