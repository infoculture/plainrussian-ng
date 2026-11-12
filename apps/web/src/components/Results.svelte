<script lang="ts">
  import type { Measurement, SentenceSpan } from '../lib/types';
  import { plural } from '../lib/segments';
  import Compare from './Compare.svelte';
  import LongSentences from './LongSentences.svelte';
  import {
    AUDIENCES,
    SCALE_MAX as MAX,
    SCALE_MIN as MIN,
    audience,
    onScale,
    setAudience,
    verdict,
  } from '../lib/target.svelte.ts';

  let {
    measurement,
    text,
    sentences,
    onselect,
    baseline,
    edited,
    onremember,
  }: {
    measurement: Measurement;
    /** Текст и его предложения — для списка самых длинных. */
    text: string;
    sentences: SentenceSpan[];
    onselect: (start: number, end: number) => void;
    /** Показатели исходного варианта для сравнения «было и стало»; null, если его нет. */
    baseline: Measurement | null;
    /** Текст правили после того, как запомнили исходный вариант. */
    edited: boolean;
    onremember: () => void;
  } = $props();

  const fmt = (n: number, digits = 2) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let c = $derived(measurement.counts);
  let f = $derived(measurement.features);
  let ix = $derived(measurement.indexes);
  let calibration = $derived(measurement.calibration);
  let school = $derived(measurement.school);
  let schoolCalibration = $derived(measurement.school_calibration);
  let tooShort = $derived(c.sentences < 2 || c.words < 10);

  // Корпус размечен от 1 до 17 («17 и выше»), за этими границами формулы экстраполируют.
  // Поэтому и главная оценка, и отдельные формулы показываются в пределах шкалы.
  let average = $derived(ix?.average.value ?? 0);
  let score = $derived(onScale(average, 1));
  let years = $derived(plural(Math.min(Math.max(Math.round(average), MIN), MAX), ['год', 'года', 'лет']));

  // Цель: оценка сравнивается с верхней границей выбранной аудитории.
  let goal = $derived(audience());
  let outcome = $derived(verdict(average, goal, calibration.average_error));
  let position = $derived(Math.min(Math.max(average, MIN), MAX) - MIN);

  let formulas = $derived(
    ix
      ? [
          { name: 'Flesch-Kincaid', index: ix.fk },
          { name: 'Coleman-Liau', index: ix.cl },
          { name: 'SMOG', index: ix.smog },
          { name: 'Automated Readability Index', index: ix.ari },
          { name: 'Dale-Chall (упрощённая)', index: ix.dc },
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

  <label class="target">
    Для кого текст
    <select value={goal.id} onchange={(e) => setAudience(e.currentTarget.value)}>
      {#each AUDIENCES as a (a.id)}
        <option value={a.id}>{a.title}</option>
      {/each}
    </select>
  </label>
  <!-- Шкала: закрашена зона цели, отметка — оценка текста. Размеры — атрибутами SVG:
       встроенные стили запрещает CSP. Смысл дублирует вердикт под шкалой. -->
  <svg class="scale" viewBox="0 0 {MAX - MIN} 2" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <rect class="scale-track" y="0.6" width={MAX - MIN} height="0.8" />
    <rect class="scale-zone" y="0.6" width={goal.max - MIN} height="0.8" />
    <line class="scale-mark {outcome.level}" x1={position} x2={position} y1="0" y2="2" vector-effect="non-scaling-stroke" />
  </svg>
  <div class="scale-ends">
    <span aria-hidden="true">{MIN}</span>
    <span>цель — до {goal.max} лет обучения</span>
    <span aria-hidden="true">{MAX}+</span>
  </div>
  <p class="verdict {outcome.level}">{outcome.text}</p>

  <p class="note">
    Среднее пяти формул по шкале от {MIN} до {MAX}+ лет обучения. На размеченных текстах
    оценка ошибается в среднем на {fmt(calibration.average_error, 1)} года.
  </p>

  {#if school}
    <p class="school">По школьной шкале — <strong>{school.grade}</strong>.</p>
    <p class="note">
      Школьная шкала — для текстов, написанных для школьников: учебных и научно-популярных. Она
      подобрана на текстах проверочных работ ФИПИ для 2–11 классов и ошибается на них в среднем на
      {fmt(schoolCalibration.average_error, 1)} класса. Художественные тексты она занижает, а тексты сложнее
      школьных не различает: для них верна главная оценка.
    </p>
  {/if}

  {#if tooShort}
    <p class="warning">Текст слишком короткий: оценка надёжна от 2 предложений и 10 слов.</p>
  {/if}

  <Compare was={baseline} now={measurement} {edited} {onremember} />

  <LongSentences {text} {sentences} {onselect} />

  <h2>Индексы читаемости</h2>
  <table>
    <thead>
      <tr><th>Формула</th><th class="num">Лет обучения</th><th>Уровень</th></tr>
    </thead>
    <tbody>
      {#each formulas as row (row.name)}
        <tr><td>{row.name}</td><td class="num">{onScale(row.index.value, 2)}</td><td>{row.index.grade}</td></tr>
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
  <p class="note">Калибровка {calibration.version}, школьная шкала {schoolCalibration.version}.</p>
{:else}
  <p class="warning">В тексте нет слов, оценивать нечего.</p>
{/if}
