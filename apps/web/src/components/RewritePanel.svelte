<script lang="ts">
  import { clarityLabel } from '../lib/clarity';
  import { LlmError, destination, provider, rewrite, runsLocally } from '../lib/llm/providers.ts';
  import { MAX_REWRITE_CHARS, missingNumbers, rewritePrompt, type Hints } from '../lib/llm/rewrite.ts';
  import { toRequest } from '../lib/llm/settings.ts';
  import { isConfigured, llm } from '../lib/llm/store.svelte.ts';
  import { LONG_SENTENCE, charCount } from '../lib/segments';
  import { AUDIENCES, audience, setAudience, type Audience } from '../lib/target.svelte.ts';
  import type { Analysis, ClarityReport } from '../lib/types';

  let {
    text,
    analysis,
    clarity,
    selection,
    onapply,
    onrestore,
  }: {
    text: string;
    /** Разметка и показатели: подсказки для модели, если посчитаны по этому тексту. */
    analysis: { text: string; result: Analysis } | null;
    clarity: { text: string; report: ClarityReport } | null;
    /** Выделение в поле ввода и текст, в котором его сделали. */
    selection: { text: string; start: number; end: number };
    /** Заменить текст поля. `range` — где в нём переписанный фрагмент; null, если переписан весь текст. */
    onapply: (next: string, range: { start: number; end: number } | null) => void;
    /** Вернуть текст, который был до замены. */
    onrestore: (previous: string) => void;
  } = $props();

  const fmt = (n: number) => n.toLocaleString('ru-RU', { maximumFractionDigits: 1 });
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

  let running = $state<AbortController | null>(null);
  let error = $state('');
  let draft = $state<{
    /** Текст поля на момент запроса и границы переписанного в нём. */
    source: string;
    start: number;
    end: number;
    fragment: boolean;
    result: string;
    goal: Audience;
    title: string;
    model: string;
    seconds: number;
    /** Числа исходника, которых нет в черновике. */
    missing: string[];
  } | null>(null);
  // Последняя замена: пока текст не правили, её можно отменить.
  let applied = $state<{ before: string; after: string } | null>(null);

  let stale = $derived(draft !== null && draft.source !== text);
  let canRestore = $derived(applied !== null && applied.after === text);

  function snippet(s: string): string {
    const t = s.replace(/\s+/g, ' ').trim();
    return t.length > SNIPPET ? `${t.slice(0, SNIPPET).trimEnd()}…` : t;
  }

  /** Что нашли формулы и проверки ясного языка внутри переписываемой части. */
  function hints(source: string, start: number, end: number, fragment: boolean): Hints {
    const a = analysis?.text === source ? analysis.result : null;
    const c = clarity?.text === source ? clarity.report : null;
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

  async function run() {
    error = '';
    const source = text;
    const { start, end, fragment } = range;
    const part = source.slice(start, end);
    const prompt = rewritePrompt(part, goal, hints(source, start, end, fragment));
    const controller = new AbortController();
    running = controller;
    const started = performance.now();
    // Подписи берутся до запроса: пока он идёт, в настройках могут выбрать другое.
    const meta = { goal, title: p.title, model: ps.model };
    try {
      const result = await rewrite(prompt, toRequest(llm), controller.signal);
      // В настольном приложении запрос не прерывается, отменённый ответ просто отбрасываем.
      if (controller.signal.aborted) return;
      draft = {
        source,
        start,
        end,
        fragment,
        result,
        ...meta,
        seconds: (performance.now() - started) / 1000,
        missing: missingNumbers(part, result),
      };
    } catch (e) {
      if (!controller.signal.aborted) {
        error = e instanceof LlmError ? e.message : `Не получилось: ${(e as Error).message}`;
      }
    } finally {
      if (running === controller) running = null;
    }
  }

  function apply() {
    if (!draft || draft.source !== text) return;
    const d = draft;
    const next = d.source.slice(0, d.start) + d.result + d.source.slice(d.end);
    applied = { before: d.source, after: next };
    draft = null;
    onapply(next, d.fragment ? { start: d.start, end: d.start + d.result.length } : null);
  }

  function restore() {
    if (!applied || applied.after !== text) return;
    onrestore(applied.before);
    applied = null;
  }
</script>

{#if configured}
  <section class="llm rewrite" aria-labelledby="rewrite-title">
    <h2 id="rewrite-title">Переписать с помощью LLM</h2>
    <label class="target">
      Для кого переписать
      <select value={goal.id} onchange={(e) => setAudience(e.currentTarget.value)}>
        {#each AUDIENCES as a (a.id)}
          <option value={a.id}>{a.title}</option>
        {/each}
      </select>
    </label>
    <p class="privacy" class:local>
      {#if local}
        {p.title}, {ps.model}: текст останется на этом компьютере.
      {:else}
        {ps.model}: {range.fragment ? 'фрагмент' : 'текст'} ({charCount(size)}) будет отправлен: {target}.
      {/if}
      <a href="#settings">Изменить</a>
    </p>
    <div class="toolbar">
      <button type="button" onclick={run} disabled={size === 0 || tooLong || running !== null}>
        {running ? 'Переписываю…' : range.fragment ? 'Переписать выделенное' : 'Переписать текст'}
      </button>
      {#if running}
        <!-- Запрос из настольного приложения не прерывается: панель просто перестаёт его ждать. -->
        <button
          type="button"
          class="secondary"
          onclick={() => {
            running?.abort();
            running = null;
          }}>Отменить</button
        >
      {/if}
    </div>
    {#if tooLong}
      <p class="warning">
        За один раз можно переписать до {charCount(MAX_REWRITE_CHARS)}. Выделите часть текста в поле ввода.
      </p>
    {:else if !range.fragment && !draft}
      <p class="note">
        Чтобы переписать часть текста, выделите её в поле ввода или нажмите на предложение в списке самых
        длинных.
      </p>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}

    {#if draft}
      <h3>Черновик {draft.fragment ? 'фрагмента' : 'текста'}: {draft.goal.title.toLowerCase()}</h3>
      <textarea class="draft" readonly value={draft.result} aria-label="Переписанный текст"></textarea>
      {#if draft.missing.length}
        <p class="warning">
          В черновике нет чисел из исходного текста: {draft.missing.join(', ')}. Проверьте, не потерялись
          ли факты.
        </p>
      {/if}
      {#if stale}
        <p class="warning">Текст в поле изменился после запроса: заменить нельзя, перепишите заново.</p>
      {/if}
      <div class="toolbar">
        <button type="button" onclick={apply} disabled={stale}>
          {draft.fragment ? 'Заменить фрагмент' : 'Заменить текст'}
        </button>
        <button type="button" class="secondary" onclick={() => (draft = null)}>Отбросить</button>
      </div>
      <p class="note">
        {draft.title} · {draft.model} · {fmt(draft.seconds)} с. Модель может исказить смысл: прочитайте
        черновик, прежде чем заменять.
      </p>
    {/if}

    {#if canRestore}
      <p class="note">Текст заменён. Как изменилась оценка — в разделе «Было и стало».</p>
      <button type="button" class="secondary" onclick={restore}>Вернуть как было</button>
    {/if}
  </section>
{/if}
