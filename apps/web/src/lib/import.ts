// Извлечение текста из файлов. Всё происходит в браузере; разборщики docx и pdf
// загружаются только при первом таком файле.

export const ACCEPT = '.txt,.md,.html,.htm,.docx,.pdf';

export class ImportError extends Error {}

export async function readFileText(file: File): Promise<string> {
  const ext = file.name.toLowerCase().split('.').pop() ?? '';
  switch (ext) {
    case 'txt':
    case 'md':
      return file.text();
    case 'html':
    case 'htm':
      return htmlToText(await file.text());
    case 'docx':
      return docxToText(await file.arrayBuffer());
    case 'pdf':
      return pdfToText(await file.arrayBuffer());
    default:
      throw new ImportError(`Формат .${ext} не поддерживается. Подходят: ${ACCEPT}`);
  }
}

const SKIP = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'NAV', 'HEADER', 'FOOTER', 'ASIDE']);
const LINE = new Set(['BR', 'LI', 'TR', 'DT', 'DD']);
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
        const sep = PARAGRAPH.has(tag) ? '\n\n' : LINE.has(tag) ? '\n' : '';
        out.push(sep);
        walk(child, pre || tag === 'PRE');
        out.push(sep);
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
