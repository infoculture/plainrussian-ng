// Поле ввода: переход от находки к месту в тексте.

/**
 * Выделяет фрагмент в поле ввода и прокручивает поле к нему, чтобы его можно было
 * сразу править. Смещения — в единицах UTF-16, как у разметки ядра.
 */
export function selectRange(area: HTMLTextAreaElement, start: number, end: number): void {
  area.focus();
  area.setSelectionRange(start, end);
  // setSelectionRange поле не прокручивает: строку с началом фрагмента ставим сами,
  // на треть высоты поля от верха.
  area.scrollTop = Math.max(0, lineTop(area, start) - area.clientHeight / 3);
}

/**
 * Расстояние от верха содержимого поля до строки с этим смещением, в пикселях.
 * Измеряется по невидимой копии поля с тем же шрифтом и шириной: её высота с текстом
 * до смещения — низ искомой строки. Так результат не зависит от браузера.
 */
function lineTop(area: HTMLTextAreaElement, offset: number): number {
  const style = getComputedStyle(area);
  const mirror = document.createElement('div');
  // Свойства задаются через CSSOM: атрибут style запретила бы CSP сайта.
  const s = mirror.style;
  s.position = 'absolute';
  s.visibility = 'hidden';
  s.top = '0';
  s.left = '-9999px';
  s.boxSizing = 'border-box';
  // clientWidth — без полосы прокрутки: текст в копии переносится там же, где в поле.
  s.width = `${area.clientWidth}px`;
  s.whiteSpace = 'pre-wrap';
  s.overflowWrap = 'break-word';
  s.fontFamily = style.fontFamily;
  s.fontSize = style.fontSize;
  s.fontWeight = style.fontWeight;
  s.fontStyle = style.fontStyle;
  s.lineHeight = style.lineHeight;
  s.letterSpacing = style.letterSpacing;
  s.wordSpacing = style.wordSpacing;
  s.tabSize = style.tabSize;
  s.paddingTop = style.paddingTop;
  s.paddingRight = style.paddingRight;
  s.paddingBottom = style.paddingBottom;
  s.paddingLeft = style.paddingLeft;
  // Перевод строки в конце не даёт высоты, пока за ним нет знака.
  mirror.textContent = `${area.value.slice(0, offset)}​`;
  document.body.append(mirror);
  const lineHeight = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.2;
  const top = mirror.offsetHeight - parseFloat(style.paddingBottom) - lineHeight;
  mirror.remove();
  return top;
}
