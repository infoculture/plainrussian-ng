<script lang="ts">
  import type { Measurement } from '../lib/types';
  import { plural } from '../lib/segments';

  let { measurement }: { measurement: Measurement } = $props();

  const fmt = (n: number, digits = 2) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let m = $derived(measurement.metrics);
  let ix = $derived(measurement.indexes);
  // Старый бот предупреждал: оценка корректна от 2 предложений и 10 слов.
  let tooShort = $derived(m.n_sentences < 2 || m.n_words < 10);

  let formulas = $derived([
    { name: 'SMOG', value: ix.index_SMOG, grade: ix.grade_SMOG },
    { name: 'Flesch-Kincaid', value: ix.index_fk, grade: ix.grade_fk },
    { name: 'Coleman-Liau', value: ix.index_cl, grade: ix.grade_cl },
    { name: 'Automated Readability Index', value: ix.index_ari, grade: ix.grade_ari },
    { name: 'Dale-Chale (упрощённая)', value: ix.index_dc, grade: ix.grade_dc },
  ]);
</script>

<div class="summary">
  <div class="score">{fmt(ix.index_SMOG, 1)}</div>
  <div>
    <div class="score-label">{plural(Math.round(ix.index_SMOG), ['год', 'года', 'лет'])} обучения по SMOG</div>
    <div class="audience">{ix.grade_SMOG}</div>
  </div>
</div>

{#if tooShort}
  <p class="warning">Текст слишком короткий: оценка надёжна от 2 предложений и 10 слов.</p>
{/if}

<h2>Индексы читаемости</h2>
<table>
  <thead>
    <tr><th>Формула</th><th class="num">Лет обучения</th><th>Уровень</th></tr>
  </thead>
  <tbody>
    {#each formulas as f (f.name)}
      <tr><td>{f.name}</td><td class="num">{fmt(f.value)}</td><td>{f.grade}</td></tr>
    {/each}
  </tbody>
</table>

<h2>Показатели текста</h2>
<dl class="metrics">
  <div><dt>Слов</dt><dd>{fmt(m.n_words)}</dd></div>
  <div><dt>Предложений</dt><dd>{fmt(m.n_sentences)}</dd></div>
  <div><dt>Слов в предложении</dt><dd>{fmt(m.avg_slen, 1)}</dd></div>
  <div><dt>Слогов в слове</dt><dd>{fmt(m.avg_syl)}</dd></div>
  <div><dt>Сложных слов</dt><dd>{fmt(m.n_complex_words)} ({fmt(m.c_share, 1)}%)</dd></div>
  <div><dt>Слогов</dt><dd>{fmt(m.n_syllabes)}</dd></div>
  <div><dt>Знаков</dt><dd>{fmt(m.chars)}</dd></div>
  <div><dt>Букв</dt><dd>{fmt(m.letters)}</dd></div>
</dl>
