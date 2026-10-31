// Ядро считает в отдельном потоке, чтобы длинный текст не подвешивал страницу.
// Модуль проверок ясного языка и словарь к нему загружаются только по запросу.
import init, { analyze } from '../wasm/pkg/pr_wasm.js';
import type { Clarity } from '../wasm/clarity-pkg/pr_wasm_clarity.js';

export type Request =
  | { type: 'analyze'; id: number; text: string }
  | { type: 'clarity'; id: number; text: string; dictionaryUrl: string };

export type Response =
  | { type: 'analyze'; id: number; json: string }
  | { type: 'clarity'; id: number; json: string }
  | { type: 'error'; id: number; message: string };

// Библиотеки DOM и WebWorker в TypeScript конфликтуют, поэтому тип потока описан вручную.
const scope = self as unknown as {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: Response): void;
};

const ready = init();
let clarity: Promise<Clarity> | null = null;

/**
 * Словарь кешируется здесь, а не в service worker: запросы из потока до него
 * доходят не во всех браузерах. В имени файла хеш содержимого, поэтому прежние
 * версии из кеша удаляются.
 */
async function fetchDictionary(url: string): Promise<ArrayBuffer> {
  // Cache API есть только в защищённом контексте (https, localhost).
  const cache = typeof caches === 'undefined' ? null : await caches.open('dictionary').catch(() => null);
  const cached = await cache?.match(url);
  if (cached) return cached.arrayBuffer();
  const response = await fetch(url);
  if (!response.ok) throw new Error(`словарь не загрузился (${response.status})`);
  if (cache) {
    for (const old of await cache.keys()) if (old.url !== url) await cache.delete(old);
    await cache.put(url, response.clone()).catch(() => {});
  }
  return response.arrayBuffer();
}

async function loadClarity(dictionaryUrl: string): Promise<Clarity> {
  const glue = await import('../wasm/clarity-pkg/pr_wasm_clarity.js');
  await glue.default();
  let bytes = new Uint8Array(await fetchDictionary(dictionaryUrl));
  // Словарь лежит сжатым; если хостинг уже распаковал его сам, сигнатуры gzip не будет.
  if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
    const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
    bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  }
  return new glue.Clarity(bytes);
}

scope.onmessage = async (event) => {
  const request = event.data;
  // Ответ приходит на каждый запрос, даже неудачный: иначе страница ждала бы его вечно.
  try {
    if (request.type === 'analyze') {
      await ready;
      scope.postMessage({ type: 'analyze', id: request.id, json: analyze(request.text) });
      return;
    }
    clarity ??= loadClarity(request.dictionaryUrl).catch((e) => {
      clarity = null; // при следующем запросе попробуем загрузить заново
      throw e;
    });
    const checker = await clarity;
    scope.postMessage({ type: 'clarity', id: request.id, json: checker.check(request.text) });
  } catch (e) {
    scope.postMessage({ type: 'error', id: request.id, message: e instanceof Error ? e.message : String(e) });
  }
};
