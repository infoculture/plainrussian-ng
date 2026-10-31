<script lang="ts">
  import { CLARITY_KINDS } from '../lib/clarity';
  import type { ClarityReport } from '../lib/types';

  let {
    state,
    report,
    error,
    onenable,
    ondisable,
  }: {
    state: 'off' | 'loading' | 'ready' | 'error';
    report: ClarityReport | null;
    error: string;
    onenable: () => void;
    ondisable: () => void;
  } = $props();

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
              <div class="clarity-head">
                <span class="clarity-{k.kind} swatch" aria-hidden="true"></span>
                <span>{k.label}</span>
                <span class="num">{report.counts[k.kind]}</span>
              </div>
              <p class="note">{examples(k.kind).map((e) => `«${e}»`).join(', ')}</p>
              <p class="note advice">{k.advice}</p>
            </li>
          {/if}
        {/each}
      </ul>
      <p class="note">Находки можно подсветить в тексте: переключатель «Ясный язык» над подсветкой.</p>
    {/if}
    <button type="button" class="link" onclick={ondisable}>Выключить проверку</button>
  {/if}
</section>
