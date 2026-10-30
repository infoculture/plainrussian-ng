<script lang="ts">
  import { onMount } from 'svelte';
  import { persistLlmSettings } from './lib/llm/store.svelte.ts';
  import { REPO_URL } from './lib/project';
  import About from './components/About.svelte';
  import Home from './components/Home.svelte';
  import Logo from './components/Logo.svelte';
  import Settings from './components/Settings.svelte';

  // Страницы переключаются по хешу: так работает и статический хостинг, и Tauri.
  let route = $state(location.hash);
  let page = $derived(route === '#settings' ? 'settings' : route === '#about' ? 'about' : 'home');

  onMount(() => {
    persistLlmSettings();
    const onHash = () => {
      route = location.hash;
      window.scrollTo(0, 0);
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  });
</script>

<header class="site">
  <a href="#home" class="brand" aria-label="Простой русский — на главную">
    <Logo size={44} />
    <span>
      <span class="title">Простой русский</span>
      <span class="tagline">Оценка читаемости текста. Текст обрабатывается на вашем устройстве.</span>
    </span>
  </a>
  <a href="#settings" class="button secondary nav" aria-current={page === 'settings' ? 'page' : undefined}>
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M4 6h9M19 6h1M4 12h3M13 12h7M4 18h11M21 18h-1" />
      <circle cx="16" cy="6" r="2.5" />
      <circle cx="10" cy="12" r="2.5" />
      <circle cx="18" cy="18" r="2.5" />
    </svg>
    Настройки
  </a>
</header>

<main>
  <!-- Главная не размонтируется: текст и оценки сохраняются при переходе в настройки. -->
  <div hidden={page !== 'home'}><Home /></div>
  {#if page === 'settings'}
    <Settings />
  {:else if page === 'about'}
    <About />
  {/if}
</main>

<footer class="site">
  <nav aria-label="О сервисе">
    <a href="#about">О проекте</a>
    <a href={REPO_URL} target="_blank" rel="noopener noreferrer">Исходный код на GitHub</a>
    <a href="https://infoculture.ru">Информационная культура</a>
  </nav>
  <p>Работает без интернета после первой загрузки.</p>
</footer>
