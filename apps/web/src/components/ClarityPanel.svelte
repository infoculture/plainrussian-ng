<script lang="ts">
  import { CLARITY_KINDS } from '../lib/clarity';
  import type { ClarityKind, ClarityReport } from '../lib/types';

  let {
    state,
    report,
    error,
    onenable,
    ondisable,
    onselect,
  }: {
    state: 'off' | 'loading' | 'ready' | 'error';
    report: ClarityReport | null;
    error: string;
    onenable: () => void;
    ondisable: () => void;
    /** Находку с этими смещениями нужно показать в поле ввода. */
    onselect: (start: number, end: number) => void;
  } = $props();

  // Какая находка каждого вида показана последней. С новым отчётом счёт начинается
  // заново: после правки смещения и сами находки другие.
  let seen: ClarityReport | null = null;
  let shown: Partial<Record<ClarityKind, number>> = {};

  /** Каждое нажатие ведёт к следующей находке этого вида, после последней — к первой. */
  function next(kind: ClarityKind) {
    if (!report) return;
    if (seen !== report) {
      seen = report;
      shown = {};
    }
    const found = report.findings.filter((f) => f.kind === kind);
    if (!found.length) return;
    const index = ((shown[kind] ?? -1) + 1) % found.length;
    shown[kind] = index;
    onselect(found[index].start, found[index].end);
  }

  /** Неповторяющиеся примеры в порядке появления. */
  function examples(kind: string, limit = 4): string[] {
    const seen = new Set<string>();
    for (const f of report?.findings ?? []) {
      if (f.kind === kind) seen.add(f.text);
      if (seen.size >= limit) break;
    }
    return [...seen];
  }

  let total = $derived(report ? report.findings.length : 0);
</script>

<section class="clarity" aria-labelledby="clarity-title">
  <h2 id="clarity-title">Ясный язык</h2>
  {#if state === 'off' || state === 'error'}
    <p class="note">
      Найдёт приёмы, которые утяжеляют текст: цепочки родительных падежей, отглагольные
      существительные, канцеляризмы, страдательный залог, аббревиатуры. Для этого нужен словарь
      русского языка, около 7 МБ: он загрузится один раз и дальше будет работать без интернета.
    </p>
    {#if error}<p class="error" role="alert">Не получилось: {error}</p>{/if}
    <button type="button" class="secondary" onclick={onenable}>Включить проверку</button>
  {:else if !report}
    <p class="note">{state === 'loading' ? 'Загружаю словарь…' : 'Проверяю…'}</p>
  {:else if report}
    {#if report.words === 0}
      <p class="note">Вставьте текст — проверка начнётся сама.</p>
    {:else if total === 0}
      <p>Ничего не нашлось: текст написан просто.</p>
    {:else}
      <ul class="clarity-list">
        {#each CLARITY_KINDS as k (k.kind)}
          {#if report.counts[k.kind] > 0}
            <li>
              <button
                type="button"
                class="clarity-head"
                title="Перейти к следующей находке в тексте"
                onclick={() => next(k.kind)}
              >
                <span class="clarity-{k.kind} swatch" aria-hidden="true"></span>
                <span>{k.label}</span>
                <span class="num">{report.counts[k.kind]}</span>
              </button>
              <p class="note">{examples(k.kind).map((e) => `«${e}»`).join(', ')}</p>
              <p class="note advice">{k.advice}</p>
            </li>
          {/if}
        {/each}
      </ul>
      <p class="note">
        Нажмите на вид находок — следующая выделится в поле ввода. Все находки сразу подсвечивает
        переключатель «Ясный язык» над подсветкой.
      </p>
    {/if}
    <button type="button" class="link" onclick={ondisable}>Выключить проверку</button>
  {/if}
</section>
