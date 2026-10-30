<script lang="ts">
  import { ISSUE_KINDS, clampGrade, gradeLabel, quoteFound, type Assessment } from '../lib/llm/assessment.ts';
  import { LlmError, PROVIDERS, assess, listModels, provider } from '../lib/llm/providers.ts';
  import { loadSecrets, loadSettings, saveSettings, toRequest } from '../lib/llm/settings.ts';
  import { isDesktop } from '../lib/platform.ts';
  import { plural } from '../lib/segments';
  import { onMount } from 'svelte';

  let { text, formula }: { text: string; formula: number | null } = $props();

  const fmt = (n: number, digits = 1) => n.toLocaleString('ru-RU', { maximumFractionDigits: digits });

  let settings = $state(loadSettings());
  let p = $derived(provider(settings.provider));
  let ps = $derived(settings.providers[settings.provider]);
  let models = $state<string[]>([]);
  let running = $state<AbortController | null>(null);
  let error = $state('');
  let result = $state<{
    text: string;
    title: string;
    model: string;
    seconds: number;
    assessment: Assessment;
  } | null>(null);

  $effect(() => saveSettings($state.snapshot(settings)));
  onMount(() => {
    loadSecrets(settings);
  });
  $effect(() => {
    // Подсказки моделей сбрасываются при смене провайдера.
    models = provider(settings.provider).models;
  });

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
      const assessment = await assess(current, toRequest(settings), controller.signal);
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

  async function loadModels() {
    error = '';
    try {
      models = await listModels(toRequest(settings));
      if (!ps.model && models.length) ps.model = models[0];
    } catch (e) {
      error = e instanceof LlmError ? e.message : (e as Error).message;
    }
  }
</script>

<section class="llm" aria-labelledby="llm-title">
  <h2 id="llm-title">Оценка с помощью LLM</h2>
  <p class="note">
    Другой взгляд на текст: языковая модель оценивает его целиком и объясняет, что мешает понять. Нужен
    свой ключ API или модель на этом компьютере.
  </p>

  <details class="llm-settings" open={p.key === 'required' ? !ps.apiKey : !ps.model}>
    <summary>{p.title}{ps.model ? ` · ${ps.model}` : ''}</summary>
    <label>
      Провайдер
      <select bind:value={settings.provider}>
        {#each PROVIDERS as option (option.id)}
          <option value={option.id}>{option.title}</option>
        {/each}
      </select>
    </label>
    {#if p.editableUrl}
      <label>
        Адрес API
        <input bind:value={ps.baseUrl} spellcheck="false" placeholder="https://…/v1" />
      </label>
    {/if}
    <label>
      Модель
      <input bind:value={ps.model} list="llm-models" spellcheck="false" placeholder="название модели" />
      <datalist id="llm-models">
        {#each models as model (model)}<option value={model}></option>{/each}
      </datalist>
    </label>
    {#if p.kind === 'openai'}
      <button type="button" class="secondary" onclick={loadModels}>Загрузить список моделей</button>
    {/if}
    {#if p.key !== 'none'}
      <label>
        Ключ API{p.key === 'optional' ? ' (если нужен)' : ''}
        <input type="password" bind:value={ps.apiKey} autocomplete="off" spellcheck="false" />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={ps.rememberKey} />
        {isDesktop ? 'Запомнить ключ в системном хранилище' : 'Запомнить ключ в этом браузере'}
      </label>
      <p class="note">
        Ключ {isDesktop ? 'хранится в системном хранилище ключей' : 'хранится только в этом браузере'}
        и отправляется только {destination}.
        {#if p.keyUrl}<a href={p.keyUrl} target="_blank" rel="noopener noreferrer">Получить ключ</a>.{/if}
      </p>
    {/if}
  </details>

  <p class="privacy" class:local={p.local}>
    {#if p.local}
      Текст останется на этом компьютере: модель работает локально.
    {:else}
      Текст ({text.length.toLocaleString('ru-RU')} знаков) будет отправлен: {destination}.
    {/if}
  </p>
  <div class="toolbar">
    <button type="button" onclick={run} disabled={!text.trim() || running !== null}>
      {running ? 'Оцениваю…' : 'Оценить'}
    </button>
    {#if running}
      <button type="button" class="secondary" onclick={() => running?.abort()}>Отменить</button>
    {/if}
  </div>
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
