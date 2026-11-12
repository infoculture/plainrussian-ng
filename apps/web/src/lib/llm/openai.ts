// Оценка и переписывание через OpenAI-совместимый API: OpenAI, OpenRouter, GigaChat,
// Ollama, LM Studio и другие.
import { z } from 'zod';
import { invoke, isDesktop } from '../platform.ts';
import { SYSTEM_PROMPT, userMessage } from './assessment.ts';
import { Assessment } from './schema.ts';
import { LlmError, provider, runsLocally, type ProviderId, type Request } from './providers.ts';
import type { Prompt } from './rewrite.ts';

const SCHEMA = (() => {
  const { $schema: _, ...schema } = z.toJSONSchema(Assessment);
  return schema;
})();

/**
 * Content-Type только у запроса с телом: GET без него и без ключа браузер шлёт
 * сразу, без предварительного OPTIONS, который LM Studio без CORS не понимает.
 */
function headers(req: Request, json: boolean): Record<string, string> {
  const h: Record<string, string> = json ? { 'Content-Type': 'application/json' } : {};
  if (req.apiKey.trim()) h.Authorization = `Bearer ${req.apiKey.trim()}`;
  return h;
}

/** Что мешает браузеру достучаться до локального сервера, кроме того, что он не запущен. */
const CORS_HINTS: Partial<Record<ProviderId, string>> = {
  lmstudio:
    'Если запущен, включите CORS: в LM Studio на вкладке Developer откройте настройки сервера ' +
    'и включите Enable CORS (или запустите сервер командой lms server start --cors).',
  ollama: 'Для Ollama на опубликованном сайте нужно разрешить его адрес в переменной OLLAMA_ORIGINS.',
};

interface Raw {
  status: number;
  body: string;
}

/**
 * В браузере — fetch. В настольном приложении запрос делает Rust: так не мешают
 * ни CORS (Ollama не пускает чужие источники), ни CSP. Отменить его нельзя,
 * поэтому отменённый ответ отбрасывает вызывающий код.
 */
async function send(path: string, req: Request, body?: string, signal?: AbortSignal): Promise<Raw> {
  const method = body === undefined ? 'GET' : 'POST';
  if (req.provider === 'gigachat') {
    // Ключ авторизации обменивается на токен доступа в Rust: src-tauri/src/gigachat.rs.
    const request = { authKey: req.apiKey.trim(), scope: req.scope, method, path, body };
    return invoke<Raw>('gigachat_request', { request });
  }
  const url = `${req.baseUrl.trim().replace(/\/$/, '')}${path}`;
  const h = headers(req, body !== undefined);
  if (isDesktop) {
    return invoke<Raw>('http_request', { request: { method, url, headers: h, body } });
  }
  const response = await fetch(url, { method, headers: h, body, signal });
  return { status: response.status, body: await response.text() };
}

/** Соединиться не удалось: сообщение для пользователя. Отмену пропускает как есть. */
function connectionError(e: unknown, req: Request): unknown {
  if (e instanceof Error && e.name === 'AbortError') return e;
  // Rust возвращает готовое сообщение строкой.
  if (isDesktop) return new LlmError(String(e));
  const p = provider(req.provider);
  return new LlmError(
    runsLocally(p, req.baseUrl)
      ? [`Не удалось соединиться с ${req.baseUrl}. Запущен ли сервер?`, CORS_HINTS[p.id]].filter(Boolean).join(' ')
      : `Не удалось соединиться с ${p.title}. Проверьте подключение к интернету.`,
  );
}

function checkStatus(raw: Raw): void {
  if (raw.status >= 200 && raw.status < 300) return;
  if (raw.status === 401) throw new LlmError('Ключ API не подошёл.');
  if (raw.status === 403) throw new LlmError('У ключа нет доступа к этой модели.');
  if (raw.status === 404) throw new LlmError('Модель или адрес API не найдены.');
  if (raw.status === 429) throw new LlmError('Превышен лимит запросов. Подождите и повторите.');
  throw new LlmError(`Ошибка API (${raw.status}): ${raw.body.slice(0, 300)}`);
}

async function call(path: string, req: Request, body?: string, signal?: AbortSignal): Promise<unknown> {
  let raw: Raw;
  try {
    raw = await send(path, req, body, signal);
  } catch (e) {
    throw connectionError(e, req);
  }
  checkStatus(raw);
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

/** Запрос к /chat/completions; возвращает текст ответа модели. */
async function complete(body: Record<string, unknown>, req: Request, signal?: AbortSignal): Promise<string> {
  return content(await call('/chat/completions', req, JSON.stringify(body), signal));
}

/** Текст ответа из JSON, который вернул сервер вместо потока. */
function completion(json: string): string {
  try {
    return content(JSON.parse(json));
  } catch (e) {
    if (e instanceof LlmError) throw e;
    throw new LlmError('Сервер вернул не JSON. Проверьте адрес API.');
  }
}

function content(data: unknown): string {
  const parsed = Completion.safeParse(data);
  if (!parsed.success) throw new LlmError('Сервер вернул неожиданный ответ.');
  const choice = parsed.data.choices[0];
  if (choice.finish_reason === 'length') {
    throw new LlmError('Ответ модели оборвался: текст слишком длинный для этой модели.');
  }
  return choice.message.content ?? '';
}

export async function assessWithOpenAI(text: string, req: Request, signal?: AbortSignal): Promise<Assessment> {
  const body = {
    model: req.model,
    messages: [
      { role: 'system', content: SYSTEM_PROMPT },
      { role: 'user', content: userMessage(text) },
    ],
    // GigaChat ждёт схему прямо в response_format, остальные — внутри json_schema, как OpenAI.
    response_format:
      req.provider === 'gigachat'
        ? { type: 'json_schema', schema: SCHEMA, strict: true }
        : { type: 'json_schema', json_schema: { name: 'assessment', strict: true, schema: SCHEMA } },
  };
  return parseAssessment(await complete(body, req, signal));
}

/**
 * Переписывание: ответ обычным текстом, без схемы — её понимают не все локальные модели.
 * В браузере ответ приходит потоком и `onAnswer` получает написанное к этому моменту.
 * Запросы настольного приложения делает Rust, и там ответ приходит целиком.
 */
export async function rewriteWithOpenAI(
  prompt: Prompt,
  req: Request,
  signal?: AbortSignal,
  onAnswer?: (answer: string) => void,
): Promise<string> {
  const body = {
    model: req.model,
    messages: [
      { role: 'system', content: prompt.system },
      { role: 'user', content: prompt.user },
    ],
  };
  if (onAnswer && !isDesktop && req.provider !== 'gigachat') return streamChat(body, req, onAnswer, signal);
  return complete(body, req, signal);
}

const Chunk = z.object({
  choices: z
    .array(z.object({ delta: z.object({ content: z.string().nullish() }).nullish(), finish_reason: z.string().nullish() }))
    .optional(),
  error: z.object({ message: z.string() }).optional(),
});

/** Потоковый ответ /chat/completions: события SSE с кусками текста, как у OpenAI. */
async function streamChat(
  body: Record<string, unknown>,
  req: Request,
  onAnswer: (answer: string) => void,
  signal?: AbortSignal,
): Promise<string> {
  const url = `${req.baseUrl.trim().replace(/\/$/, '')}/chat/completions`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: 'POST',
      headers: headers(req, true),
      body: JSON.stringify({ ...body, stream: true }),
      signal,
    });
  } catch (e) {
    throw connectionError(e, req);
  }
  if (!response.ok || !response.body) {
    checkStatus({ status: response.status, body: await response.text() });
    throw new LlmError('Сервер вернул пустой ответ.');
  }
  // Сервер, который не умеет поток, отвечает обычным JSON.
  if (!(response.headers.get('content-type') ?? '').includes('text/event-stream')) {
    return completion(await response.text());
  }
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = '';
  let answer = '';
  let finish: string | null | undefined;
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += value;
    const lines = buffer.split('\n');
    buffer = lines.pop() ?? '';
    for (const line of lines) {
      // Строки-комментарии («: OPENROUTER PROCESSING») и пустые пропускаем.
      if (!line.startsWith('data:')) continue;
      const data = line.slice(5).trim();
      if (data === '[DONE]') continue;
      let parsed;
      try {
        parsed = Chunk.safeParse(JSON.parse(data));
      } catch {
        continue;
      }
      if (!parsed.success) continue;
      if (parsed.data.error) throw new LlmError(`Ошибка API: ${parsed.data.error.message.slice(0, 300)}`);
      const choice = parsed.data.choices?.[0];
      if (choice?.delta?.content) {
        answer += choice.delta.content;
        onAnswer(answer);
      }
      if (choice?.finish_reason) finish = choice.finish_reason;
    }
  }
  if (finish === 'length') throw new LlmError('Ответ модели оборвался: текст слишком длинный для этой модели.');
  return answer;
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
