<script lang="ts">
  // Логотип: строки текста, последняя переходит в галочку — текст проверен на простоту.
  // Тот же рисунок в public/icon.svg и в иконках приложения.
  let { size = 40 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true" focusable="false">
  <rect width="512" height="512" rx="112" fill="#1f5f8b" />
  <g fill="none" stroke-width="44" stroke-linecap="round" stroke-linejoin="round">
    <path d="M112 120H400M112 200H344M112 280H280" stroke="#fff" />
    <path d="M112 360H216L264 408L400 272" stroke="#4cc47f" />
  </g>
</svg>
