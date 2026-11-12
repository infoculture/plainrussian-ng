<script lang="ts">
  import { facts } from '../lib/compare';
  import type { Measurement } from '../lib/types';
  import Delta from './Delta.svelte';

  let {
    was,
    now,
    edited,
    onremember,
  }: {
    /** Исходный вариант; null, если его ещё не запомнили. */
    was: Measurement | null;
    now: Measurement;
    /** Текст правили после того, как запомнили исходный вариант. */
    edited: boolean;
    /** Запомнить текущий вариант как исходный. */
    onremember: () => void;
  } = $props();
</script>

<h2>Было и стало</h2>
{#if !was || !facts(was)}
  <p class="note">Запомните этот вариант — после правки здесь появится сравнение с ним.</p>
  <button type="button" class="link" onclick={onremember}>Запомнить вариант</button>
{:else if !edited || !facts(now)}
  <p class="note">Исходный вариант запомнен. Правьте текст — здесь появится сравнение с ним.</p>
{:else}
  <Delta {was} {now} />
  <button type="button" class="link" onclick={onremember}>Считать этот вариант исходным</button>
{/if}
