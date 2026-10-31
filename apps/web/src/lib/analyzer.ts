import type { Analysis, ClarityReport } from './types';
import type { Request, Response } from './worker';

type Kind = 'analyze' | 'clarity';

/** Клиент Web Worker с ядром. Ответ на устаревший запрос отбрасывается. */
export class Analyzer {
  private worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
  private nextId = 0;
  // У оценки и у проверок свои последние запросы: они не вытесняют друг друга.
  private lastId: Record<Kind, number> = { analyze: 0, clarity: 0 };
  private pending = new Map<number, { resolve: (value: unknown) => void; reject: (e: Error) => void }>();

  constructor() {
    this.worker.onmessage = (event: MessageEvent<Response>) => {
      const response = event.data;
      const waiter = this.pending.get(response.id);
      this.pending.delete(response.id);
      if (!waiter) return;
      if (response.type === 'clarity-error') {
        waiter.reject(new Error(response.message));
        return;
      }
      const kind: Kind = response.type;
      waiter.resolve(response.id === this.lastId[kind] ? JSON.parse(response.json) : null);
    };
  }

  /** Отменяет текущий расчёт оценки: его ответ придёт как null. Проверки не трогает. */
  cancel(): void {
    this.lastId.analyze = ++this.nextId;
  }

  /** Возвращает null, если за время расчёта пришёл более новый текст. */
  analyze(text: string): Promise<Analysis | null> {
    return this.send({ type: 'analyze', id: 0, text }) as Promise<Analysis | null>;
  }

  /**
   * Проверки ясного языка. При первом вызове загружает словарь (около 7 МБ);
   * если загрузить не удалось, промис отклоняется.
   */
  clarity(text: string, dictionaryUrl: string): Promise<ClarityReport | null> {
    return this.send({ type: 'clarity', id: 0, text, dictionaryUrl }) as Promise<ClarityReport | null>;
  }

  private send(request: Request): Promise<unknown> {
    const id = ++this.nextId;
    this.lastId[request.type] = id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...request, id });
    });
  }
}
