<script lang="ts">
  import { Analyzer } from './lib/analyzer';
  import { ACCEPT, ImportError, readFileText } from './lib/import';
  import { SAMPLES } from './lib/samples';
  import type { Algorithm, Result } from './lib/types';
  import Highlighted from './components/Highlighted.svelte';
  import LegacyResults from './components/LegacyResults.svelte';
  import Results from './components/Results.svelte';

  const analyzer = new Analyzer();
  const ALGORITHM_KEY = 'plainrussian.algorithm';

  // Выбор алгоритма — удобство одного зрителя, поэтому он хранится в браузере.
  function savedAlgorithm(): Algorithm {
    try {
      return localStorage.getItem(ALGORITHM_KEY) === 'legacy' ? 'legacy' : 'current';
    } catch {
      return 'current';
    }
  }

  let text = $state('');
  let algorithm = $state<Algorithm>(savedAlgorithm());
  // Разметка ссылается на смещения в том тексте, по которому она посчитана.
  let analysis = $state<{ text: string; result: Result } | null>(null);
  let busy = $state(false);
  let error = $state('');
  let dragging = $state(false);
  let fileInput: HTMLInputElement;

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const current = text;
    const chosen = algorithm;
    clearTimeout(timer);
    if (!current.trim()) {
      analyzer.cancel();
      analysis = null;
      busy = false;
      return;
    }
    busy = true;
    timer = setTimeout(async () => {
      const result = await analyzer.analyze(current, chosen);
      if (result) {
        analysis = { text: current, result };
        busy = false;
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

  function choose(value: Algorithm) {
    algorithm = value;
    try {
      localStorage.setItem(ALGORITHM_KEY, value);
    } catch {
      // Без хранилища выбор просто не запомнится.
    }
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    openFile(event.dataTransfer?.files[0]);
  }
</script>

<header class="site">
  <h1>Простой русский</h1>
  <p>Оценка читаемости текста. Текст обрабатывается на вашем устройстве и никуда не отправляется.</p>
</header>

<main class="layout">
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
    <textarea
      bind:value={text}
      placeholder="Вставьте текст или перетащите сюда файл: txt, html, docx, pdf"
      spellcheck="false"
    ></textarea>
    <div class="status">
      {#if error}
        <span class="error" role="alert">{error}</span>
      {:else if busy}
        <span>Считаю…</span>
      {:else if text}
        <span>{text.length.toLocaleString('ru-RU')} знаков</span>
      {/if}
    </div>
  </section>

  {#if analysis}
    <section class="highlight-area" aria-label="Подсветка">
      <Highlighted text={analysis.text} annotation={analysis.result.analysis.annotation} />
    </section>
  {/if}

  <aside class="results" aria-live="polite">
    <div class="switch" role="radiogroup" aria-label="Алгоритм">
      <button type="button" role="radio" aria-checked={algorithm === 'current'} onclick={() => choose('current')}>
        Новый алгоритм
      </button>
      <button type="button" role="radio" aria-checked={algorithm === 'legacy'} onclick={() => choose('legacy')}>
        Как на старом сайте
      </button>
    </div>
    {#if analysis?.result.algorithm === 'current'}
      <Results measurement={analysis.result.analysis.measurement} />
    {:else if analysis?.result.algorithm === 'legacy'}
      <LegacyResults measurement={analysis.result.analysis.measurement} />
    {:else}
      <p class="empty">Здесь появится оценка: уровень образования, нужный для понимания текста, и подробные показатели.</p>
    {/if}
    <details class="method">
      <summary>Как считается оценка</summary>
      <p>
        Пять формул читаемости, адаптированных для русского языка: Flesch-Kincaid, Coleman-Liau,
        SMOG, ARI и упрощённая Dale-Chale. Каждая даёт число лет обучения, нужное для понимания
        текста. Главная оценка — их среднее.
      </p>
      <p>
        Предложения делятся по правилам библиотеки razdel: сокращения вроде «т. е.», «ст.» и
        инициалы не обрывают предложение. Слово — буквы, внутри допустимы дефис и апостроф;
        числа словами не считаются. Слог — гласная буква; аббревиатура без гласных («ВВП»)
        читается по буквам. Сложное слово — больше четырёх слогов.
      </p>
      <p>
        Коэффициенты формул подобраны на текстах с экспертной оценкой уровня. Текстов немного,
        поэтому оценка ориентировочная.
      </p>
      <p>
        Режим «Как на старом сайте» повторяет старый plainrussian.ru, чтобы можно было сравнить
        с прежними оценками. У него известные ошибки: заглавные буквы не учитываются, а «т.е.»
        и «3.5» делят предложение на части.
      </p>
    </details>
  </aside>
</main>

<footer class="site">
  <p>Проект <a href="https://infoculture.ru">Информационной культуры</a>. Работает без интернета после первой загрузки.</p>
</footer>
