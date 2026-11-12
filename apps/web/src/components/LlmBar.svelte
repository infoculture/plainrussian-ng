<script lang="ts">
  // Действия с языковой моделью прямо под полем ввода: оценить текст и переписать его
  // для аудитории. Результаты — карточками там же, чтобы не искать их внизу страницы.
  import { clarityLabel } from '../lib/clarity';
  import { LlmError, assess, destination, provider, rewrite, runsLocally } from '../lib/llm/providers.ts';
  import { MAX_REWRITE_CHARS, missingNumbers, rewritePrompt, type Hints } from '../lib/llm/rewrite.ts';
  import { toRequest } from '../lib/llm/settings.ts';
  import { isConfigured, llm } from '../lib/llm/store.svelte.ts';
  import { LONG_SENTENCE, charCount } from '../lib/segments';
  import { AUDIENCES, audience, setAudience } from '../lib/target.svelte.ts';
  import type { Analysis, ClarityReport, Measurement } from '../lib/types';
  import LlmAssessment, { type AssessmentResult } from './LlmAssessment.svelte';
  import RewriteDraft, { type Draft } from './RewriteDraft.svelte';

  let {
    text,
    analysis,
    clarity,
    selection,
    formula,
    measure,
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
    /** Заменить текст поля. `range` — где в нём переписанный фрагмент; null, если переписан весь текст. */
    onapply: (next: string, range: { start: number; end: number } | null) => void;
    /** Вернуть текст, который был до замены. */
    onrestore: (previous: string) => void;
  } = $props();

  /** Сколько подсказок каждого рода отдать модели: остальное она найдёт сама. */
  const TOP_SENTENCES = 5;
  const TOP_FINDINGS = 15;
  const SNIPPET = 80;

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
  let tooLong = $derived(size > MAX_REWRITE_CHARS);

  let rewriting = $state<AbortController | null>(null);
  let assessing = $state<AbortController | null>(null);
  let error = $state('');
  // Черновик и оценка заменяются только целиком: в них разметка длинного текста.
  let draft = $state.raw<Draft | null>(null);
  let assessment = $state.raw<AssessmentResult | null>(null);
  // Последняя замена: пока текст не правили, её можно отменить.
  let applied = $state<{ before: string; after: string } | null>(null);

  let stale = $derived(draft !== null && draft.source !== text);
  let canRestore = $derived(applied !== null && applied.after === text);

  function snippet(s: string): string {
    const t = s.replace(/\s+/g, ' ').trim();
    return t.length > SNIPPET ? `${t.slice(0, SNIPPET).trimEnd()}…` : t;
  }

  /** Что нашли формулы и проверки ясного языка в части `source` от start до end. */
  function hints(
    source: string,
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
      .map((s) => ({ text: snippet(source.slice(s.start, s.end)), words: s.words }));
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

  /** Просит модель переписать `input` и ставит черновик на место части base.start…base.end. */
  async function requestRewrite(base: Base, input: string, found: Hints) {
    error = '';
    const prompt = rewritePrompt(input, goal, found);
    const controller = new AbortController();
    rewriting = controller;
    const started = performance.now();
    // Подписи берутся до запроса: пока он идёт, в настройках могут выбрать другое.
    const meta = { goal, title: p.title, model: ps.model };
    try {
      const result = await rewrite(prompt, toRequest(llm), controller.signal);
      // В настольном приложении запрос не прерывается, отменённый ответ просто отбрасываем.
      if (controller.signal.aborted) return;
      const next = base.source.slice(0, base.start) + result + base.source.slice(base.end);
      draft = {
        ...base,
        ...meta,
        result,
        next,
        after: null,
        seconds: (performance.now() - started) / 1000,
        missing: missingNumbers(base.original, result),
      };
      const measured = await measure(next);
      // Пока считалось, черновик могли отбросить или заменить новым.
      if (measured && draft?.next === next) draft = { ...draft, after: measured };
    } catch (e) {
      if (!controller.signal.aborted) error = explain(e);
    } finally {
      if (rewriting === controller) rewriting = null;
    }
  }

  function runRewrite() {
    const source = text;
    const { start, end, fragment } = range;
    const a = analysis?.text === source ? analysis.result : null;
    const c = clarity?.text === source ? clarity.report : null;
    const original = source.slice(start, end);
    const before: Measurement | null = a?.measurement ?? null;
    requestRewrite({ source, start, end, fragment, original, before }, original, hints(source, a, c, start, end, fragment));
  }

  /** Переписать черновик ещё проще. Сравнение и числа — по-прежнему с исходным текстом. */
  function rewriteAgain() {
    if (!draft) return;
    const { source, start, end, fragment, original, before, result, next, after } = draft;
    const found = hints(next, after, null, start, start + result.length, fragment);
    requestRewrite({ source, start, end, fragment, original, before }, result, found);
  }

  function apply() {
    if (!draft || draft.source !== text) return;
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

  /** Запрос из настольного приложения не прерывается: панель просто перестаёт его ждать. */
  function cancel() {
    rewriting?.abort();
    assessing?.abort();
    rewriting = null;
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
      <button type="button" onclick={runRewrite} disabled={size === 0 || tooLong || rewriting !== null}>
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
    {#if tooLong}
      <p class="warning">
        За один раз можно переписать до {charCount(MAX_REWRITE_CHARS)}. Выделите часть текста в поле ввода.
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
  <RewriteDraft {draft} {stale} busy={rewriting !== null} onapply={apply} onagain={rewriteAgain} ondiscard={() => (draft = null)} />
{/if}
{#if assessment}
  <LlmAssessment result={assessment} {text} {formula} onclose={() => (assessment = null)} />
{/if}
