// Оценка через OpenAI-совместимый API: OpenAI, OpenRouter, Ollama, LM Studio и другие.
import { z } from 'zod';
import { invoke, isDesktop } from '../platform.ts';
import { SYSTEM_PROMPT, userMessage } from './assessment.ts';
import { Assessment } from './schema.ts';
import { LlmError, provider, type Request } from './providers.ts';

const SCHEMA = (() => {
  const { $schema: _, ...schema } = z.toJSONSchema(Assessment);
  return schema;
})();

function headers(req: Request): Record<string, string> {
  const h: Record<string, string> = { 'Content-Type': 'application/json' };
  if (req.apiKey.trim()) h.Authorization = `Bearer ${req.apiKey.trim()}`;
  return h;
}

interface Raw {
  status: number;
  body: string;
}

/**
 * В браузере — fetch. В настольном приложении запрос делает Rust: так не мешают
 * ни CORS (Ollama не пускает чужие источники), ни CSP. Отменить его нельзя,
 * поэтому отменённый ответ отбрасывает вызывающий код.
 */
async function send(url: string, req: Request, body?: string, signal?: AbortSignal): Promise<Raw> {
  const method = body === undefined ? 'GET' : 'POST';
  if (isDesktop) {
    return invoke<Raw>('http_request', { request: { method, url, headers: headers(req), body } });
  }
  const response = await fetch(url, { method, headers: headers(req), body, signal });
  return { status: response.status, body: await response.text() };
}

async function call(path: string, req: Request, body?: string, signal?: AbortSignal): Promise<unknown> {
  let raw: Raw;
  try {
    raw = await send(`${req.baseUrl.trim().replace(/\/$/, '')}${path}`, req, body, signal);
  } catch (e) {
    if (e instanceof Error && e.name === 'AbortError') throw e;
    // Rust возвращает готовое сообщение строкой.
    if (isDesktop) throw new LlmError(String(e));
    const p = provider(req.provider);
    throw new LlmError(
      p.local
        ? `Не удалось соединиться с ${req.baseUrl}. Запущен ли сервер? Для Ollama на опубликованном сайте ` +
            'нужно разрешить его адрес в переменной OLLAMA_ORIGINS.'
        : `Не удалось соединиться с ${p.title}. Проверьте подключение к интернету.`,
    );
  }
  if (raw.status < 200 || raw.status >= 300) {
    if (raw.status === 401) throw new LlmError('Ключ API не подошёл.');
    if (raw.status === 403) throw new LlmError('У ключа нет доступа к этой модели.');
    if (raw.status === 404) throw new LlmError('Модель или адрес API не найдены.');
    if (raw.status === 429) throw new LlmError('Превышен лимит запросов. Подождите и повторите.');
    throw new LlmError(`Ошибка API (${raw.status}): ${raw.body.slice(0, 300)}`);
  }
  try {
    return JSON.parse(raw.body);
  } catch {
    throw new LlmError('Сервер вернул не JSON. Проверьте адрес API.');
  }
}

const Completion = z.object({
  choices: z
    .array(z.object({ message: z.object({ content: z.string().nullable() }), finish_reason: z.string().nullish() }))
    .min(1),
});

export async function assessWithOpenAI(text: string, req: Request, signal?: AbortSignal): Promise<Assessment> {
  const body = {
    model: req.model,
    messages: [
      { role: 'system', content: SYSTEM_PROMPT },
      { role: 'user', content: userMessage(text) },
    ],
    response_format: { type: 'json_schema', json_schema: { name: 'assessment', strict: true, schema: SCHEMA } },
  };
  const data = await call('/chat/completions', req, JSON.stringify(body), signal);
  const completion = Completion.safeParse(data);
  if (!completion.success) throw new LlmError('Сервер вернул неожиданный ответ.');
  const choice = completion.data.choices[0];
  if (choice.finish_reason === 'length') {
    throw new LlmError('Ответ модели оборвался: текст слишком длинный для этой модели.');
  }
  return parseAssessment(choice.message.content ?? '');
}

/** Модели без строгого режима иногда оборачивают JSON в ```json … ```. */
export function parseAssessment(content: string): Assessment {
  const json = content.replace(/^\s*```(?:json)?\s*/i, '').replace(/\s*```\s*$/, '');
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    throw new LlmError('Модель ответила не в формате JSON. Попробуйте другую модель.');
  }
  const parsed = Assessment.safeParse(value);
  if (!parsed.success) throw new LlmError('Модель ответила не по схеме. Попробуйте ещё раз или другую модель.');
  return parsed.data;
}

const ModelList = z.object({ data: z.array(z.object({ id: z.string() })) });

export async function listOpenAIModels(req: Request, signal?: AbortSignal): Promise<string[]> {
  const data = await call('/models', req, undefined, signal);
  const parsed = ModelList.safeParse(data);
  if (!parsed.success) throw new LlmError('Сервер вернул неожиданный список моделей.');
  return parsed.data.data.map((m) => m.id).sort();
}
