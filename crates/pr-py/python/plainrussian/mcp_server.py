"""MCP-сервер: оценка простоты русских текстов для LLM-агентов.

Запуск (нужен пакет с дополнением ``mcp``: ``pip install "plainrussian[mcp]"``)::

    plainrussian-mcp                         # stdio, для Claude Desktop и других клиентов
    plainrussian-mcp --http 127.0.0.1:9000   # Streamable HTTP

Текст обрабатывается локально, наружу ничего не отправляется.
"""

from __future__ import annotations

import argparse
from collections import Counter
from typing import Any

from mcp.server.mcpserver import MCPServer
from mcp.types import ToolAnnotations

import plainrussian

INSTRUCTIONS = """\
Инструменты оценивают, насколько просто читать текст на русском языке.
Оценка — число лет обучения, нужное для понимания: 1–3 — младшая школа,
7–9 — средняя школа, 12–14 — первые курсы вуза, 17 и больше — законы и канцелярит.
measure_readability даёт оценку и показатели; find_hard_parts — длинные предложения
и сложные слова, чтобы предложить автору, что упростить."""

server = MCPServer(name="plainrussian", instructions=INSTRUCTIONS, version=plainrussian.__version__)

READ_ONLY = ToolAnnotations(read_only_hint=True, destructive_hint=False, idempotent_hint=True, open_world_hint=False)


@server.tool(annotations=READ_ONLY)
def measure_readability(text: str) -> dict[str, Any]:
    """Оценивает простоту русского текста: сколько лет обучения нужно, чтобы его понять.

    Возвращает главную оценку (среднее пяти формул), уровень словами, оценки каждой
    формулы, счётчики (слова, предложения, слоги, сложные слова) и средние показатели.
    Оценка надёжна для текстов от двух предложений и десяти слов.
    """
    m = plainrussian.measure(text)
    indexes = m["indexes"]
    result: dict[str, Any] = {
        "counts": m["counts"],
        "features": m["features"],
        "calibration": m["calibration"]["version"],
        "average_error_years": m["calibration"]["average_error"],
    }
    if indexes is None:
        result["note"] = "В тексте нет слов, оценивать нечего."
        return result
    # Шкала размечена от 1 до 17 («17 и больше»); за её пределами формулы экстраполируют.
    raw = indexes["average"]["value"]
    result["grade_years"] = round(min(max(raw, 1.0), 17.0), 2)
    result["grade_years_raw"] = round(raw, 2)
    if raw < 1 or raw >= 17:
        result["note"] = "Оценка за пределами шкалы 1–17 лет: текст проще младшей школы или сложнее вуза."
    result["level"] = indexes["average"]["grade"]
    result["formulas"] = {
        name: round(indexes[key]["value"], 2)
        for name, key in [
            ("Flesch-Kincaid", "fk"),
            ("Coleman-Liau", "cl"),
            ("SMOG", "smog"),
            ("Dale-Chale", "dc"),
            ("ARI", "ari"),
        ]
    }
    return result


@server.tool(annotations=READ_ONLY)
def find_hard_parts(text: str, min_sentence_words: int = 20, max_items: int = 20) -> dict[str, Any]:
    """Находит, что мешает понять русский текст: длинные предложения и сложные слова.

    min_sentence_words — с какого числа слов предложение считается длинным (по умолчанию 20).
    max_items — сколько предложений и слов вернуть, самые длинные и частые первыми.
    Сложное слово — больше четырёх слогов.
    """
    d = plainrussian.details(text)
    long_sentences = sorted(
        (s for s in d["sentences"] if s["words"] >= min_sentence_words),
        key=lambda s: s["words"],
        reverse=True,
    )
    syllables = {w["text"].lower(): w["syllables"] for w in d["complex_words"]}
    frequent = Counter(w["text"].lower() for w in d["complex_words"]).most_common(max_items)
    return {
        "long_sentences": long_sentences[:max_items],
        "long_sentences_total": len(long_sentences),
        "complex_words": [{"word": w, "syllables": syllables[w], "count": n} for w, n in frequent],
        "complex_words_total": len(d["complex_words"]),
        "words_total": d["measurement"]["counts"]["words"],
    }


def main() -> None:
    parser = argparse.ArgumentParser(prog="plainrussian-mcp", description=__doc__.splitlines()[0])
    parser.add_argument("--http", metavar="HOST:PORT", help="Streamable HTTP вместо stdio, например 127.0.0.1:9000")
    args = parser.parse_args()
    if args.http:
        host, _, port = args.http.rpartition(":")
        server.run("streamable-http", host=host or "127.0.0.1", port=int(port))
    else:
        server.run()


if __name__ == "__main__":
    main()
