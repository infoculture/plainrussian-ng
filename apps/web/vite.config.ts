import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { VitePWA } from 'vite-plugin-pwa';

// Страница ничего не загружает со сторонних адресов. Текст уходит наружу только
// при оценке LLM и только к провайдерам из src/lib/llm/providers.ts (или на localhost).
// 'wasm-unsafe-eval' нужен только для компиляции WebAssembly.
const CSP = [
  "default-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self'",
  "img-src 'self' data:",
  "connect-src 'self' https://api.anthropic.com https://api.openai.com https://openrouter.ai " +
    'http://localhost:* http://127.0.0.1:*',
  "worker-src 'self'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join('; ');

/** CSP только в сборке: dev-сервер Vite вставляет встроенные скрипты и стили. */
function csp(): Plugin {
  return {
    name: 'plainrussian-csp',
    apply: 'build',
    transformIndexHtml: () => [
      {
        tag: 'meta',
        attrs: { 'http-equiv': 'Content-Security-Policy', content: CSP },
        injectTo: 'head-prepend',
      },
    ],
  };
}

// Режим desktop (`vite --mode desktop`) — сборка для Tauri: без service worker, который
// не работает на протоколе tauri://, и без meta-CSP — её задаёт tauri.conf.json.
export default defineConfig(({ mode }) => {
  const desktop = mode === 'desktop';
  return {
    // Относительные пути: сборку можно положить в любой каталог статического хостинга.
    base: './',
    build: { outDir: desktop ? 'dist-desktop' : 'dist' },
    plugins: [svelte(), ...(desktop ? [] : [csp(), pwa()])],
    worker: { format: 'es' as const },
  };
});

function pwa() {
  return VitePWA({
    registerType: 'autoUpdate',
    injectRegister: 'script',
    includeAssets: ['icon.svg'],
    manifest: {
      name: 'Простой русский — оценка читаемости текста',
      short_name: 'Простой русский',
      description: 'Оценка простоты русскоязычных текстов прямо на вашем устройстве',
      lang: 'ru',
      start_url: '.',
      scope: '.',
      display: 'standalone',
      background_color: '#ffffff',
      theme_color: '#1f5f8b',
      icons: [
        { src: 'icon-192.png', sizes: '192x192', type: 'image/png' },
        { src: 'icon-512.png', sizes: '512x512', type: 'image/png' },
        { src: 'icon-512-maskable.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
      ],
    },
    workbox: {
      // WASM и разборщики docx/pdf тоже кешируются, иначе офлайн не откроются файлы.
      globPatterns: ['**/*.{html,js,mjs,css,wasm,svg,png}'],
      maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
      // При injectRegister: 'script' плагин сам их не включает: без них новая
      // версия ждёт, пока закроются все вкладки.
      skipWaiting: true,
      clientsClaim: true,
    },
  });
}
