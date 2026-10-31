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
  | { type: 'clarity-error'; id: number; message: string };

// Библиотеки DOM и WebWorker в TypeScript конфликтуют, поэтому тип потока описан вручную.
const scope = self as unknown as {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: Response): void;
};

const ready = init();
let clarity: Promise<Clarity> | null = null;

async function loadClarity(dictionaryUrl: string): Promise<Clarity> {
  const glue = await import('../wasm/clarity-pkg/pr_wasm_clarity.js');
  await glue.default();
  const response = await fetch(dictionaryUrl);
  if (!response.ok) throw new Error(`словарь не загрузился (${response.status})`);
  let bytes = new Uint8Array(await response.arrayBuffer());
  // Словарь лежит сжатым; если хостинг уже распаковал его сам, сигнатуры gzip не будет.
  if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
    const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'));
    bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  }
  return new glue.Clarity(bytes);
}

scope.onmessage = async (event) => {
  const request = event.data;
  if (request.type === 'analyze') {
    await ready;
    scope.postMessage({ type: 'analyze', id: request.id, json: analyze(request.text) });
    return;
  }
  try {
    clarity ??= loadClarity(request.dictionaryUrl);
    const checker = await clarity;
    scope.postMessage({ type: 'clarity', id: request.id, json: checker.check(request.text) });
  } catch (e) {
    clarity = null; // при следующем запросе попробуем загрузить заново
    scope.postMessage({ type: 'clarity-error', id: request.id, message: e instanceof Error ? e.message : String(e) });
  }
};
