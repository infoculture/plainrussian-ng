<script lang="ts">
  import type { Measurement } from '../lib/types';
  import { plural } from '../lib/segments';

  let { measurement }: { measurement: Measurement } = $props();

  const fmt = (n: number, digits = 2) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let c = $derived(measurement.counts);
  let f = $derived(measurement.features);
  let ix = $derived(measurement.indexes);
  let calibration = $derived(measurement.calibration);
  let tooShort = $derived(c.sentences < 2 || c.words < 10);

  // Корпус размечен от 1 до 17 («17 и выше»), за этими границами формулы экстраполируют.
  const MIN = 1;
  const MAX = 17;
  let average = $derived(ix?.average.value ?? 0);
  let score = $derived(average < MIN ? `<${MIN}` : average >= MAX ? `${MAX}+` : fmt(average, 1));
  let years = $derived(plural(Math.min(Math.max(Math.round(average), MIN), MAX), ['год', 'года', 'лет']));

  let formulas = $derived(
    ix
      ? [
          { name: 'Flesch-Kincaid', index: ix.fk },
          { name: 'Coleman-Liau', index: ix.cl },
          { name: 'SMOG', index: ix.smog },
          { name: 'Automated Readability Index', index: ix.ari },
          { name: 'Dale-Chale (упрощённая)', index: ix.dc },
        ]
      : [],
  );
</script>

{#if ix && f}
  <div class="summary">
    <div class="score">{score}</div>
    <div>
      <div class="score-label">{years} обучения</div>
      <div class="audience">{ix.average.grade}</div>
    </div>
  </div>
  <p class="note">
    Среднее пяти формул по шкале от {MIN} до {MAX}+ лет обучения. На размеченных текстах
    оценка ошибается в среднем на {fmt(calibration.average_error, 1)} года.
  </p>

  {#if tooShort}
    <p class="warning">Текст слишком короткий: оценка надёжна от 2 предложений и 10 слов.</p>
  {/if}

  <h2>Индексы читаемости</h2>
  <table>
    <thead>
      <tr><th>Формула</th><th class="num">Лет обучения</th><th>Уровень</th></tr>
    </thead>
    <tbody>
      {#each formulas as row (row.name)}
        <tr><td>{row.name}</td><td class="num">{fmt(row.index.value)}</td><td>{row.index.grade}</td></tr>
      {/each}
    </tbody>
  </table>

  <h2>Показатели текста</h2>
  <dl class="metrics">
    <div><dt>Слов</dt><dd>{fmt(c.words)}</dd></div>
    <div><dt>Предложений</dt><dd>{fmt(c.sentences)}</dd></div>
    <div><dt>Слов в предложении</dt><dd>{fmt(f.words_per_sentence, 1)}</dd></div>
    <div><dt>Слогов в слове</dt><dd>{fmt(f.syllables_per_word)}</dd></div>
    <div><dt>Сложных слов</dt><dd>{fmt(c.complex_words)} ({fmt(f.complex_per_100_words, 1)}%)</dd></div>
    <div><dt>Букв в слове</dt><dd>{fmt(f.letters_per_word, 1)}</dd></div>
    <div><dt>Чисел</dt><dd>{fmt(c.numbers)}</dd></div>
    <div><dt>Знаков</dt><dd>{fmt(c.chars)}</dd></div>
  </dl>
  <p class="note">Калибровка {calibration.version}.</p>
{:else}
  <p class="warning">В тексте нет слов, оценивать нечего.</p>
{/if}
