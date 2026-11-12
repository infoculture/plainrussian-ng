<script lang="ts">
  import { plural } from '../lib/segments';
  import { SCALE_MAX, SCALE_MIN, onScale } from '../lib/target.svelte.ts';
  import type { Measurement } from '../lib/types';

  let {
    was,
    now,
    edited,
    onremember,
  }: {
    /** Исходный вариант; null, если его ещё не запомнили. */
    was: Measurement | null;
    now: Measurement;
    /** Текст правили после того, как запомнили исходный вариант. */
    edited: boolean;
    /** Запомнить текущий вариант как исходный. */
    onremember: () => void;
  } = $props();

  const fmt = (n: number) => n.toLocaleString('ru-RU', { maximumFractionDigits: 1 });
  const clamp = (value: number) => Math.min(Math.max(value, SCALE_MIN), SCALE_MAX);

  /** Что нужно для сравнения; null, если в варианте нет слов. */
  function facts(m: Measurement | null) {
    return m?.indexes && m.features
      ? {
          grade: m.indexes.average.value,
          sentence: m.features.words_per_sentence,
          complex: m.features.complex_per_100_words,
        }
      : null;
  }

  let before = $derived(facts(was));
  let after = $derived(facts(now));

  interface Change {
    tone: 'ok' | 'hard' | 'same';
    text: string;
  }

  /** Сравнивает оценки в пределах шкалы: за её границами числа смысла не имеют. */
  function change(from: number, to: number): Change {
    const diff = clamp(to) - clamp(from);
    const size = Math.round(Math.abs(diff) * 10) / 10;
    if (size === 0) {
      // Обе оценки выше шкалы: на шкале то же «17+», хотя текст изменился.
      if (from >= SCALE_MAX && to >= SCALE_MAX && Math.abs(to - from) >= 0.05) {
        return to < from
          ? { tone: 'ok', text: 'Стало проще, но оценка всё ещё выше шкалы.' }
          : { tone: 'hard', text: 'Стало сложнее, оценка по-прежнему выше шкалы.' };
      }
      return { tone: 'same', text: 'Оценка не изменилась.' };
    }
    // Дробное число требует родительного падежа единственного числа: «на 4,6 года».
    const unit = Number.isInteger(size) ? plural(size, ['год', 'года', 'лет']) : 'года';
    const by = `${fmt(size)} ${unit} обучения`;
    return diff < 0
      ? { tone: 'ok', text: `Стало проще на ${by}.` }
      : { tone: 'hard', text: `Стало сложнее на ${by}.` };
  }

  let outcome = $derived(before && after ? change(before.grade, after.grade) : null);
</script>

<h2>Было и стало</h2>
{#if !before}
  <p class="note">Запомните этот вариант — после правки здесь появится сравнение с ним.</p>
  <button type="button" class="link" onclick={onremember}>Запомнить вариант</button>
{:else if !edited || !after || !outcome}
  <p class="note">Исходный вариант запомнен. Правьте текст — здесь появится сравнение с ним.</p>
{:else}
  <p class="verdict {outcome.tone}">{outcome.text}</p>
  <dl class="metrics wide">
    <div><dt>Оценка</dt><dd>{onScale(before.grade, 1)} → {onScale(after.grade, 1)}</dd></div>
    <div><dt>Слов в предложении</dt><dd>{fmt(before.sentence)} → {fmt(after.sentence)}</dd></div>
    <div><dt>Сложных слов</dt><dd>{fmt(before.complex)}% → {fmt(after.complex)}%</dd></div>
  </dl>
  <button type="button" class="link" onclick={onremember}>Считать этот вариант исходным</button>
{/if}
