// Провайдеры LLM по модели BYOK: ключ принадлежит пользователю, запросы идут
// из браузера напрямую провайдеру. Список адресов совпадает с connect-src в CSP
// (vite.config.ts): с другими адресами страница соединиться не может.
import type { Assessment } from './assessment.ts';

export type ProviderId = 'anthropic' | 'openai' | 'openrouter' | 'ollama' | 'lmstudio';

export interface Provider {
  id: ProviderId;
  title: string;
  /** Как с ним говорить: SDK Anthropic или OpenAI-совместимый API. */
  kind: 'anthropic' | 'openai';
  baseUrl: string;
  needsKey: boolean;
  /** Модель работает на этом компьютере, текст никуда не уходит. */
  local: boolean;
  defaultModel: string;
  /** Подсказки для поля модели. */
  models: string[];
  keyUrl?: string;
}

export const PROVIDERS: Provider[] = [
  {
    id: 'anthropic',
    title: 'Anthropic (Claude)',
    kind: 'anthropic',
    baseUrl: 'https://api.anthropic.com',
    needsKey: true,
    local: false,
    defaultModel: 'claude-opus-5-5',
    models: ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5'],
    keyUrl: 'https://platform.claude.com/settings/keys',
  },
  {
    id: 'openai',
    title: 'OpenAI',
    kind: 'openai',
    baseUrl: 'https://api.openai.com/v1',
    needsKey: true,
    local: false,
    defaultModel: '',
    models: [],
  },
  {
    id: 'openrouter',
    title: 'OpenRouter',
    kind: 'openai',
    baseUrl: 'https://openrouter.ai/api/v1',
    needsKey: true,
    local: false,
    defaultModel: '',
    models: [],
  },
  {
    id: 'ollama',
    title: 'Ollama (на этом компьютере)',
    kind: 'openai',
    baseUrl: 'http://localhost:11434/v1',
    needsKey: false,
    local: true,
    defaultModel: '',
    models: [],
  },
  {
    id: 'lmstudio',
    title: 'LM Studio (на этом компьютере)',
    kind: 'openai',
    baseUrl: 'http://localhost:1234/v1',
    needsKey: false,
    local: true,
    defaultModel: '',
    models: [],
  },
];

export function provider(id: ProviderId): Provider {
  return PROVIDERS.find((p) => p.id === id) ?? PROVIDERS[0];
}

export interface Request {
  provider: ProviderId;
  model: string;
  apiKey: string;
  /** Для локальных провайдеров адрес можно поменять (другой порт). */
  baseUrl: string;
}

/** Ошибка, понятная пользователю. */
export class LlmError extends Error {}

export async function assess(text: string, req: Request, signal?: AbortSignal): Promise<Assessment> {
  const p = provider(req.provider);
  if (!req.model.trim()) throw new LlmError('Укажите модель.');
  if (p.needsKey && !req.apiKey.trim()) throw new LlmError(`Укажите ключ API ${p.title}.`);
  // Адаптеры загружаются только при первом запросе: SDK Anthropic немаленький.
  if (p.kind === 'anthropic') {
    const { assessWithAnthropic } = await import('./anthropic.ts');
    return assessWithAnthropic(text, req, signal);
  }
  const { assessWithOpenAI } = await import('./openai.ts');
  return assessWithOpenAI(text, req, signal);
}

export async function listModels(req: Request, signal?: AbortSignal): Promise<string[]> {
  const p = provider(req.provider);
  if (p.kind === 'anthropic') return p.models;
  const { listOpenAIModels } = await import('./openai.ts');
  return listOpenAIModels(req, signal);
}
