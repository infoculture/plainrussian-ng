// Ядро считает в отдельном потоке, чтобы длинный текст не подвешивал страницу.
import init, { analyze, analyzeLegacy } from '../wasm/pkg/pr_wasm.js';
import type { Algorithm } from './types';

export interface Request {
  id: number;
  text: string;
  algorithm: Algorithm;
}

export interface Response {
  id: number;
  algorithm: Algorithm;
  json: string;
}

// Библиотеки DOM и WebWorker в TypeScript конфликтуют, поэтому тип потока описан вручную.
const scope = self as unknown as {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: Response): void;
};

const ready = init();

scope.onmessage = async (event) => {
  await ready;
  const { id, text, algorithm } = event.data;
  const json = algorithm === 'legacy' ? analyzeLegacy(text) : analyze(text);
  scope.postMessage({ id, algorithm, json });
};
