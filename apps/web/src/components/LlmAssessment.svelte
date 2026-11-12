<script lang="ts" module>
  import type { Assessment } from '../lib/llm/assessment.ts';

  export interface AssessmentResult {
    /** Текст, который оценивала модель. */
    text: string;
    title: string;
    model: string;
    seconds: number;
    assessment: Assessment;
  }
</script>

<script lang="ts">
  import { ISSUE_KINDS, clampGrade, gradeLabel, quoteFound } from '../lib/llm/assessment.ts';
  import { plural } from '../lib/segments';
  import { audience, verdict } from '../lib/target.svelte.ts';

  let {
    result,
    text,
    formula,
    onclose,
  }: {
    result: AssessmentResult;
    /** Текст в поле сейчас: если он изменился, оценка устарела. */
    text: string;
    /** Оценка формулами того же текста — для сравнения. */
    formula: number | null;
    onclose: () => void;
  } = $props();

  const fmt = (n: number, digits = 1) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let grade = $derived(clampGrade(result.assessment.grade));
  // Ошибка оценки модели не измерена, поэтому допуска нет: выше цели — значит сложнее.
  let outcome = $derived(verdict(grade, audience(), 0));
  let stale = $derived(result.text !== text);
</script>

<section class="llm-card" aria-labelledby="assessment-title">
  <h2 id="assessment-title">Оценка с помощью LLM</h2>
  <div class="summary">
    <div class="score">{grade >= 17 ? '17+' : fmt(grade)}</div>
    <div>
      <div class="score-label">{plural(Math.round(grade), ['год', 'года', 'лет'])} обучения по оценке модели</div>
      <div class="audience">{gradeLabel(grade)}</div>
    </div>
  </div>
  <p class="verdict {outcome.level}">{outcome.text}</p>
  {#if formula !== null && !stale}
    <p class="note">Формулы: {fmt(Math.min(Math.max(formula, 1), 17))}, модель: {fmt(grade)}.</p>
  {/if}
  <p>{result.assessment.summary}</p>
  {#if result.assessment.issues.length}
    <h3>Что мешает понять текст</h3>
    <ol class="issues">
      {#each result.assessment.issues as issue, i (i)}
        <li>
          <span class="kind">{ISSUE_KINDS[issue.kind]}</span>
          <blockquote>{issue.quote}</blockquote>
          {#if !quoteFound(result.text, issue.quote)}
            <p class="warning">Такой фразы в тексте нет: модель пересказала, а не процитировала.</p>
          {/if}
          <p>{issue.comment}</p>
          {#if issue.suggestion}<p class="suggestion">Проще: {issue.suggestion}</p>{/if}
        </li>
      {/each}
    </ol>
  {/if}
  <p><strong>Совет:</strong> {result.assessment.advice}</p>
  <p class="note">
    {result.title} · {result.model} · {fmt(result.seconds)} с{#if stale}. Текст с тех пор изменился.{/if}
  </p>
  <button type="button" class="link" onclick={onclose}>Скрыть оценку</button>
</section>
