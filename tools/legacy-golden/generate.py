"""Генерирует golden-фикстуры для legacy-алгоритма из исходного Python-кода.

Источник — боевой `api/textmetric/metric.py` из репозитория plainrussian
(Python 2). Python 2 не нужен: скрипт берёт код до `def print_metrics`,
убирает единственный `print` и запускает его в Python 3. Разница семантики
Python 2 и 3 в этом коде — только round(): он подменяется на округление
половин от нуля, как в Python 2.

Корпус под авторским правом, поэтому в фикстуру попадают только путь,
SHA-256 и результат. Тексты граничных случаев синтетические и хранятся целиком.

Запуск из корня репозитория:
    python3 -I tools/legacy-golden/generate.py
"""

import argparse
import decimal
import hashlib
import json
import sys
import types
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LEGACY_ROOT = REPO_ROOT.parent / "plainrussian" / "api" / "textmetric"
OUT_DIR = REPO_ROOT / "crates" / "pr-core" / "tests" / "golden"

EDGE_CASES = [
    ("empty", ""),
    ("spaces_only", "   \t  "),
    ("only_punct", "...!!!???"),
    ("no_sentence_end", "просто слова без точки в конце"),
    ("no_vowels", "Пск ВВП ст. гг."),
    ("short", "Да."),
    ("simple", "Мама мыла раму. Папа читал газету. Кот спал."),
    ("uppercase", "ОБРАЗОВАНИЕ. Образование. образование."),
    ("abbreviations", "См. ст. 5 т.е. А.С. Пушкин писал... Что?! Да."),
    ("numbers", "Выручка выросла на 3.5% в 2024 г."),
    ("yo", "Ёжик ёлку нёс. ЁЁЁ ёёё."),
    ("latin_mixed", "Компания Google и iPhone-приложение? Да!"),
    ("long_words", "Высокопревосходительство и человеконенавистничество."),
    (
        "bureaucratic",
        "Комиссия Правительства по законопроектной деятельности одобрила законопроект, "
        "направленный на повышение эффективности реализации государственной политики "
        "и выполнения мероприятий в области гражданской обороны и предупреждения "
        "чрезвычайных ситуаций природного и техногенного характера.",
    ),
    ("crlf", "Первая строка.\r\nВторая строка.\r\n\r\nТретья."),
    ("unicode_spaces", "Слово слово слово　слово слово."),
    (
        "line_separators",
        "раз два три\x85четыре\x0bпять\x0cшесть\x1cсемь\x1dвосемь\x1eдевять\x1fдесять.",
    ),
    ("combining", "йод йогурт."),
    ("bom", "﻿Текст с BOM."),
    ("emoji", "Привет 👋 мир! 👨‍👩‍👧 семья."),
]


def py2_round(x):
    """round() из Python 2: точное значение float, половины от нуля."""
    d = decimal.Decimal(x).quantize(decimal.Decimal(1), rounding=decimal.ROUND_HALF_UP)
    return float(d)


def load_legacy(metric_path):
    source = metric_path.read_text(encoding="utf-8")
    head, sep, _ = source.partition("\ndef print_metrics")
    if not sep:
        sys.exit(f"{metric_path}: не найден def print_metrics")
    if head.count("print grade") != 1:
        sys.exit(f"{metric_path}: ожидался ровно один 'print grade'")
    head = head.replace("print grade", "pass")
    try:
        import numpy  # noqa: F401  (метрики numpy не используют, нужен только import)
    except ImportError:
        sys.modules["numpy"] = types.SimpleNamespace(mean=None, arange=None)
    namespace = {"__name__": "legacy_metric", "round": py2_round}
    exec(compile(head, str(metric_path), "exec"), namespace)
    return namespace["calc_readability_metrics"]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--metric", type=Path, default=LEGACY_ROOT / "metric.py")
    parser.add_argument("--corpus", type=Path, default=LEGACY_ROOT / "textsbygrade")
    parser.add_argument("--out", type=Path, default=OUT_DIR)
    args = parser.parse_args()

    measure = load_legacy(args.metric)
    args.out.mkdir(parents=True, exist_ok=True)

    edge = [{"name": name, "text": text, "expected": measure(text)} for name, text in EDGE_CASES]
    write_json(args.out / "legacy_edge_cases.json", edge)

    corpus = []
    for path in sorted(args.corpus.rglob("*.txt")):
        data = path.read_bytes()
        corpus.append({
            "path": path.relative_to(args.corpus).as_posix(),
            "sha256": hashlib.sha256(data).hexdigest(),
            "expected": measure(data.decode("utf-8")),
        })
    if not corpus:
        sys.exit(f"{args.corpus}: тексты не найдены")
    write_json(args.out / "legacy_corpus.json", corpus)
    print(f"граничных случаев: {len(edge)}, текстов корпуса: {len(corpus)} -> {args.out}")


def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
