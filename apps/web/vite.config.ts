import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { VitePWA } from 'vite-plugin-pwa';

/** Счётчик Яндекс Метрики. Он есть только на сайте, в настольном приложении его нет. */
const METRIKA_ID = 24037120;

// Адреса Метрики из https://yandex.ru/support/metrica/ru/code/install-counter-csp,
// кроме адресов Вебвизора: он не включён.
const METRIKA_ORIGINS = ['ru', 'az', 'by', 'co.il', 'com', 'com.am', 'com.ge', 'com.tr', 'ee',
  'fr', 'kg', 'kz', 'lt', 'lv', 'md', 'tj', 'tm', 'uz']
  .map((zone) => `https://mc.yandex.${zone}`)
  .join(' ');

// Со сторонних адресов страница загружает только счётчик Метрики. Текст уходит наружу
// только при оценке LLM и только к провайдерам из src/lib/llm/providers.ts (или на localhost).
// 'wasm-unsafe-eval' нужен только для компиляции WebAssembly.
const CSP = [
  "default-src 'self'",
  `script-src 'self' 'wasm-unsafe-eval' ${METRIKA_ORIGINS} https://yastatic.net`,
  "style-src 'self'",
  `img-src 'self' data: ${METRIKA_ORIGINS} https://informer.yandex.ru`,
  "connect-src 'self' https://api.anthropic.com https://api.openai.com https://openrouter.ai " +
    `http://localhost:* http://127.0.0.1:* ${METRIKA_ORIGINS}`,
  `frame-src ${METRIKA_ORIGINS}`,
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

/**
 * Счётчик Метрики отдельным файлом: встроенный скрипт запретила бы CSP. Его код —
 * из кабинета Метрики, плюс trackHash: страницы сайта переключаются по хешу.
 */
function metrika(): Plugin {
  const script = `(function(m,e,t,r,i,k,a){
    m[i]=m[i]||function(){(m[i].a=m[i].a||[]).push(arguments)};
    m[i].l=1*new Date();
    for (var j = 0; j < document.scripts.length; j++) {if (document.scripts[j].src === r) { return; }}
    k=e.createElement(t),a=e.getElementsByTagName(t)[0],k.async=1,k.src=r,a.parentNode.insertBefore(k,a)
})(window, document,'script','https://mc.yandex.ru/metrika/tag.js', 'ym');

ym(${METRIKA_ID}, 'init', {clickmap:true, referrer: document.referrer, url: location.href, accurateTrackBounce:true, trackLinks:true, trackHash:true});
`;
  return {
    name: 'plainrussian-metrika',
    apply: 'build',
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: 'metrika.js', source: script });
    },
    transformIndexHtml: {
      // После обработки HTML самим Vite: иначе он попробует собрать metrika.js как модуль.
      order: 'post',
      handler: () => [
        { tag: 'script', attrs: { src: './metrika.js', async: true }, injectTo: 'head' },
        {
          tag: 'noscript',
          children: `<div><img src="https://mc.yandex.ru/watch/${METRIKA_ID}" alt="" /></div>`,
          injectTo: 'body',
        },
      ],
    },
  };
}

// Режим desktop (`vite --mode desktop`) — сборка для Tauri: без service worker, который
// не работает на протоколе tauri://, без meta-CSP — её задаёт tauri.conf.json, и без
// счётчика Метрики. На dev-сервере счётчика тоже нет, чтобы не засорять статистику.
export default defineConfig(({ command, mode }) => {
  const desktop = mode === 'desktop';
  const withMetrika = command === 'build' && !desktop;
  return {
    // Относительные пути: сборку можно положить в любой каталог статического хостинга.
    base: './',
    build: { outDir: desktop ? 'dist-desktop' : 'dist' },
    // Информер Метрики в футере (App.svelte) показывается там же, где работает счётчик.
    define: { 'import.meta.env.METRIKA_ID': JSON.stringify(withMetrika ? METRIKA_ID : 0) },
    plugins: [svelte(), ...(desktop ? [] : [csp(), metrika(), pwa()])],
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
      // Словарь для проверок ясного языка (7 МБ, .gz) не входит в предзагрузку:
      // он скачивается, когда проверку включают, и кешируется в src/lib/worker.ts.
    },
  });
}
