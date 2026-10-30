<script lang="ts">
  import { ISSUE_KINDS, clampGrade, gradeLabel, quoteFound, type Assessment } from '../lib/llm/assessment.ts';
  import { LlmError, assess, provider } from '../lib/llm/providers.ts';
  import { toRequest } from '../lib/llm/settings.ts';
  import { isConfigured, llm } from '../lib/llm/store.svelte.ts';
  import { plural } from '../lib/segments';

  let { text, formula }: { text: string; formula: number | null } = $props();

  const fmt = (n: number, digits = 1) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let p = $derived(provider(llm.provider));
  let ps = $derived(llm.providers[llm.provider]);
  let configured = $derived(isConfigured());
  let running = $state<AbortController | null>(null);
  let error = $state('');
  let result = $state<{
    text: string;
    title: string;
    model: string;
    seconds: number;
    assessment: Assessment;
  } | null>(null);

  let grade = $derived(result ? clampGrade(result.assessment.grade) : 0);
  let stale = $derived(result !== null && result.text !== text);
  let destination = $derived.by(() => {
    if (p.id !== 'custom') return p.title;
    try {
      return new URL(ps.baseUrl).host;
    } catch {
      return 'указанный сервис';
    }
  });

  async function run() {
    error = '';
    const controller = new AbortController();
    running = controller;
    const started = performance.now();
    const current = text;
    try {
      const assessment = await assess(current, toRequest(llm), controller.signal);
      // В настольном приложении запрос не прерывается, отменённый ответ просто отбрасываем.
      if (controller.signal.aborted) return;
      result = {
        text: current,
        title: p.title,
        model: ps.model,
        seconds: (performance.now() - started) / 1000,
        assessment,
      };
    } catch (e) {
      if (!controller.signal.aborted) {
        error = e instanceof LlmError ? e.message : `Не получилось: ${(e as Error).message}`;
      }
    } finally {
      if (running === controller) running = null;
    }
  }
</script>

<section class="llm" aria-labelledby="llm-title">
  <h2 id="llm-title">Оценка с помощью LLM</h2>
  {#if !configured}
    <p class="note">
      Языковая модель может оценить текст целиком и объяснить, что мешает его понять.
      <a href="#settings">Подключить модель</a>
    </p>
  {:else}
    <p class="privacy" class:local={p.local}>
      {#if p.local}
        {p.title}, {ps.model}: текст останется на этом компьютере.
      {:else}
        {ps.model}: текст ({text.length.toLocaleString('ru-RU')} знаков) будет отправлен: {destination}.
      {/if}
      <a href="#settings">Изменить</a>
    </p>
    <div class="toolbar">
      <button type="button" onclick={run} disabled={!text.trim() || running !== null}>
        {running ? 'Оцениваю…' : 'Оценить'}
      </button>
      {#if running}
        <button type="button" class="secondary" onclick={() => running?.abort()}>Отменить</button>
      {/if}
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if result}
    <div class="summary">
      <div class="score">{grade >= 17 ? '17+' : fmt(grade)}</div>
      <div>
        <div class="score-label">{plural(Math.round(grade), ['год', 'года', 'лет'])} обучения по оценке модели</div>
        <div class="audience">{gradeLabel(grade)}</div>
      </div>
    </div>
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
  {/if}
</section>
