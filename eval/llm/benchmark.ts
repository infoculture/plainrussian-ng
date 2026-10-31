// Бенчмарк оценки LLM на размеченном корпусе: сравнивает оценку модели с
// экспертной и с ошибкой формул. Использует тот же промпт и те же адаптеры,
// что и веб-приложение (apps/web/src/lib/llm).
//
// Запуск из корня репозитория (Node 23.6+ исполняет TypeScript напрямую):
//   ANTHROPIC_API_KEY=... node eval/llm/benchmark.ts --provider anthropic --model claude-opus-5-5 --yes
//   node eval/llm/benchmark.ts --provider ollama --model qwen3:8b
//
// Облачный провайдер тратит деньги на каждом тексте, поэтому без --yes скрипт
// только показывает объём. Результаты пишутся в eval/llm/results/ и в git не
// попадают: в них цитаты из текстов корпуса, защищённых авторским правом.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import { clampGrade } from '../../apps/web/src/lib/llm/assessment.ts';
import { assess, provider, PROVIDERS, type ProviderId } from '../../apps/web/src/lib/llm/providers.ts';

const ROOT = resolve(import.meta.dirname, '../..');
const CEILING = 17;
const KEY_ENV: Partial<Record<ProviderId, string>> = {
  anthropic: 'ANTHROPIC_API_KEY',
  openai: 'OPENAI_API_KEY',
  openrouter: 'OPENROUTER_API_KEY',
};

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

const { values: args } = parseArgs({
  options: {
    provider: { type: 'string', default: 'anthropic' },
    model: { type: 'string' },
    'base-url': { type: 'string' },
    corpus: { type: 'string', default: join(ROOT, '../plainrussian/api/textmetric/textsbygrade') },
    limit: { type: 'string' },
    yes: { type: 'boolean', default: false },
  },
});

const id = args.provider as ProviderId;
if (!PROVIDERS.some((p) => p.id === id)) {
  fail(`--provider: одно из ${PROVIDERS.map((p) => p.id).join(', ')}`);
}
const p = provider(id);
const model = args.model ?? p.defaultModel;
if (!model) fail('--model: укажите модель');
const envName = KEY_ENV[id];
const apiKey = envName ? (process.env[envName] ?? '') : '';
if (p.key === 'required' && !apiKey) fail(`Нужен ключ в переменной ${envName}`);
const req = { provider: id, model, apiKey, baseUrl: args['base-url'] ?? p.baseUrl, scope: '' };

// corpus.csv: path,grade,fit,title,url,note — в названиях бывают запятые в кавычках,
// но нужны только первые три колонки, а в них запятых нет.
const texts = readFileSync(join(ROOT, 'calibration/corpus.csv'), 'utf8')
  .trim()
  .split('\n')
  .slice(1)
  .map((line) => line.split(','))
  .filter((cols) => cols[2] === '1')
  .map(([path, grade]) => ({ path, grade: Number(grade), text: readFileSync(join(args.corpus, path), 'utf8') }))
  .slice(0, args.limit ? Number(args.limit) : undefined);

const chars = texts.reduce((n, t) => n + t.text.length, 0);
console.log(`${p.title}, ${model}: ${texts.length} текстов, ${chars.toLocaleString('ru-RU')} знаков`);
if (!p.local && !args.yes) {
  console.log('Облачный провайдер: запустите с --yes, чтобы отправить тексты и оплатить запросы.');
  process.exit(0);
}

const formulas = JSON.parse(readFileSync(join(ROOT, 'calibration/2026.1.json'), 'utf8'));
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const outDir = join(ROOT, 'eval/llm/results');
mkdirSync(outDir, { recursive: true });
const outFile = join(outDir, `${stamp}-${id}-${model.replace(/[^\w.-]+/g, '_')}.jsonl`);

const errors: number[] = [];
const lines: string[] = [];
for (const t of texts) {
  const started = performance.now();
  try {
    const a = await assess(t.text, req);
    const grade = clampGrade(a.grade);
    const error = t.grade >= CEILING && grade >= CEILING ? 0 : Math.abs(t.grade - grade);
    errors.push(error);
    lines.push(JSON.stringify({ path: t.path, expected: t.grade, grade, error, issues: a.issues.length, assessment: a }));
    console.log(`${t.path}: эксперт ${t.grade}, модель ${grade.toFixed(1)}, ошибка ${error.toFixed(1)}`);
  } catch (e) {
    lines.push(JSON.stringify({ path: t.path, expected: t.grade, failure: (e as Error).message }));
    console.log(`${t.path}: не получилось: ${(e as Error).message}`);
  }
  lines[lines.length - 1] = lines[lines.length - 1].replace(/}$/, `,"seconds":${((performance.now() - started) / 1000).toFixed(1)}}`);
  writeFileSync(outFile, lines.join('\n') + '\n');
}

const mae = errors.reduce((s, e) => s + e, 0) / (errors.length || 1);
const within2 = errors.filter((e) => e <= 2).length / (errors.length || 1);
console.log(`
Оценено ${errors.length} из ${texts.length}, результаты: ${outFile}
Средняя ошибка LLM: ${mae.toFixed(2)} года, в пределах 2 лет: ${(within2 * 100).toFixed(0)}%
Средняя ошибка формул (калибровка ${formulas.version}, LOO): ${formulas.average.loo.mae} года`);
