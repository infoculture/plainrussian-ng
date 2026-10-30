<script lang="ts">
  import type { Annotation } from '../lib/types';
  import { buildBlocks, HIGHLIGHT_LIMIT, LONG_SENTENCE, VERY_LONG_SENTENCE, plural } from '../lib/segments';

  let { text, annotation }: { text: string; annotation: Annotation } = $props();

  let blocks = $derived(buildBlocks(text, annotation));
  const words = (n: number) => `${n} ${plural(n, ['слово', 'слова', 'слов'])}`;
</script>

<details class="highlighted" open>
  <summary>Подсветка</summary>
  <p class="legend">
    <mark>сложное слово</mark> — больше 4 слогов;
    <span class="s l1">предложение</span> — {LONG_SENTENCE}–{VERY_LONG_SENTENCE - 1} слов;
    <span class="s l2">предложение</span> — {VERY_LONG_SENTENCE} слов и больше.
  </p>
  {#if text.length > HIGHLIGHT_LIMIT}
    <p class="warning">Подсвечены первые {HIGHLIGHT_LIMIT.toLocaleString('ru-RU')} знаков. Оценка посчитана по всему тексту.</p>
  {/if}
  <!-- Разметка в одну строку: переносы внутри шаблона попали бы в текст. -->
  <div class="annotated">{#each blocks as b, i (i)}{#if b.kind === 'gap'}{b.text}{:else}<span class="s l{b.level}" title={words(b.words)}>{#each b.parts as p, j (j)}{#if p.syllables}<mark title="{p.syllables} {plural(p.syllables, ['слог', 'слога', 'слогов'])}">{p.text}</mark>{:else}{p.text}{/if}{/each}</span>{/if}{/each}</div>
</details>
