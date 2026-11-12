<script lang="ts">
  import { change, facts } from '../lib/compare';
  import { onScale } from '../lib/target.svelte.ts';
  import type { Measurement } from '../lib/types';

  /** Как изменились оценка, длина предложений и доля сложных слов. Ничего, если в варианте нет слов. */
  let { was, now }: { was: Measurement; now: Measurement } = $props();

  const fmt = (n: number) => n.toLocaleString('ru-RU', { maximumFractionDigits: 1 });

  let before = $derived(facts(was));
  let after = $derived(facts(now));
  let outcome = $derived(before && after ? change(before.grade, after.grade) : null);
</script>

{#if before && after && outcome}
  <p class="verdict {outcome.tone}">{outcome.text}</p>
  <dl class="metrics wide">
    <div><dt>Оценка</dt><dd>{onScale(before.grade, 1)} → {onScale(after.grade, 1)}</dd></div>
    <div><dt>Слов в предложении</dt><dd>{fmt(before.sentence)} → {fmt(after.sentence)}</dd></div>
    <div><dt>Сложных слов</dt><dd>{fmt(before.complex)}% → {fmt(after.complex)}%</dd></div>
  </dl>
{/if}
