<script lang="ts">
  // Действия с языковой моделью прямо под полем ввода: оценить текст и переписать его
  // для аудитории. Результаты — карточками там же, чтобы не искать их внизу страницы.
  import { clarityLabel } from '../lib/clarity';
  import { LlmError, assess, destination, provider, rewrite, runsLocally } from '../lib/llm/providers.ts';
  import {
    MAX_REWRITE_CHARS,
    MAX_REWRITE_PARTS,
    missingNumbers,
    rewritePrompt,
    splitForRewrite,
    type Hints,
  } from '../lib/llm/rewrite.ts';
  import { toRequest } from '../lib/llm/settings.ts';
  import { isConfigured, llm } from '../lib/llm/store.svelte.ts';
  import { LONG_SENTENCE, charCount, plural } from '../lib/segments';
  import { AUDIENCES, audience, setAudience } from '../lib/target.svelte.ts';
  import type { Analysis, ClarityReport } from '../lib/types';
  import LlmAssessment, { type AssessmentResult } from './LlmAssessment.svelte';
  import RewriteDraft, { type Draft } from './RewriteDraft.svelte';

  let {
    text,
    analysis,
    clarity,
    selection,
    formula,
    measure,
    onselect,
    onapply,
    onrestore,
  }: {
    text: string;
    /** Разметка и показатели: подсказки для модели, если посчитаны по этому тексту. */
    analysis: { text: string; result: Analysis } | null;
    clarity: { text: string; report: ClarityReport } | null;
    /** Выделение в поле ввода и текст, в котором его сделали. */
    selection: { text: string; start: number; end: number };
    /** Оценка формулами — для сравнения с оценкой LLM. */
    formula: number | null;
    /** Оценка другого текста (черновика); оценку поля ввода не трогает. */
    measure: (text: string) => Promise<Analysis | null>;
    /** Выделить фрагмент текущего текста в поле ввода. */
    onselect: (start: number, end: number) => void;
    /** Заменить текст поля. `range` — где в нём переписанный фрагмент; null, если переписан весь текст. */
    onapply: (next: string, range: { start: number; end: number } | null) => void;
    /** Вернуть текст, который был до замены. */
    onrestore: (previous: string) => void;
  } = $props();

  /** Сколько подсказок каждого рода отдать модели: остальное она найдёт сама. */
  const TOP_SENTENCES = 5;
  const TOP_FINDINGS = 15;
  const SNIPPET = 80;
  /** Как часто обновлять черновик, пока модель его пишет, мс. */
  const REDRAW = 100;

  let p = $derived(provider(llm.provider));
  let ps = $derived(llm.providers[llm.provider]);
  let configured = $derived(isConfigured());
  let target = $derived(destination(p, ps.baseUrl));
  let local = $derived(runsLocally(p, ps.baseUrl));
  let goal = $derived(audience());

  /** Границы без пробелов по краям: их модель всё равно не вернёт. */
  function trimmed(start: number, end: number) {
    while (start < end && /\s/.test(text[start])) start++;
    while (end > start && /\s/.test(text[end - 1])) end--;
    return { start, end };
  }

  let whole = $derived(trimmed(0, text.length));
  // Переписывается выделенный фрагмент, а без выделения — весь текст. Выделение из
  // прежнего текста не годится: смещения в нём указывают не туда.
  let range = $derived.by(() => {
    if (selection.text !== text || selection.end <= selection.start) return { ...whole, fragment: false };
    const r = trimmed(selection.start, selection.end);
    if (r.start === r.end) return { ...whole, fragment: false };
    return { ...r, fragment: r.start !== whole.start || r.end !== whole.end };
  });
  let size = $derived(range.end - range.start);
  let sentences = $derived(analysis?.text === text ? analysis.result.annotation.sentences : []);
  // Длинный текст уходит модели кусками по абзацам; null — его не разрезать.
  let parts = $derived(
    size > MAX_REWRITE_CHARS ? (splitForRewrite(text, range.start, range.end, sentences)?.length ?? null) : 1,
  );
  let partsWord = $derived(parts === null ? '' : `${parts} ${plural(parts, ['запрос', 'запроса', 'запросов'])}`);

  let rewriting = $state<AbortController | null>(null);
  let assessing = $state<AbortController | null>(null);
  let error = $state('');
  // Черновик и оценка заменяются только целиком: в них разметка длинного текста.
  let draft = $state.raw<Draft | null>(null);
  // Готовый черновик до нового запроса: к нему возвращаемся при отмене или ошибке.
  let beforeRewrite: Draft | null = null;
  let assessment = $state.raw<AssessmentResult | null>(null);
  // Последняя замена: пока текст не правили, её можно отменить.
  let applied = $state<{ before: string; after: string } | null>(null);

  let stale = $derived(draft !== null && draft.source !== text);
  let canRestore = $derived(applied !== null && applied.after === text);

  function snippet(s: string): string {
    const t = s.replace(/\s+/g, ' ').trim();
    return t.length > SNIPPET ? `${t.slice(0, SNIPPET).trimEnd()}…` : t;
  }

  /** Что нашли формулы и проверки ясного языка в части `doc` от start до end. */
  function hints(
    doc: string,
    a: Analysis | null,
    c: ClarityReport | null,
    start: number,
    end: number,
    fragment: boolean,
  ): Hints {
    const inside = (s: { start: number; end: number }) => s.start >= start && s.end <= end;
    const longSentences = (a?.annotation.sentences ?? [])
      .filter((s) => s.words >= LONG_SENTENCE && inside(s))
      .sort((x, y) => y.words - x.words || x.start - y.start)
      .slice(0, TOP_SENTENCES)
      .map((s) => ({ text: snippet(doc.slice(s.start, s.end)), words: s.words }));
    const seen = new Set<string>();
    const findings: Hints['findings'] = [];
    for (const f of c?.findings ?? []) {
      const key = `${f.kind}:${f.text.replace(/\s+/g, ' ').toLowerCase()}`;
      if (!inside(f) || seen.has(key)) continue;
      seen.add(key);
      findings.push({ label: clarityLabel(f.kind), text: f.text, suggestion: f.suggestion });
      if (findings.length >= TOP_FINDINGS) break;
    }
    // Оценка всего текста к фрагменту не относится.
    const grade = !fragment && a?.measurement.indexes ? a.measurement.indexes.average.value : null;
    return { grade, longSentences, findings };
  }

  function explain(e: unknown): string {
    return e instanceof LlmError ? e.message : `Не получилось: ${(e as Error).message}`;
  }

  type Base = Pick<Draft, 'source' | 'start' | 'end' | 'fragment' | 'original' | 'before'>;

  /**
   * Переписывает часть `doc` от from до to и ставит результат черновиком на место
   * base.start…base.end исходного текста. `doc` — исходный текст или прежний черновик
   * («Ещё проще»), `a` и `c` — его разметка. Длинная часть уходит модели кусками по
   * абзацам, по очереди; пока модель пишет, черновик показывается по мере ответа.
   */
  async function requestRewrite(
    base: Base,
    doc: string,
    a: Analysis | null,
    c: ClarityReport | null,
    from: number,
    to: number,
  ) {
    error = '';
    const chunks = splitForRewrite(doc, from, to, a?.annotation.sentences ?? []);
    if (!chunks || chunks.length > MAX_REWRITE_PARTS) {
      error = !chunks
        ? `В тексте есть предложение длиннее ${charCount(MAX_REWRITE_CHARS)}: выделите и перепишите его отдельно.`
        : `Текст слишком длинный: больше ${MAX_REWRITE_PARTS} запросов. Выделите часть текста.`;
      return;
    }
    const controller = new AbortController();
    rewriting = controller;
    beforeRewrite = draft?.progress ? beforeRewrite : draft;
    const started = performance.now();
    // Подписи берутся до запроса: пока он идёт, в настройках могут выбрать другое.
    const meta = { goal, title: p.title, model: ps.model };
    const results: string[] = [];
    // Куски черновика, а между ними — разделители абзацев из `doc`.
    const join = (done: string[]) =>
      done.map((part, i) => (i === 0 ? '' : doc.slice(chunks[i - 1].end, chunks[i].start)) + part).join('');

    // Черновик, который пишется, обновляется не чаще раза в REDRAW мс.
    let shown = { partial: '', part: 0 };
    let timer: ReturnType<typeof setTimeout> | undefined;
    const redraw = () => {
      timer = undefined;
      if (rewriting !== controller) return;
      draft = {
        ...base,
        ...meta,
        result: join([...results, shown.partial]),
        next: '',
        after: null,
        seconds: 0,
        missing: [],
        progress: { part: shown.part + 1, parts: chunks.length },
      };
    };
    const show = (partial: string, part: number) => {
      shown = { partial, part };
      timer ??= setTimeout(redraw, REDRAW);
    };

    try {
      for (const [i, chunk] of chunks.entries()) {
        const fragment = base.fragment || chunks.length > 1;
        const prompt = rewritePrompt(doc.slice(chunk.start, chunk.end), goal, hints(doc, a, c, chunk.start, chunk.end, fragment));
        show('', i);
        const result = await rewrite(prompt, toRequest(llm), controller.signal, (partial) => show(partial, i));
        // В настольном приложении запрос не прерывается, отменённый ответ просто отбрасываем.
        if (controller.signal.aborted) return;
        results.push(result);
      }
      clearTimeout(timer);
      const result = join(results);
      const next = base.source.slice(0, base.start) + result + base.source.slice(base.end);
      draft = {
        ...base,
        ...meta,
        result,
        next,
        after: null,
        seconds: (performance.now() - started) / 1000,
        missing: missingNumbers(base.original, result),
        progress: null,
      };
      beforeRewrite = null;
      const measured = await measure(next);
      // Пока считалось, черновик могли отбросить или заменить новым.
      if (measured && draft?.next === next) draft = { ...draft, after: measured };
    } catch (e) {
      clearTimeout(timer);
      if (!controller.signal.aborted) {
        const where = chunks.length > 1 ? `Часть ${results.length + 1} из ${chunks.length}. ` : '';
        error = where + explain(e);
        draft = beforeRewrite;
        beforeRewrite = null;
      }
    } finally {
      if (rewriting === controller) rewriting = null;
    }
  }

  /** Переписать часть текста поля от start до end. */
  function rewriteRange(start: number, end: number) {
    const source = text;
    const a = analysis?.text === source ? analysis.result : null;
    const c = clarity?.text === source ? clarity.report : null;
    const fragment = start !== whole.start || end !== whole.end;
    const base = { source, start, end, fragment, original: source.slice(start, end), before: a?.measurement ?? null };
    requestRewrite(base, source, a, c, start, end);
  }

  /** Переписать черновик ещё проще. Сравнение и числа — по-прежнему с исходным текстом. */
  function rewriteAgain() {
    if (!draft || draft.progress) return;
    const { source, start, end, fragment, original, before, result, next, after } = draft;
    requestRewrite({ source, start, end, fragment, original, before }, next, after, null, start, start + result.length);
  }

  /**
   * Переписать фрагмент, который нашла оценка LLM. Берутся целые предложения: иначе
   * склейка с соседним текстом могла бы порвать фразу.
   */
  function rewriteQuote(start: number, end: number) {
    const covering = sentences.filter((s) => s.end > start && s.start < end);
    if (covering.length) {
      start = Math.min(start, covering[0].start);
      end = Math.max(end, covering[covering.length - 1].end);
    }
    const r = trimmed(start, end);
    onselect(r.start, r.end);
    rewriteRange(r.start, r.end);
  }

  function apply() {
    if (!draft || draft.progress || draft.source !== text) return;
    const d = draft;
    applied = { before: d.source, after: d.next };
    draft = null;
    onapply(d.next, d.fragment ? { start: d.start, end: d.start + d.result.length } : null);
  }

  function restore() {
    if (!applied || applied.after !== text) return;
    onrestore(applied.before);
    applied = null;
  }

  async function runAssess() {
    error = '';
    const controller = new AbortController();
    assessing = controller;
    const started = performance.now();
    const current = text;
    const title = p.title;
    const model = ps.model;
    try {
      const result = await assess(current, toRequest(llm), controller.signal);
      if (controller.signal.aborted) return;
      assessment = { text: current, title, model, seconds: (performance.now() - started) / 1000, assessment: result };
    } catch (e) {
      if (!controller.signal.aborted) error = explain(e);
    } finally {
      if (assessing === controller) assessing = null;
    }
  }

  function cancelRewrite() {
    rewriting?.abort();
    rewriting = null;
    // Недописанный черновик уступает место прежнему, готовому.
    if (draft?.progress) draft = beforeRewrite;
    beforeRewrite = null;
  }

  /** Запрос из настольного приложения не прерывается: панель просто перестаёт его ждать. */
  function cancel() {
    cancelRewrite();
    assessing?.abort();
    assessing = null;
  }
</script>

<div class="llm-bar">
  {#if !configured}
    <p class="note">
      Языковая модель может оценить текст целиком, объяснить, что мешает его понять, и переписать его
      проще для выбранной аудитории. <a href="#settings">Подключить модель</a>
    </p>
  {:else}
    <div class="llm-actions">
      <label>
        Для кого
        <select value={goal.id} onchange={(e) => setAudience(e.currentTarget.value)}>
          {#each AUDIENCES as a (a.id)}
            <option value={a.id}>{a.title}</option>
          {/each}
        </select>
      </label>
      <button
        type="button"
        onclick={() => rewriteRange(range.start, range.end)}
        disabled={size === 0 || parts === null || parts > MAX_REWRITE_PARTS || rewriting !== null}
      >
        {rewriting ? 'Переписываю…' : range.fragment ? 'Переписать выделенное' : 'Переписать текст'}
      </button>
      <button type="button" class="secondary" onclick={runAssess} disabled={!text.trim() || assessing !== null}>
        {assessing ? 'Оцениваю…' : 'Оценить с помощью LLM'}
      </button>
      {#if rewriting || assessing}
        <button type="button" class="secondary" onclick={cancel}>Отменить</button>
      {/if}
    </div>
    <p class="privacy" class:local>
      {#if local}
        {p.title}, {ps.model}: текст останется на этом компьютере.
      {:else}
        {ps.model}: текст будет отправлен: {target}.
      {/if}
      {#if range.fragment}
        Переписывается только выделенное ({charCount(size)}).
      {:else}
        Чтобы переписать часть текста, выделите её в поле.
      {/if}
      <a href="#settings">Изменить</a>
    </p>
    {#if parts === null}
      <p class="warning">
        В тексте есть предложение длиннее {charCount(MAX_REWRITE_CHARS)}: выделите и перепишите его отдельно.
      </p>
    {:else if parts > MAX_REWRITE_PARTS}
      <p class="warning">
        Текст слишком длинный для переписывания: нужно больше {MAX_REWRITE_PARTS} запросов. Выделите часть текста.
      </p>
    {:else if parts > 1}
      <p class="note">
        Текст длиннее {charCount(MAX_REWRITE_CHARS)}: модель перепишет его по абзацам, это {partsWord} подряд.
      </p>
    {/if}
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if canRestore}
    <p class="note">
      Текст заменён черновиком.
      <button type="button" class="link" onclick={restore}>Вернуть как было</button>
    </p>
  {/if}
</div>

{#if draft}
  <RewriteDraft
    {draft}
    {stale}
    busy={rewriting !== null}
    onapply={apply}
    onagain={rewriteAgain}
    ondiscard={() => {
      if (draft?.progress) cancelRewrite();
      else draft = null;
    }}
  />
{/if}
{#if assessment}
  <LlmAssessment
    result={assessment}
    {text}
    {formula}
    busy={rewriting !== null}
    onshow={onselect}
    onrewrite={rewriteQuote}
    onclose={() => (assessment = null)}
  />
{/if}
