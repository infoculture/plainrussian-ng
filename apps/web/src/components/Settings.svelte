<script lang="ts">
  import { LlmError, PROVIDERS, listModels, provider } from '../lib/llm/providers.ts';
  import { toRequest } from '../lib/llm/settings.ts';
  import { isConfigured, llm } from '../lib/llm/store.svelte.ts';
  import { isDesktop } from '../lib/platform.ts';

  let p = $derived(provider(llm.provider));
  let ps = $derived(llm.providers[llm.provider]);
  let models = $state<string[]>([]);
  let error = $state('');
  let loading = $state(false);
  let configured = $derived(isConfigured());

  $effect(() => {
    // Подсказки моделей сбрасываются при смене провайдера.
    models = provider(llm.provider).models;
  });

  let destination = $derived.by(() => {
    if (p.id !== 'custom') return p.title;
    try {
      return new URL(ps.baseUrl).host;
    } catch {
      return 'указанный сервис';
    }
  });

  async function loadModels() {
    error = '';
    loading = true;
    try {
      models = await listModels(toRequest(llm));
      if (!ps.model && models.length) ps.model = models[0];
    } catch (e) {
      error = e instanceof LlmError ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

<section class="page" aria-labelledby="settings-title">
  <h1 id="settings-title">Настройки</h1>

  <h2>Оценка с помощью LLM</h2>
  <p>
    Языковая модель оценивает текст целиком и объясняет, что мешает его понять. Это дополнение к
    формулам: нужен свой ключ API у одного из провайдеров или модель, запущенная на этом компьютере.
  </p>

  <div class="form">
    <label>
      Провайдер
      <select bind:value={llm.provider}>
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
      <span class="row">
        <input bind:value={ps.model} list="llm-models" spellcheck="false" placeholder="название модели" />
        {#if p.kind === 'openai'}
          <button type="button" class="secondary" onclick={loadModels} disabled={loading}>
            {loading ? 'Загружаю…' : 'Список моделей'}
          </button>
        {/if}
      </span>
      <datalist id="llm-models">
        {#each models as model (model)}<option value={model}></option>{/each}
      </datalist>
    </label>

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

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <p class="privacy" class:local={p.local}>
      {#if p.local}
        Текст останется на этом компьютере: модель работает локально.
      {:else}
        При оценке текст будет отправлен: {destination}.
      {/if}
    </p>
    <p class="note">
      {configured ? 'Готово: на главной появится кнопка «Оценить».' : 'Заполните поля, чтобы включить оценку.'}
      Настройки сохраняются сразу.
    </p>
  </div>

  <p><a href="#home" class="back">← Вернуться к тексту</a></p>
</section>
