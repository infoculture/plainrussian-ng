import type { Analysis } from './types';
import type { Request, Response } from './worker';

/** Клиент Web Worker с ядром. Ответ на устаревший запрос отбрасывается. */
export class Analyzer {
  private worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
  private lastId = 0;
  private resolvers = new Map<number, (analysis: Analysis | null) => void>();

  constructor() {
    this.worker.onmessage = (event: MessageEvent<Response>) => {
      const { id, json } = event.data;
      const resolve = this.resolvers.get(id);
      this.resolvers.delete(id);
      resolve?.(id === this.lastId ? (JSON.parse(json) as Analysis) : null);
    };
  }

  /** Отменяет текущий расчёт: его ответ придёт как null. */
  cancel(): void {
    this.lastId++;
  }

  /** Возвращает null, если за время расчёта пришёл более новый текст. */
  analyze(text: string): Promise<Analysis | null> {
    const id = ++this.lastId;
    const request: Request = { id, text };
    return new Promise((resolve) => {
      this.resolvers.set(id, resolve);
      this.worker.postMessage(request);
    });
  }
}
