// Извлечение текста из файлов. Всё происходит в браузере; разборщики docx и pdf
// загружаются только при первом таком файле.

import type { Page } from './platform.ts';

export const ACCEPT = '.txt,.md,.html,.htm,.docx,.pdf';

export class ImportError extends Error {}

export async function readFileText(file: File): Promise<string> {
  const ext = file.name.toLowerCase().split('.').pop() ?? '';
  switch (ext) {
    case 'txt':
    case 'md':
      return decode(new Uint8Array(await file.arrayBuffer()), false);
    case 'html':
    case 'htm':
      return htmlToText(decode(new Uint8Array(await file.arrayBuffer()), true));
    case 'docx':
      return docxToText(await file.arrayBuffer());
    case 'pdf':
      return pdfToText(await file.arrayBuffer());
    default:
      throw new ImportError(`Формат .${ext} не поддерживается. Подходят: ${ACCEPT}`);
  }
}

/**
 * Кодировка: по BOM, у HTML — из `<meta charset>`; иначе UTF-8, а файл не в UTF-8
 * читается как windows-1251, как в командной строке и в настольном приложении.
 */
export function decode(bytes: Uint8Array, html: boolean): string {
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder('utf-16le').decode(bytes);
  if (bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder('utf-16be').decode(bytes);
  if (html) {
    const head = new TextDecoder('latin1').decode(bytes.subarray(0, 4096));
    const label = /<meta[^>]+charset\s*=\s*["']?([\w-]+)/i.exec(head)?.[1];
    if (label) {
      try {
        return new TextDecoder(label).decode(bytes);
      } catch {
        // Неизвестная кодировка: определяем сами.
      }
    }
  }
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder('windows-1251').decode(bytes);
  }
}

const SKIP = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'NAV', 'HEADER', 'FOOTER', 'ASIDE']);
const LINE = new Set(['BR', 'LI', 'TR', 'DT', 'DD', 'CAPTION', 'FIGCAPTION']);
const CELL = new Set(['TD', 'TH']);
const PARAGRAPH = new Set([
  'P', 'DIV', 'SECTION', 'ARTICLE', 'MAIN', 'BLOCKQUOTE', 'PRE', 'TABLE', 'UL', 'OL', 'DL',
  'H1', 'H2', 'H3', 'H4', 'H5', 'H6',
]);

/**
 * Документ из DOMParser инертен: скрипты не выполняются, картинки не грузятся.
 * Его узлы нельзя переносить в страницу, поэтому текст собирается обходом дерева.
 * Если в файле есть `<style>`, Chrome пишет в консоль нарушение CSP: так и задумано.
 */
function htmlToText(html: string): string {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const out: string[] = [];
  const walk = (node: Node, pre: boolean) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === Node.TEXT_NODE) {
        const data = (child as Text).data;
        out.push(pre ? data : data.replace(/\s+/g, ' '));
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        const tag = (child as Element).tagName;
        if (SKIP.has(tag)) continue;
        // Абзац отделяется пустой строкой с обеих сторон, строка списка — переводом строки
        // после себя (иначе пункты разделились бы пустыми строками), ячейка — пробелом.
        const paragraph = PARAGRAPH.has(tag);
        if (paragraph) out.push('\n\n');
        walk(child, pre || tag === 'PRE');
        out.push(paragraph ? '\n\n' : LINE.has(tag) ? '\n' : CELL.has(tag) ? ' ' : '');
      }
    }
  };
  walk(doc.body, false);
  return out
    .join('')
    .replace(/[ \t]*\n[ \t]*/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/**
 * Текст страницы, загруженной по ссылке (только в настольном приложении).
 * Из HTML берётся основная статья (Readability, как в Firefox Reader View),
 * без меню, рекламы и подвала; если статью выделить не удалось — вся страница.
 */
export async function pageToText(page: Page): Promise<string> {
  if (page.content_type === 'text/plain' || page.content_type === 'text/markdown') return page.text.trim();
  if (page.content_type !== 'text/html' && page.content_type !== 'application/xhtml+xml') {
    throw new ImportError(`По ссылке не текст и не веб-страница, а ${page.content_type || 'неизвестный формат'}.`);
  }
  const { Readability } = await import('@mozilla/readability');
  const doc = new DOMParser().parseFromString(page.text, 'text/html');
  const article = new Readability(doc).parse();
  const text = htmlToText(article?.content ?? page.text);
  if (!text) throw new ImportError('На странице не нашлось текста.');
  return text;
}

async function docxToText(data: ArrayBuffer): Promise<string> {
  const mammoth = await import('mammoth');
  const result = await mammoth.extractRawText({ arrayBuffer: data });
  return result.value.replace(/\n{3,}/g, '\n\n').trim();
}

async function pdfToText(data: ArrayBuffer): Promise<string> {
  const pdfjs = await import('pdfjs-dist');
  const { default: workerUrl } = await import('pdfjs-dist/build/pdf.worker.min.mjs?url');
  pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
  const task = pdfjs.getDocument({ data });
  const pdf = await task.promise;
  const pages: string[] = [];
  for (let n = 1; n <= pdf.numPages; n++) {
    const page = await pdf.getPage(n);
    const content = await page.getTextContent();
    let line = '';
    const lines: string[] = [];
    for (const item of content.items) {
      if (!('str' in item)) continue;
      line += item.str;
      if (item.hasEOL) {
        lines.push(line);
        line = '';
      }
    }
    if (line) lines.push(line);
    pages.push(joinPdfLines(lines));
  }
  await task.destroy();
  const text = pages.join('\n\n').trim();
  if (!text) throw new ImportError('В PDF нет текстового слоя. Сканы без распознавания не поддерживаются.');
  return text;
}

/** Склеивает строки PDF в абзацы: перенос по слогам убирается, пустая строка — граница абзаца. */
function joinPdfLines(lines: string[]): string {
  let out = '';
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      out = out.trimEnd() + '\n\n';
    } else if (/[а-яёa-z]-$/i.test(out)) {
      out = out.slice(0, -1) + line;
    } else {
      out += (out && !out.endsWith('\n') ? ' ' : '') + line;
    }
  }
  return out;
}
