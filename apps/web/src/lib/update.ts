// Переход на новую версию сайта.
//
// Новый service worker сразу берёт страницы под управление (skipWaiting и clientsClaim
// в vite.config.ts) и удаляет из кеша файлы прежней версии. Страница прежней версии
// после этого не может догрузить свои части: поток с ядром оценки, проверки ясного
// языка, разборщики файлов, модули LLM — их уже нет ни в кеше, ни на сервере. Поэтому
// страница перезагружается, а набранный текст переживает перезагрузку в sessionStorage.
//
// Перезагрузок две. По controllerchange — когда новая версия берёт страницу. И по
// неудачной загрузке своей части — для страниц, которые новая версия не взяла: открытых
// до первого service worker, после перезагрузки без кеша (Shift+обновить) или в
// браузере без service worker.

const KEY = 'plainrussian.reload-text';
const RELOADED_AT = 'plainrussian.reloaded-at';
/**
 * Перезагрузка из-за незагрузившейся части — не чаще раза в минуту: если часть не
 * грузится и на новой странице (сеть, сбой выкладки), ошибка остаётся видна как есть.
 */
const RELOAD_PAUSE = 60_000;

let currentText: () => string = () => '';
let reloading = false;

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

function saveTextAndReload(): void {
  reloading = true;
  try {
    const text = currentText();
    if (text) sessionStorage.setItem(KEY, text);
  } catch {
    // Без хранилища текст пропадёт, но страница всё равно должна обновиться:
    // прежняя версия уже не работает.
  }
  location.reload();
}

/**
 * Часть страницы не загрузилась: скорее всего, сайт обновился и файлов этой версии
 * больше нет. Перезагружает страницу на новую версию; false — если уже перезагружали
 * меньше минуты назад или без sessionStorage нельзя это проверить.
 */
export function reloadAfterFailedLoad(): boolean {
  if (reloading) return true;
  try {
    const last = Number(sessionStorage.getItem(RELOADED_AT));
    if (last && Date.now() - last < RELOAD_PAUSE) return false;
    sessionStorage.setItem(RELOADED_AT, String(Date.now()));
  } catch {
    return false;
  }
  saveTextAndReload();
  return true;
}

/**
 * Перезагружает страницу на новую версию сайта. Вызывается один раз при запуске,
 * до события load: service worker регистрируется после него.
 */
export function reloadOnUpdate(text: () => string): void {
  currentText = text;
  if (typeof window === 'undefined') return;
  // Vite сообщает так о динамическом импорте, который не загрузился.
  window.addEventListener('vite:preloadError', () => reloadAfterFailedLoad());
  const sw = navigator.serviceWorker;
  // Без service worker на момент загрузки (первое посещение, настольное приложение,
  // dev-сервер) страница уже новая: первый service worker тоже вызовет
  // controllerchange, но перезагружать её незачем.
  if (!sw?.controller) return;
  sw.addEventListener('controllerchange', () => {
    if (!reloading) saveTextAndReload();
  });
}
