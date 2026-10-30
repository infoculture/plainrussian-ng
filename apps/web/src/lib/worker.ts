// Ядро считает в отдельном потоке, чтобы длинный текст не подвешивал страницу.
import init, { analyze } from '../wasm/pkg/pr_wasm.js';

export interface Request {
  id: number;
  text: string;
}

export interface Response {
  id: number;
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
  const { id, text } = event.data;
  scope.postMessage({ id, json: analyze(text) });
};
