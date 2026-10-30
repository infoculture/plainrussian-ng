// В настольном приложении (Tauri) у страницы есть команды Rust: сеть без CORS
// и системное хранилище ключей (apps/desktop/src-tauri). В браузере их нет.

export const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}

export interface Page {
  /** Адрес после перенаправлений. */
  url: string;
  /** MIME-тип без параметров. */
  content_type: string;
  text: string;
}

/** Загружает страницу по ссылке. Только в настольном приложении. */
export function fetchPage(url: string): Promise<Page> {
  return invoke<Page>('fetch_url', { url });
}
