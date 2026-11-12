<script lang="ts" module>
  import type { Audience } from '../lib/target.svelte.ts';
  import type { Analysis, Measurement } from '../lib/types';

  export interface Draft {
    /** Текст поля на момент запроса и границы переписанного в нём. */
    source: string;
    start: number;
    end: number;
    fragment: boolean;
    /** Что было на месте черновика: с ним сверяются числа. */
    original: string;
    /** Оценка исходного текста; null, если к запросу она ещё не была готова. */
    before: Measurement | null;
    result: string;
    /** Текст поля, каким он станет после замены. */
    next: string;
    /** Оценка `next`; null, пока считается. */
    after: Analysis | null;
    goal: Audience;
    title: string;
    model: string;
    seconds: number;
    /** Числа исходника, которых нет в черновике. */
    missing: string[];
  }
</script>

<script lang="ts">
  import { onScale, verdict } from '../lib/target.svelte.ts';
  import Delta from './Delta.svelte';

  let {
    draft,
    stale,
    busy,
    onapply,
    onagain,
    ondiscard,
  }: {
    draft: Draft;
    /** Текст в поле изменился после запроса: черновик заменять нельзя. */
    stale: boolean;
    /** Идёт новый запрос к модели. */
    busy: boolean;
    onapply: () => void;
    /** Переписать черновик ещё проще. */
    onagain: () => void;
    ondiscard: () => void;
  } = $props();

  const fmt = (n: number) => n.toLocaleString('ru-RU', { maximumFractionDigits: 1 });

  let measurement = $derived(draft.after?.measurement ?? null);
  // Подходит ли черновик аудитории, для которой его писали. Допуск — ошибка формул.
  let fit = $derived(
    measurement?.indexes
      ? verdict(measurement.indexes.average.value, draft.goal, measurement.calibration.average_error)
      : null,
  );
</script>

<section class="llm-card" aria-labelledby="draft-title" aria-busy={draft.after === null}>
  <h2 id="draft-title">Черновик {draft.fragment ? 'фрагмента' : 'текста'}: {draft.goal.title.toLowerCase()}</h2>
  {#if !measurement}
    <p class="note">Считаю оценку черновика…</p>
  {:else}
    {#if draft.before}
      <Delta was={draft.before} now={measurement} />
    {:else if measurement.indexes}
      <p>Оценка черновика по формулам: {onScale(measurement.indexes.average.value, 1)}.</p>
    {/if}
    {#if fit}
      <p class="verdict {fit.level}">Цель — до {draft.goal.max} лет обучения. {fit.text}</p>
    {/if}
    {#if draft.fragment}
      <p class="note">Оценка — для всего текста с заменённым фрагментом.</p>
    {/if}
  {/if}

  <div class="draft-columns">
    <label>
      Сейчас
      <textarea readonly value={draft.original}></textarea>
    </label>
    <label>
      Черновик
      <textarea readonly value={draft.result}></textarea>
    </label>
  </div>

  {#if draft.missing.length}
    <p class="warning">
      В черновике нет чисел из исходного текста: {draft.missing.join(', ')}. Проверьте, не потерялись ли
      факты.
    </p>
  {/if}
  {#if stale}
    <p class="warning">Текст в поле изменился после запроса: заменить нельзя, перепишите заново.</p>
  {/if}
  <div class="toolbar">
    <button type="button" onclick={onapply} disabled={stale || busy}>
      {draft.fragment ? 'Заменить фрагмент' : 'Заменить текст'}
    </button>
    <button type="button" class="secondary" onclick={onagain} disabled={stale || busy}>
      {busy ? 'Переписываю…' : 'Ещё проще'}
    </button>
    <button type="button" class="secondary" onclick={ondiscard}>Отбросить</button>
  </div>
  <p class="note">
    {draft.title} · {draft.model} · {fmt(draft.seconds)} с. Модель может исказить смысл: прочитайте черновик,
    прежде чем заменять.
  </p>
</section>
