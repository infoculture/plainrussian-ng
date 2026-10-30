<script lang="ts">
  import { Analyzer } from './lib/analyzer';
  import { ACCEPT, ImportError, readFileText } from './lib/import';
  import { SAMPLES } from './lib/samples';
  import type { Analysis } from './lib/types';
  import Highlighted from './components/Highlighted.svelte';
  import Results from './components/Results.svelte';

  const analyzer = new Analyzer();

  let text = $state('');
  // Разметка ссылается на смещения в том тексте, по которому она посчитана.
  let analysis = $state<{ text: string; result: Analysis } | null>(null);
  let busy = $state(false);
  let error = $state('');
  let dragging = $state(false);
  let fileInput: HTMLInputElement;

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const current = text;
    clearTimeout(timer);
    if (!current.trim()) {
      analyzer.cancel();
      analysis = null;
      busy = false;
      return;
    }
    busy = true;
    timer = setTimeout(async () => {
      const result = await analyzer.analyze(current);
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
      <Highlighted text={analysis.text} annotation={analysis.result.annotation} />
    </section>
  {/if}

  <aside class="results" aria-live="polite">
    {#if analysis}
      <Results measurement={analysis.result.measurement} />
    {:else}
      <p class="empty">Здесь появится оценка: уровень образования, нужный для понимания текста, и подробные показатели.</p>
    {/if}
    <details class="method">
      <summary>Как считается оценка</summary>
      <p>
        Используются пять формул читаемости, адаптированных для русского языка: Flesch-Kincaid,
        Coleman-Liau, SMOG, ARI и упрощённая Dale-Chale. Результат — число лет обучения,
        нужное для понимания текста. Главная оценка — по SMOG.
      </p>
      <p>
        Слог — гласная буква. Сложное слово — больше четырёх слогов. Конец предложения — точка,
        вопросительный или восклицательный знак.
      </p>
      <p>
        Алгоритм повторяет старый plainrussian.ru, чтобы оценки совпадали с прежними. Поэтому у него
        те же ограничения: заглавные буквы не учитываются, а сокращения вроде «т.е.» и числа вроде
        «3.5» делят предложение на части. Исправленный алгоритм появится в следующих версиях.
      </p>
    </details>
  </aside>
</main>

<footer class="site">
  <p>Проект <a href="https://infoculture.ru">Информационной культуры</a>. Работает без интернета после первой загрузки.</p>
</footer>
