// Настройки LLM, общие для страницы настроек и панели оценки на главной.
import { provider } from './providers.ts';
import { loadSecrets, loadSettings, saveSettings } from './settings.ts';

export const llm = $state(loadSettings());

let started = false;

/** Сохраняет настройки при каждом изменении и подставляет запомненные ключи. Вызывается один раз. */
export function persistLlmSettings(): void {
  if (started) return;
  started = true;
  $effect.root(() => {
    $effect(() => saveSettings($state.snapshot(llm)));
  });
  loadSecrets(llm);
}

/** Хватает ли настроек, чтобы отправить запрос. */
export function isConfigured(): boolean {
  const p = provider(llm.provider);
  const s = llm.providers[llm.provider];
  const baseUrl = p.editableUrl ? s.baseUrl : p.baseUrl;
  return Boolean(baseUrl.trim() && s.model.trim() && (p.key !== 'required' || s.apiKey.trim()));
}
