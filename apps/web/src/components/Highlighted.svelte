<script lang="ts">
  import type { Annotation, ClarityReport } from '../lib/types';
  import { CLARITY_KINDS, clarityMarks } from '../lib/clarity';
  import {
    buildBlocks,
    complexWordMarks,
    HIGHLIGHT_LIMIT,
    LONG_SENTENCE,
    VERY_LONG_SENTENCE,
    plural,
  } from '../lib/segments';

  let {
    text,
    annotation,
    clarity = null,
    onselect,
  }: {
    text: string;
    annotation: Annotation;
    clarity?: ClarityReport | null;
    /** Щелчок по выделенному: фрагмент с этими смещениями нужно показать в поле ввода. */
    onselect: (start: number, end: number) => void;
  } = $props();

  let mode = $state<'words' | 'clarity'>('words');
  let showClarity = $derived(mode === 'clarity' && clarity !== null);
  let marks = $derived(showClarity && clarity ? clarityMarks(clarity) : complexWordMarks(annotation));
  let blocks = $derived(buildBlocks(text, annotation.sentences, marks));
  const words = (n: number) => `${n} ${plural(n, ['слово', 'слова', 'слов'])}`;

  // Один обработчик на всю подсветку: отметок в длинном тексте тысячи.
  function pick(event: MouseEvent) {
    // Выделение мышью — не щелчок: текст из подсветки можно копировать.
    if (!getSelection()?.isCollapsed) return;
    const target = (event.target as Element).closest<HTMLElement>('[data-start]');
    if (target) onselect(Number(target.dataset.start), Number(target.dataset.end));
  }
</script>

<details class="highlighted" open>
  <summary>Подсветка</summary>
  {#if clarity}
    <div class="modes" role="radiogroup" aria-label="Что подсвечивать">
      <button type="button" role="radio" aria-checked={!showClarity} onclick={() => (mode = 'words')}>
        Сложные слова
      </button>
      <button type="button" role="radio" aria-checked={showClarity} onclick={() => (mode = 'clarity')}>
        Ясный язык
      </button>
    </div>
  {/if}
  <p class="legend">
    {#if showClarity}
      {#each CLARITY_KINDS as k (k.kind)}
        <span class="clarity-{k.kind}">{k.label.toLowerCase()}</span>{' '}
      {/each}
    {:else}
      <mark class="complex">сложное слово</mark> — больше 4 слогов;
      <span class="s l1">предложение</span> — {LONG_SENTENCE}–{VERY_LONG_SENTENCE - 1} слов;
      <span class="s l2">предложение</span> — {VERY_LONG_SENTENCE} слов и больше.
    {/if}
  </p>
  {#if text.length > HIGHLIGHT_LIMIT}
    <p class="warning">Подсвечены первые {HIGHLIGHT_LIMIT.toLocaleString('ru-RU')} знаков. Оценка посчитана по всему тексту.</p>
  {/if}
  <p class="note">Нажмите на выделенное — оно выделится в поле ввода, и его можно сразу править.</p>
  <!-- Щелчок — удобство для мыши. С клавиатуры к тем же местам ведут кнопки в панели
       оценки: длинные предложения и находки ясного языка. -->
  <!-- Разметка в одну строку: переносы внутри шаблона попали бы в текст. -->
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="annotated" onclick={pick}>{#each blocks as b, i (i)}{#if b.kind === 'gap'}{b.text}{:else}<span class="s l{showClarity ? 0 : b.level}" title={words(b.words)} data-start={!showClarity && b.level > 0 ? b.start : undefined} data-end={!showClarity && b.level > 0 ? b.end : undefined}>{#each b.parts as p, j (j)}{#if p.mark}<mark class={p.mark.cls} title={p.mark.title} data-start={p.mark.start} data-end={p.mark.end}>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>{/if}{/each}</div>
</details>
