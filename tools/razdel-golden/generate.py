"""Генерирует эталон деления на предложения из Python-пакета razdel.

Нужен razdel 0.5.0 (pip install razdel==0.5.0), лучше в отдельном venv.
Пустые предложения razdel (для пустого текста и т. п.) отбрасываются, как и в Rust.
Смещения — в символах (code points), как у строк Python.

Тексты корпуса под авторским правом: в фикстуру попадают только путь, SHA-256
и границы предложений. Короткие фрагменты из тестов razdel (MIT) и синтетические
примеры хранятся целиком.

Запуск из корня репозитория:
    path/to/venv/bin/python -I tools/razdel-golden/generate.py
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path

from importlib.metadata import version as package_version

from razdel import sentenize

REPO_ROOT = Path(__file__).resolve().parents[2]
CORPUS = REPO_ROOT.parent / "plainrussian" / "api" / "textmetric" / "textsbygrade"
OUT_DIR = REPO_ROOT / "crates" / "pr-core" / "tests" / "golden"

# Фрагменты из razdel/tests/test_sentenize.py без разметки «|».
RAZDEL_UNIT = [
    'фонетических правил языка; в случае, если',
    '(Прилепин — очень хороший писатель, лучше, чем Лимонов. Но враг)',
    'Петров - 176! Михайлов - 180!',
    'если бы… не тот широко',
    'Георгий Иванов. На грани музыки и сна',
    'исполняется 150 лет. 31 мая 1859 года после неоднократных',
    'И т. д. и т. п. В общем, вся газета',
    'специалистом, к.п.н. И. П. Карташовым.',
    'основании п. 2, ст. 5 УПК',
    'Вблизи оз. Селяха',
    'уменьшить с 20 до 18 проц. (при сохранении',
    '6 июля 2007 г. "в связи с совершением',
    'на 500 тыс. машин',
    'Влияние взглядов Л. В. Щербы',
    'директор фирмы Чарльз Дж. Филлипс',
    'Т.е. ОБЯЗАТЕЛЬНО письменно',
    'была утечка т.н. Таблицы боевых действий',
    'В 1996-1999гг. теффт',
    'России, т. е. 55 % опрошенных',
    'я ощущал в 1990-е. Славное было время',
    'словам, "не будет точно". "Возможно, у нас',
    'Брось!.." Связываться не хотелось',
    'Peter Goldreich,Scott Tremaine (1979). «Относительно теории колец Урана».',
    'Это чудовищные риски. "Яндекс" попал под удар',
    'кто они такие… »',
    '- "Так в чем же дело?" - "Не ра-ду-ют".',
    '— Ты ей скажи, что я ей гостинца дам. — А мне дашь?',
    '4. Я присутствовал во время встречи',
    'IV. Гестационный сахарный диабет',
    '§2. Нахождение оптимального объекта.',
    '8.1. Зачем нужны эти классы?',
    'в данной квартире; 2) отчуждать свою долю',
    'пастухов - тоже ;) Я вспомнила',
    'распределённой жабы :)) А платить мне будут аж 1200 рублей',
]

EDGE_CASES = [
    '',
    '   \n\t ',
    'См. ст. 5 т.е. А.С. Пушкин писал... Что?! Да.',
    'Выручка выросла на 3.5% в 2024 г. И это всё.',
    'Первая строка.\r\nВторая строка.\r\n\r\nТретья',
    'Он сказал: «Приду завтра». Не пришёл.',
    'Цены (в руб.) выросли. Спрос (т.е. продажи) упал!',
    'Привет 👋 мир! 👨‍👩‍👧 семья. Конец',
    'Глава I. Общие положения\n1. Закон регулирует отношения.\n2) Иное.',
    'Ул. Ленина, д. 5, кв. 12. Звоните тел. 123-45-67.',
    'Mr. Smith went to Washington. He was happy!',
    'Он ушёл?! — Нет... — ответила она.',
    'В 1990-е гг. всё было иначе. Так.',
    ':) Смайлик в начале. А это ;-( в середине. Конец :)))',
    'Аббревиатура ООН. Затем ВОЗ. И т.д. Ещё одно предложение',
]


def spans(text):
    return [[s.start, s.stop] for s in sentenize(text) if s.stop > s.start]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    parser.add_argument("--out", type=Path, default=OUT_DIR)
    args = parser.parse_args()

    version = package_version("razdel")
    cases = [{"text": t, "sentences": spans(t)} for t in RAZDEL_UNIT + EDGE_CASES]
    write_json(args.out / "razdel_cases.json", {"razdel": version, "cases": cases})

    corpus = []
    for path in sorted(args.corpus.rglob("*.txt")):
        data = path.read_bytes()
        corpus.append({
            "path": path.relative_to(args.corpus).as_posix(),
            "sha256": hashlib.sha256(data).hexdigest(),
            "sentences": spans(data.decode("utf-8")),
        })
    if not corpus:
        sys.exit(f"{args.corpus}: тексты не найдены")
    write_json(args.out / "razdel_corpus.json", {"razdel": version, "texts": corpus})
    total = sum(len(c["sentences"]) for c in corpus)
    print(f"примеров: {len(cases)}, текстов: {len(corpus)}, предложений в корпусе: {total}")


def write_json(path, data):
    # Компактно: в корпусе тысячи пар чисел.
    path.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
