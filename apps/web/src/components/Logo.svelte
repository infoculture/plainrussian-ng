<script lang="ts">
  // Логотип: буква «Я» — русский текст, галочка — проверен на простоту.
  // Тот же рисунок в public/icon.svg и в иконках приложения.
  let { size = 40 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true" focusable="false">
  <rect width="512" height="512" rx="112" fill="#1f5f8b" />
  <path
    d="M300 112V384M300 112H214a74 74 0 0 0 0 148H300M236 260 148 384"
    fill="none"
    stroke="#fff"
    stroke-width="56"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <circle cx="384" cy="384" r="96" fill="#2e9d5b" stroke="#1f5f8b" stroke-width="20" />
  <path
    d="M340 386 372 418 430 354"
    fill="none"
    stroke="#fff"
    stroke-width="28"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
