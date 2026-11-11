<script lang="ts">
  import { LONG_SENTENCE, plural } from '../lib/segments';
  import type { SentenceSpan } from '../lib/types';

  let {
    text,
    sentences,
    onselect,
  }: {
    text: string;
    sentences: SentenceSpan[];
    /** Предложение с этими смещениями нужно показать в поле ввода. */
    onselect: (start: number, end: number) => void;
  } = $props();

  /** Сколько предложений показать: с них проще всего начать правку. */
  const TOP = 5;
  /** Начало предложения в списке, знаков. */
  const SNIPPET = 80;

  let long = $derived(sentences.filter((s) => s.words >= LONG_SENTENCE));
  // Сортируется копия: long остаётся в порядке текста.
  let longest = $derived([...long].sort((a, b) => b.words - a.words || a.start - b.start).slice(0, TOP));

  function snippet(s: SentenceSpan): string {
    const sentence = text.slice(s.start, s.end).replace(/\s+/g, ' ').trim();
    return sentence.length > SNIPPET ? `${sentence.slice(0, SNIPPET).trimEnd()}…` : sentence;
  }
</script>

{#if longest.length}
  <h2>Самые длинные предложения</h2>
  <ol class="jump-list">
    {#each longest as s (s.start)}
      <li>
        <button type="button" class="jump" onclick={() => onselect(s.start, s.end)}>
          <span class="count">{s.words} {plural(s.words, ['слово', 'слова', 'слов'])}</span>
          {snippet(s)}
        </button>
      </li>
    {/each}
  </ol>
  <p class="note">
    Нажмите на предложение — оно выделится в поле ввода.
    {#if long.length > TOP}Всего предложений от {LONG_SENTENCE} слов: {long.length}.{/if}
  </p>
{/if}
