// Схема ответа LLM. Используется для структурированного вывода и проверки ответа.
import { z } from 'zod';
import { ISSUE_KINDS, type IssueKind } from './assessment.ts';

// Без JIT: он пробует new Function, а CSP страницы запрещает eval.
z.config({ jitless: true });

const kinds = Object.keys(ISSUE_KINDS) as [IssueKind, ...IssueKind[]];

// strictObject даёт additionalProperties: false — этого требует строгий режим OpenAI.
export const Assessment = z.strictObject({
  grade: z
    .number()
    .describe('Сколько лет обучения нужно, чтобы понять текст: от 1 до 17, где 17 означает «17 и больше»'),
  summary: z.string().describe('Почему такая оценка: 1–3 предложения'),
  issues: z
    .array(
      z.strictObject({
        quote: z.string().describe('Фрагмент текста дословно, как в оригинале, не длиннее 200 знаков'),
        kind: z.enum(kinds),
        comment: z.string().describe('Чем фрагмент мешает пониманию'),
        suggestion: z.string().describe('Как написать проще; пустая строка, если предложить нечего'),
      }),
    )
    .describe('Главные препятствия для понимания, самые важные первыми, не больше 10'),
  advice: z.string().describe('Общий совет автору: 1–3 предложения'),
});

export type Assessment = z.infer<typeof Assessment>;
