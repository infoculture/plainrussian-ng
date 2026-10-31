import type { Analysis, ClarityReport } from './types';
import type { Request, Response } from './worker';

type Kind = 'analyze' | 'clarity';

interface Job {
  request: Request;
  resolve: (value: unknown) => void;
  reject: (e: Error) => void;
}

/** Клиент Web Worker с ядром. Ответ на устаревший запрос отбрасывается. */
export class Analyzer {
  private worker: Worker | null = null;
  private nextId = 0;
  // У оценки и у проверок свои последние запросы: они не вытесняют друг друга.
  private lastId: Record<Kind, number> = { analyze: 0, clarity: 0 };
  private pending = new Map<number, { resolve: (value: unknown) => void; reject: (e: Error) => void }>();
  // В потоке не больше одного расчёта каждого вида. Пока он идёт, следующий запрос
  // ждёт здесь, а более новый вытесняет его: иначе при наборе длинного текста поток
  // считал бы подряд все промежуточные версии.
  private busy: Record<Kind, boolean> = { analyze: false, clarity: false };
  private queued: Record<Kind, Job | null> = { analyze: null, clarity: null };

  private spawn(): Worker {
    const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
    worker.onmessage = (event: MessageEvent<Response>) => {
      const response = event.data;
      const waiter = this.pending.get(response.id);
      this.pending.delete(response.id);
      if (!waiter) return;
      if (response.type === 'error') {
        waiter.reject(new Error(response.message));
        return;
      }
      const kind: Kind = response.type;
      waiter.resolve(response.id === this.lastId[kind] ? JSON.parse(response.json) : null);
    };
    // Поток не запустился или упал: ждать его ответов бессмысленно, следующий запрос
    // запустит новый.
    worker.onerror = (event) => {
      event.preventDefault();
      worker.terminate();
      if (this.worker === worker) this.worker = null;
      const error = new Error(event.message || 'поток с ядром оценки остановился');
      const waiters = [...this.pending.values()];
      this.pending.clear();
      for (const waiter of waiters) waiter.reject(error);
    };
    return worker;
  }

  /** Отменяет текущий расчёт оценки: его ответ придёт как null. Проверки не трогает. */
  cancel(): void {
    this.lastId.analyze = ++this.nextId;
    this.queued.analyze?.resolve(null);
    this.queued.analyze = null;
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
    const kind = request.type;
    const id = ++this.nextId;
    this.lastId[kind] = id;
    return new Promise((resolve, reject) => {
      const job = { request: { ...request, id }, resolve, reject };
      this.queued[kind]?.resolve(null);
      this.queued[kind] = null;
      if (this.busy[kind]) this.queued[kind] = job;
      else this.dispatch(job);
    });
  }

  private dispatch(job: Job): void {
    const kind = job.request.type;
    this.busy[kind] = true;
    const done = () => {
      this.busy[kind] = false;
      const next = this.queued[kind];
      this.queued[kind] = null;
      if (next) this.dispatch(next);
    };
    this.pending.set(job.request.id, {
      resolve: (value) => {
        job.resolve(value);
        done();
      },
      reject: (e) => {
        job.reject(e);
        done();
      },
    });
    this.worker ??= this.spawn();
    this.worker.postMessage(job.request);
  }
}
