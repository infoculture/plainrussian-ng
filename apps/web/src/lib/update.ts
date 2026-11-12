// Переход на новую версию сайта.
//
// Новый service worker сразу берёт страницы под управление (skipWaiting и clientsClaim
// в vite.config.ts) и удаляет из кеша файлы прежней версии. Страница прежней версии
// после этого не может догрузить свои части: поток с ядром оценки, проверки ясного
// языка, разборщики файлов — их уже нет ни в кеше, ни на сервере. Поэтому страница
// перезагружается, а набранный текст переживает перезагрузку в sessionStorage.

const KEY = 'plainrussian.reload-text';

/** Текст, сохранённый перед перезагрузкой. Читается один раз. */
export function takeSavedText(): string | null {
  try {
    const text = sessionStorage.getItem(KEY);
    sessionStorage.removeItem(KEY);
    return text;
  } catch {
    return null;
  }
}

/**
 * Перезагружает страницу, когда её берёт под управление новый service worker.
 * Вызывается один раз при запуске, до события load: service worker регистрируется после него.
 */
export function reloadOnUpdate(currentText: () => string): void {
  const sw = typeof navigator !== 'undefined' ? navigator.serviceWorker : undefined;
  // Без service worker на момент загрузки (первое посещение, настольное приложение,
  // dev-сервер) страница уже новая: первый service worker тоже вызовет
  // controllerchange, но перезагружать её незачем.
  if (!sw?.controller) return;
  let reloading = false;
  sw.addEventListener('controllerchange', () => {
    if (reloading) return;
    reloading = true;
    try {
      const text = currentText();
      if (text) sessionStorage.setItem(KEY, text);
    } catch {
      // Без хранилища текст пропадёт, но страница всё равно должна обновиться:
      // прежняя версия уже не работает.
    }
    location.reload();
  });
}
