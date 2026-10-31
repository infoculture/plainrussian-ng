"""Оценка простоты (читаемости) русскоязычных текстов.

Ядро написано на Rust (крейт pr-core) и то же самое работает на сайте и в
настольном приложении, поэтому оценки совпадают.

>>> import plainrussian
>>> m = plainrussian.measure("Мама мыла раму. Папа читал газету.")
>>> m["counts"]["words"], m["counts"]["sentences"]
(6, 2)
"""

from __future__ import annotations

import json
from typing import Any

from . import _core

__version__: str = _core.__version__

__all__ = ["measure", "details", "sentenize", "measure_legacy", "__version__"]


def measure(text: str) -> dict[str, Any]:
    """Оценка текста.

    Ключи: ``indexes`` (пять формул и их среднее ``average`` — главная оценка,
    в годах обучения, с описанием уровня), ``counts`` (предложения, слова, слоги,
    буквы, сложные слова, числа), ``features`` (средние, на которых строятся
    формулы), ``calibration`` (версия и коэффициенты). Если в тексте нет слов,
    ``indexes`` и ``features`` равны ``None``.
    """
    return json.loads(_core.measure_json(text))


def details(text: str) -> dict[str, Any]:
    """Оценка плюс подробности для правки текста.

    ``measurement`` — то же, что :func:`measure`; ``sentences`` — предложения
    с числом слов; ``complex_words`` — слова больше чем из четырёх слогов,
    в порядке появления.
    """
    return json.loads(_core.details_json(text))


def sentenize(text: str) -> list[str]:
    """Предложения текста по правилам razdel: сокращения и инициалы не рвут предложение."""
    return _core.sentenize(text)


def measure_legacy(text: str) -> dict[str, Any]:
    """Оценка в формате старого API plainrussian.ru (``/api/1.0/ru/measure/``).

    Повторяет старый алгоритм один в один, вместе с его ошибками подсчёта,
    чтобы можно было сравнить с прежними результатами.
    """
    return json.loads(_core.measure_legacy_json(text))
