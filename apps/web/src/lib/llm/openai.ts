// Оценка через OpenAI-совместимый API: OpenAI, OpenRouter, Ollama, LM Studio.
import { z } from 'zod';
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

async function call(url: string, init: RequestInit, req: Request): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(url, init);
  } catch (e) {
    if ((e as Error).name === 'AbortError') throw e;
    const p = provider(req.provider);
    throw new LlmError(
      p.local
        ? `Не удалось соединиться с ${req.baseUrl}. Запущен ли сервер? Для Ollama на опубликованном сайте ` +
            'нужно разрешить его адрес в переменной OLLAMA_ORIGINS.'
        : `Не удалось соединиться с ${p.title}. Проверьте подключение к интернету.`,
    );
  }
  if (!response.ok) {
    const detail = (await response.text()).slice(0, 300);
    if (response.status === 401) throw new LlmError('Ключ API не подошёл.');
    if (response.status === 403) throw new LlmError('У ключа нет доступа к этой модели.');
    if (response.status === 404) throw new LlmError('Модель или адрес API не найдены.');
    if (response.status === 429) throw new LlmError('Превышен лимит запросов. Подождите и повторите.');
    throw new LlmError(`Ошибка API (${response.status}): ${detail}`);
  }
  return response.json();
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
  const data = await call(
    `${req.baseUrl.replace(/\/$/, '')}/chat/completions`,
    { method: 'POST', headers: headers(req), body: JSON.stringify(body), signal },
    req,
  );
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
  const data = await call(`${req.baseUrl.replace(/\/$/, '')}/models`, { headers: headers(req), signal }, req);
  const parsed = ModelList.safeParse(data);
  if (!parsed.success) throw new LlmError('Сервер вернул неожиданный список моделей.');
  return parsed.data.data.map((m) => m.id).sort();
}
