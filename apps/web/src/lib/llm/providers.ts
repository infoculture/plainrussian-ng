// Провайдеры LLM по модели BYOK: ключ принадлежит пользователю, запросы идут
// напрямую провайдеру. В браузере список адресов совпадает с connect-src в CSP
// (vite.config.ts): с другими адресами страница соединиться не может. В настольном
// приложении OpenAI-совместимые запросы делает Rust, поэтому годится любой адрес.
import type { Assessment } from './assessment.ts';
import { isDesktop } from '../platform.ts';

export type ProviderId = 'anthropic' | 'openai' | 'openrouter' | 'ollama' | 'lmstudio' | 'custom';

export interface Provider {
  id: ProviderId;
  title: string;
  /** Как с ним говорить: SDK Anthropic или OpenAI-совместимый API. */
  kind: 'anthropic' | 'openai';
  baseUrl: string;
  key: 'required' | 'optional' | 'none';
  /** Адрес можно поменять: другой порт локального сервера или свой сервис. */
  editableUrl: boolean;
  /** Модель работает на этом компьютере, текст никуда не уходит. */
  local: boolean;
  /** Только в настольном приложении: браузер не пустит запрос на произвольный адрес. */
  desktopOnly?: boolean;
  defaultModel: string;
  /** Подсказки для поля модели. */
  models: string[];
  keyUrl?: string;
}

const ALL_PROVIDERS: Provider[] = [
  {
    id: 'anthropic',
    title: 'Anthropic (Claude)',
    kind: 'anthropic',
    baseUrl: 'https://api.anthropic.com',
    key: 'required',
    editableUrl: false,
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
    key: 'required',
    editableUrl: false,
    local: false,
    defaultModel: '',
    models: [],
  },
  {
    id: 'openrouter',
    title: 'OpenRouter',
    kind: 'openai',
    baseUrl: 'https://openrouter.ai/api/v1',
    key: 'required',
    editableUrl: false,
    local: false,
    defaultModel: '',
    models: [],
  },
  {
    id: 'ollama',
    title: 'Ollama (на этом компьютере)',
    kind: 'openai',
    baseUrl: 'http://localhost:11434/v1',
    key: 'none',
    editableUrl: true,
    local: true,
    defaultModel: '',
    models: [],
  },
  {
    id: 'lmstudio',
    title: 'LM Studio (на этом компьютере)',
    kind: 'openai',
    baseUrl: 'http://localhost:1234/v1',
    key: 'none',
    editableUrl: true,
    local: true,
    defaultModel: '',
    models: [],
  },
  {
    id: 'custom',
    title: 'Другой OpenAI-совместимый сервис',
    kind: 'openai',
    baseUrl: '',
    key: 'optional',
    editableUrl: true,
    local: false,
    desktopOnly: true,
    defaultModel: '',
    models: [],
  },
];

/** Провайдеры, доступные на этой платформе. */
export const PROVIDERS: Provider[] = ALL_PROVIDERS.filter((p) => isDesktop || !p.desktopOnly);

export function provider(id: ProviderId): Provider {
  return PROVIDERS.find((p) => p.id === id) ?? PROVIDERS[0];
}

export interface Request {
  provider: ProviderId;
  model: string;
  apiKey: string;
  baseUrl: string;
}

/** Ошибка, понятная пользователю. */
export class LlmError extends Error {}

export async function assess(text: string, req: Request, signal?: AbortSignal): Promise<Assessment> {
  const p = provider(req.provider);
  if (!req.baseUrl.trim()) throw new LlmError('Укажите адрес API.');
  if (!req.model.trim()) throw new LlmError('Укажите модель.');
  if (p.key === 'required' && !req.apiKey.trim()) throw new LlmError(`Укажите ключ API ${p.title}.`);
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
  if (!req.baseUrl.trim()) throw new LlmError('Укажите адрес API.');
  const { listOpenAIModels } = await import('./openai.ts');
  return listOpenAIModels(req, signal);
}
