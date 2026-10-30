import type { Algorithm, Result } from './types';
import type { Request, Response } from './worker';

/** Клиент Web Worker с ядром. Ответ на устаревший запрос отбрасывается. */
export class Analyzer {
  private worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
  private lastId = 0;
  private resolvers = new Map<number, (result: Result | null) => void>();

  constructor() {
    this.worker.onmessage = (event: MessageEvent<Response>) => {
      const { id, algorithm, json } = event.data;
      const resolve = this.resolvers.get(id);
      this.resolvers.delete(id);
      resolve?.(id === this.lastId ? ({ algorithm, analysis: JSON.parse(json) } as Result) : null);
    };
  }

  /** Отменяет текущий расчёт: его ответ придёт как null. */
  cancel(): void {
    this.lastId++;
  }

  /** Возвращает null, если за время расчёта пришёл более новый запрос. */
  analyze(text: string, algorithm: Algorithm): Promise<Result | null> {
    const id = ++this.lastId;
    const request: Request = { id, text, algorithm };
    return new Promise((resolve) => {
      this.resolvers.set(id, resolve);
      this.worker.postMessage(request);
    });
  }
}
