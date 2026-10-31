<script lang="ts">
  import { untrack } from 'svelte';
  import { Analyzer } from '../lib/analyzer';
  // В имени файла сборки — хеш содержимого: новый словарь не застрянет в кеше.
  import dictionaryAsset from '../wasm/clarity-pkg/ru.dict.gz?url';
  import { clarityEnabled, setClarityEnabled } from '../lib/clarity';
  import { ACCEPT, ImportError, pageToText, readFileText } from '../lib/import';
  import { fetchPage, isDesktop } from '../lib/platform';
  import { SAMPLES } from '../lib/samples';
  import type { Analysis, ClarityReport } from '../lib/types';
  import ClarityPanel from './ClarityPanel.svelte';
  import Highlighted from './Highlighted.svelte';
  import LlmPanel from './LlmPanel.svelte';
  import Results from './Results.svelte';

  const analyzer = new Analyzer();

  let text = $state('');
  // Разметка ссылается на смещения в том тексте, по которому она посчитана.
  // Результаты только заменяются целиком: глубокий $state над разметкой длинного текста
  // стоил бы памяти и времени.
  let analysis = $state.raw<{ text: string; result: Analysis } | null>(null);
  let busy = $state(false);
  // Оценка формулами — для сравнения с оценкой LLM.
  let formula = $derived(analysis?.result.measurement.indexes?.average.value ?? null);
  let error = $state('');
  let analyzeError = $state('');
  let dragging = $state(false);
  // Открытие по ссылке есть только в настольном приложении: браузеру мешает CORS.
  let urlOpen = $state(false);
  let url = $state('');
  let loadingUrl = $state(false);
  let source = $state<{ url: string; text: string } | null>(null);
  let fileInput: HTMLInputElement;

  // Проверки ясного языка: словарь загружается при включении и затем лежит в кеше.
  const dictionaryUrl = new URL(dictionaryAsset, document.baseURI).href;
  let clarityState = $state<'off' | 'loading' | 'ready' | 'error'>(clarityEnabled() ? 'loading' : 'off');
  let clarity = $state.raw<{ text: string; report: ClarityReport } | null>(null);
  let clarityError = $state('');

  // Функция, а не сравнение на месте: за время await проверку могут выключить.
  const clarityOn = () => clarityState === 'loading' || clarityState === 'ready';

  async function runClarity(current: string) {
    if (!clarityOn()) return;
    try {
      const report = await analyzer.clarity(current, dictionaryUrl);
      if (report && clarityOn()) {
        clarity = { text: current, report };
        clarityState = 'ready';
      }
    } catch (e) {
      clarityState = 'error';
      clarityError = (e as Error).message;
      setClarityEnabled(false);
    }
  }

  function enableClarity() {
    setClarityEnabled(true);
    clarityState = 'loading';
    clarityError = '';
    runClarity(text);
  }

  function disableClarity() {
    setClarityEnabled(false);
    clarityState = 'off';
    clarity = null;
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const current = text;
    clearTimeout(timer);
    if (!current.trim()) {
      analyzer.cancel();
      analysis = null;
      analyzeError = '';
      busy = false;
      // Пустой текст тоже проверяется: так при включённой проверке загрузится словарь,
      // а панель покажет, что ждёт текста.
      untrack(() => runClarity(current));
      return;
    }
    busy = true;
    timer = setTimeout(async () => {
      try {
        const result = await analyzer.analyze(current);
        if (result) {
          analysis = { text: current, result };
          analyzeError = '';
          busy = false;
          runClarity(current);
        }
      } catch (e) {
        // Если текст за это время сменился, расчёт для нового уже запущен.
        if (text !== current) return;
        analysis = null;
        busy = false;
        analyzeError = `Не удалось оценить текст: ${(e as Error).message}`;
      }
    }, 200);
  });

  async function openFile(file: File | undefined) {
    if (!file) return;
    error = '';
    try {
      text = await readFileText(file);
    } catch (e) {
      error = e instanceof ImportError ? e.message : `Не удалось прочитать «${file.name}»: ${(e as Error).message}`;
    }
  }

  async function openUrl() {
    if (!url.trim() || loadingUrl) return;
    error = '';
    loadingUrl = true;
    try {
      const page = await fetchPage(url.trim());
      text = await pageToText(page);
      source = { url: page.url, text };
      urlOpen = false;
    } catch (e) {
      // Команды Rust возвращают ошибку строкой с готовым сообщением.
      error = e instanceof ImportError ? e.message : `Не удалось загрузить страницу: ${e instanceof Error ? e.message : e}`;
    } finally {
      loadingUrl = false;
    }
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    openFile(event.dataTransfer?.files[0]);
  }
</script>

<h1 class="visually-hidden">Оценка читаемости текста</h1>
<div class="layout">
  <section
    class="input"
    class:dragging
    aria-label="Текст для оценки"
    ondragover={(e) => {
      e.preventDefault();
      dragging = true;
    }}
    ondragleave={() => (dragging = false)}
    ondrop={onDrop}
  >
    <div class="toolbar">
      <button type="button" onclick={() => fileInput.click()}>Открыть файл</button>
      {#if isDesktop}
        <button type="button" class="secondary" onclick={() => (urlOpen = !urlOpen)}>Открыть по ссылке</button>
      {/if}
      <input
        bind:this={fileInput}
        type="file"
        accept={ACCEPT}
        hidden
        onchange={(e) => {
          const input = e.currentTarget;
          openFile(input.files?.[0]);
          input.value = '';
        }}
      />
      {#each SAMPLES as sample (sample.title)}
        <button type="button" class="secondary" onclick={() => (text = sample.text)}>{sample.title}</button>
      {/each}
      {#if text}
        <button type="button" class="secondary" onclick={() => (text = '')}>Очистить</button>
      {/if}
    </div>
    {#if urlOpen}
      <div class="url-form">
        <input
          type="url"
          bind:value={url}
          placeholder="https://…"
          spellcheck="false"
          aria-label="Адрес страницы"
          onkeydown={(e) => e.key === 'Enter' && openUrl()}
        />
        <button type="button" onclick={openUrl} disabled={loadingUrl || !url.trim()}>
          {loadingUrl ? 'Загружаю…' : 'Загрузить'}
        </button>
      </div>
    {/if}
    <textarea
      bind:value={text}
      placeholder="Вставьте текст или перетащите сюда файл: txt, html, docx, pdf"
      spellcheck="false"
    ></textarea>
    <div class="status">
      {#if error || analyzeError}
        <span class="error" role="alert">{error || analyzeError}</span>
      {:else if busy}
        <span>Считаю…</span>
      {:else if text}
        <span>{text.length.toLocaleString('ru-RU')} знаков</span>
        {#if source?.text === text}<span> · {source.url}</span>{/if}
      {/if}
    </div>
  </section>

  {#if analysis}
    <section class="highlight-area" aria-label="Подсветка">
      <Highlighted
        text={analysis.text}
        annotation={analysis.result.annotation}
        clarity={clarity?.text === analysis.text ? clarity.report : null}
      />
    </section>
  {/if}

  <aside class="results" aria-live="polite">
    {#if analysis}
      <Results measurement={analysis.result.measurement} />
    {:else}
      <p class="empty">Здесь появится оценка: уровень образования, нужный для понимания текста, и подробные показатели.</p>
    {/if}
    <ClarityPanel
      state={clarityState}
      report={clarity?.text === text ? clarity.report : null}
      error={clarityError}
      onenable={enableClarity}
      ondisable={disableClarity}
    />
    <LlmPanel {text} {formula} />
    <p class="note"><a href="#about">Как считается оценка</a></p>
  </aside>
</div>
