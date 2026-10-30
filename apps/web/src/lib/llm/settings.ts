// Настройки LLM хранятся в браузере. Ключ — только если пользователь попросил его запомнить.
import { PROVIDERS, provider, type ProviderId, type Request } from './providers.ts';

const KEY = 'plainrussian.llm';

export interface ProviderSettings {
  model: string;
  baseUrl: string;
  apiKey: string;
  rememberKey: boolean;
}

export interface Settings {
  provider: ProviderId;
  providers: Record<ProviderId, ProviderSettings>;
}

function defaults(): Settings {
  const providers = {} as Record<ProviderId, ProviderSettings>;
  for (const p of PROVIDERS) {
    providers[p.id] = { model: p.defaultModel, baseUrl: p.baseUrl, apiKey: '', rememberKey: false };
  }
  return { provider: 'anthropic', providers };
}

export function loadSettings(): Settings {
  const settings = defaults();
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Settings> | null;
    if (saved?.provider && PROVIDERS.some((p) => p.id === saved.provider)) settings.provider = saved.provider;
    for (const p of PROVIDERS) {
      Object.assign(settings.providers[p.id], saved?.providers?.[p.id] ?? {});
    }
  } catch {
    // Без хранилища работаем с настройками по умолчанию.
  }
  return settings;
}

export function saveSettings(settings: Settings): void {
  const copy: Settings = structuredClone(settings);
  for (const p of PROVIDERS) {
    const s = copy.providers[p.id];
    if (!s.rememberKey) s.apiKey = '';
    // Адрес облачного провайдера не меняется: его ограничивает CSP.
    if (!p.local) s.baseUrl = p.baseUrl;
  }
  try {
    localStorage.setItem(KEY, JSON.stringify(copy));
  } catch {
    // Настройки просто не запомнятся.
  }
}

export function toRequest(settings: Settings): Request {
  const s = settings.providers[settings.provider];
  const p = provider(settings.provider);
  return { provider: p.id, model: s.model, apiKey: s.apiKey, baseUrl: p.local ? s.baseUrl : p.baseUrl };
}
