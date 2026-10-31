// Настройки LLM хранятся в браузере. Ключ запоминается, только если пользователь
// попросил: в браузере — в localStorage, в настольном приложении — в системном
// хранилище ключей (Keychain, Credential Manager, Secret Service).
import { invoke, isDesktop } from '../platform.ts';
import { PROVIDERS, provider, type ProviderId, type Request } from './providers.ts';

const KEY = 'plainrussian.llm';

export interface ProviderSettings {
  model: string;
  baseUrl: string;
  apiKey: string;
  rememberKey: boolean;
  /** Тип доступа, только для GigaChat. */
  scope: string;
}

export interface Settings {
  provider: ProviderId;
  providers: Record<ProviderId, ProviderSettings>;
}

function defaults(): Settings {
  const providers = {} as Record<ProviderId, ProviderSettings>;
  for (const p of PROVIDERS) {
    providers[p.id] = {
      model: p.defaultModel,
      baseUrl: p.baseUrl,
      apiKey: '',
      rememberKey: false,
      scope: p.scopes?.[0].id ?? '',
    };
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
      // Ключ мог остаться в localStorage от веб-версии: в настольном приложении он не нужен.
      if (isDesktop) settings.providers[p.id].apiKey = '';
    }
  } catch {
    // Без хранилища работаем с настройками по умолчанию.
  }
  return settings;
}

// Что уже лежит в системном хранилище: писать туда на каждое нажатие клавиши незачем.
const synced: Partial<Record<ProviderId, string>> = {};
// Пока ключи не прочитаны, синхронизировать нельзя: пустое поле стёрло бы сохранённый ключ.
let secretsLoaded = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** В настольном приложении подставляет запомненные ключи из системного хранилища. */
export async function loadSecrets(settings: Settings): Promise<void> {
  if (!isDesktop) return;
  for (const p of PROVIDERS) {
    const s = settings.providers[p.id];
    if (!s.rememberKey) {
      // Без «Запомнить» ключа в хранилище нет: при снятии галочки он удаляется сразу.
      synced[p.id] = '';
      continue;
    }
    const key = await invoke<string | null>('secret_get', { name: p.id }).catch(() => null);
    if (key && !s.apiKey) s.apiKey = key;
    synced[p.id] = key ?? '';
  }
  secretsLoaded = true;
  syncSecrets(settings);
}

export function saveSettings(settings: Settings): void {
  const copy: Settings = structuredClone(settings);
  for (const p of PROVIDERS) {
    const s = copy.providers[p.id];
    if (isDesktop || !s.rememberKey) s.apiKey = '';
    // Адрес облачного провайдера не меняется: его ограничивает CSP.
    if (!p.editableUrl) s.baseUrl = p.baseUrl;
  }
  try {
    localStorage.setItem(KEY, JSON.stringify(copy));
  } catch {
    // Настройки просто не запомнятся.
  }
  if (isDesktop) {
    clearTimeout(timer);
    timer = setTimeout(() => syncSecrets(settings), 500);
  }
}

function syncSecrets(settings: Settings): void {
  if (!secretsLoaded) return;
  for (const p of PROVIDERS) {
    const s = settings.providers[p.id];
    const wanted = s.rememberKey ? s.apiKey : '';
    if (synced[p.id] === wanted) continue;
    synced[p.id] = wanted;
    const done = wanted
      ? invoke('secret_set', { name: p.id, value: wanted })
      : invoke('secret_delete', { name: p.id });
    done.catch((e) => console.error(`Хранилище ключей: ${e}`));
  }
}

export function toRequest(settings: Settings): Request {
  const s = settings.providers[settings.provider];
  const p = provider(settings.provider);
  return {
    provider: p.id,
    model: s.model,
    apiKey: s.apiKey,
    baseUrl: p.editableUrl ? s.baseUrl : p.baseUrl,
    scope: s.scope,
  };
}
