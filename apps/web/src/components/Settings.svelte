<script lang="ts">
  import { LlmError, PROVIDERS, destination, listModels, provider, runsLocally } from '../lib/llm/providers.ts';
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

  let target = $derived(destination(p, ps.baseUrl));
  let local = $derived(runsLocally(p, ps.baseUrl));

  async function loadModels() {
    error = '';
    loading = true;
    // Пока список грузится, провайдера могут сменить: ответ относится к прежнему.
    const id = llm.provider;
    const settings = ps;
    try {
      const list = await listModels(toRequest(llm));
      if (llm.provider !== id) return;
      models = list;
      if (!settings.model && list.length) settings.model = list[0];
    } catch (e) {
      if (llm.provider === id) error = e instanceof LlmError ? e.message : String(e);
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

    {#if p.scopes}
      <label>
        Тип доступа
        <select bind:value={ps.scope}>
          {#each p.scopes as scope (scope.id)}
            <option value={scope.id}>{scope.title}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if p.key !== 'none'}
      <label>
        {p.id === 'gigachat' ? 'Ключ авторизации' : 'Ключ API'}{p.key === 'optional' ? ' (если нужен)' : ''}
        <input type="password" bind:value={ps.apiKey} autocomplete="off" spellcheck="false" />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={ps.rememberKey} />
        {isDesktop ? 'Запомнить ключ в системном хранилище' : 'Запомнить ключ в этом браузере'}
      </label>
      <p class="note">
        Ключ {isDesktop ? 'хранится в системном хранилище ключей' : 'хранится только в этом браузере'}
        и отправляется только {target}.
        {#if p.keyUrl}<a href={p.keyUrl} target="_blank" rel="noopener noreferrer">Получить ключ</a>.{/if}
      </p>
    {/if}

    {#if p.id === 'gigachat'}
      <p class="note">
        Ключ авторизации выдаётся в личном кабинете GigaChat API. Для соединения в системе должен
        быть установлен сертификат Минцифры «Russian Trusted Root CA»:
        <a href="https://developers.sber.ru/docs/ru/gigachat/certificates" target="_blank" rel="noopener noreferrer"
          >как установить</a
        >.
      </p>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <p class="privacy" class:local>
      {#if local}
        Текст останется на этом компьютере: модель работает локально.
      {:else}
        При оценке текст будет отправлен: {target}.
      {/if}
    </p>
    <p class="note">
      {configured ? 'Готово: на главной появится кнопка «Оценить».' : 'Заполните поля, чтобы включить оценку.'}
      Настройки сохраняются сразу.
    </p>
  </div>

  <p><a href="#home" class="back">← Вернуться к тексту</a></p>
</section>
