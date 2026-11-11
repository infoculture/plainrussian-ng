"""Режет тексты нового корпуса на фрагменты и строит его манифест.

Новый корпус — тексты, у которых уровень (лет обучения) известен из источника:
школьные учебники, экзаменационные тексты, вузовские учебники, законы. Сами тексты
защищены авторским правом и лежат вне репозитория, в репозитории — только описание.

Вход:
- calibration/graded/sources.csv — источники, заполняется вручную (колонки ниже);
- <корпус>/sources/<source_id>.txt — очищенный связный текст источника в UTF-8, без
  заголовков, заданий, таблиц, подписей и стихов. Разбивка на строки и абзацы не
  важна. Короткие тексты одного источника (например, тексты для проверки техники
  чтения) складываются в один файл.

Выход:
- <корпус>/fragments/<source_id>/NN.txt — фрагменты; устаревшие файлы NN.txt удаляются;
- calibration/graded/fragments.csv — путь, источник и sha256 каждого фрагмента.

Из длинного текста берётся до PER_SOURCE фрагментов по MIN_WORDS–MAX_WORDS слов,
равномерно от начала к концу. Фрагмент начинается и кончается на границе предложений.
Текст не длиннее MAX_WORDS слов — один фрагмент. Слова и предложения здесь считаются
приблизительно, точные счётчики даёт corpus_features.

Колонки sources.csv:
- source_id — латиница, цифры, «-» и «_»; он же имя файла и группа при проверке
  формул: фрагменты одного источника не делятся между подбором и проверкой;
- grade — уровень, от 1 до 17 (17 — «17 и выше»);
- split — fit (подбор коэффициентов) или test (отложенная проверка);
- source_type, genre — значения в SOURCE_TYPES и GENRES;
- subject, title, author, publisher, year, locator (параграфы, страницы), url, note —
  описание источника; обязательно только title.

Запуск из корня репозитория:
    python3 calibration/build_corpus.py <корпус>
"""

import argparse
import csv
import hashlib
import re
import sys
from bisect import bisect_left, bisect_right
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent

MIN_WORDS = 300
MAX_WORDS = 500
SHORTEST = 100
PER_SOURCE = 5
# Один текст в двух источниках: столько общих предложений не короче SHARED_WORDS слов.
# Короткие предложения и одна-две общие цитаты повтором не считаются.
SHARED_WORDS = 6
SHARED_SENTENCES = 3

GRADES = range(1, 18)
# Цель сбора для школьных уровней: фрагментов и источников на уровень в подборе.
SCHOOL_GRADES = range(1, 12)
TARGET_FRAGMENTS = 40
TARGET_SOURCES = 8
FICTION_SHARE = 0.2

COLUMNS = ["source_id", "grade", "split", "source_type", "genre", "subject", "title",
           "author", "publisher", "year", "locator", "url", "note"]
SPLITS = {"fit", "test"}
SOURCE_TYPES = {
    "textbook": "школьный учебник",
    "exam": "экзаменационный или диагностический текст: ВПР, ОГЭ, ЕГЭ, PIRLS, PISA",
    "reading": "текст для проверки техники чтения",
    "university": "вузовский учебник",
    "article": "научная статья",
    "official": "закон, государственный документ",
    "fiction": "художественное произведение",
}
GENRES = {
    "educational": "учебный",
    "popular": "научно-популярный",
    "fiction": "художественный",
    "scientific": "научный",
    "official": "официальный",
}

WORD = re.compile(r"[^\W\d_]+")
# Конец предложения: «!», «?», «…» или точка не после одной буквы (инициал, «г.»),
# дальше пробел и заглавная буква или цифра, возможно после кавычки или тире.
SENTENCE_END = re.compile(r"(?:(?<!\W\w)\.|[!?…])[.!?…]*[»”\")\]]*\s+(?=[«„\"(—–-]?\s*[А-ЯЁA-Z0-9])")
SOURCE_ID = re.compile(r"[a-z0-9][a-z0-9_-]*")
FRAGMENT_FILE = re.compile(r"\d\d\.txt")


def count_words(text):
    return len(WORD.findall(text))


def sentences(text):
    """Предложения текста как срезы строки, без потерь: в сумме дают весь текст."""
    starts = [0] + [m.end() for m in SENTENCE_END.finditer(text)]
    return [text[begin:end] for begin, end in zip(starts, starts[1:] + [len(text)])]


def cut(sizes):
    """Границы фрагментов (начало, конец) в предложениях по числу слов в каждом."""
    total = sum(sizes)
    if total <= MAX_WORDS:
        return [(0, len(sizes))] if total >= SHORTEST else []
    offsets = [0]
    for size in sizes[:-1]:
        offsets.append(offsets[-1] + size)
    # Последнее предложение, после которого до конца текста ещё хватает слов.
    last = bisect_right(offsets, total - MIN_WORDS) - 1
    count = min(PER_SOURCE, total // MAX_WORDS)
    zone = total / count
    result = []
    free = 0
    for i in range(count):
        # Фрагмент стоит посередине своей доли текста.
        begin = max(free, min(last, bisect_left(offsets, i * zone + (zone - MAX_WORDS) / 2)))
        end, words = begin, 0
        while end < len(sizes) and (words < MIN_WORDS or words + sizes[end] <= MAX_WORDS):
            words += sizes[end]
            end += 1
        if words >= MIN_WORDS:
            result.append((begin, end))
            free = end
    return result


def read_sources(path):
    """Строки sources.csv и ошибки в них."""
    with path.open(encoding="utf-8-sig", newline="") as f:
        reader = csv.DictReader(f)
        if reader.fieldnames != COLUMNS:
            return [], [f"{path.name}: колонки должны быть {','.join(COLUMNS)}"]
        rows = list(reader)
    errors = []
    seen = set()
    for number, row in enumerate(rows, start=2):
        where = f"{path.name}, строка {number}"
        sid = row["source_id"]
        if not SOURCE_ID.fullmatch(sid or ""):
            errors.append(f"{where}: source_id «{sid}» — нужны латиница, цифры, «-» и «_»")
        elif sid in seen:
            errors.append(f"{where}: source_id «{sid}» повторяется")
        seen.add(sid)
        if not (row["grade"] or "").isdecimal() or int(row["grade"]) not in GRADES:
            errors.append(f"{where}: grade «{row['grade']}» — нужно целое от {GRADES[0]} до {GRADES[-1]}")
        for column, allowed in (("split", SPLITS), ("source_type", SOURCE_TYPES), ("genre", GENRES)):
            if row[column] not in allowed:
                errors.append(f"{where}: {column} «{row[column]}» — одно из {', '.join(sorted(allowed))}")
        if not (row["title"] or "").strip():
            errors.append(f"{where}: нет title")
    return rows, errors


def build(corpus, manifest):
    """Строит фрагменты и fragments.csv. Возвращает (строки отчёта, ошибки).

    При ошибках ничего не записывает.
    """
    rows, errors = read_sources(manifest / "sources.csv")
    if errors:
        return [], errors
    source_dir = corpus / "sources"
    known = {row["source_id"] for row in rows}
    if source_dir.is_dir():
        for path in sorted(source_dir.glob("*.txt")):
            if path.stem not in known:
                errors.append(f"sources/{path.name}: источника нет в sources.csv")

    fragments = []  # (путь, source_id, текст)
    shared = {}  # буквы длинного предложения -> первый источник с ним
    overlaps = Counter()
    for row in rows:
        sid = row["source_id"]
        path = source_dir / f"{sid}.txt"
        if not path.is_file():
            errors.append(f"sources/{sid}.txt: файла нет")
            continue
        text = path.read_text(encoding="utf-8-sig").replace("\r\n", "\n").replace("\r", "\n").strip()
        parts = sentences(text)
        sizes = [count_words(part) for part in parts]
        # Издания одного текста расходятся в «ё», регистре, пунктуации и переносах строк.
        long = {"".join(WORD.findall(part.casefold().replace("ё", "е")))
                for part, size in zip(parts, sizes) if size >= SHARED_WORDS}
        for letters in long:
            first = shared.setdefault(letters, sid)
            if first != sid:
                overlaps[first, sid] += 1
        bounds = cut(sizes)
        if not bounds:
            errors.append(f"sources/{sid}.txt: в тексте меньше {SHORTEST} слов")
        for number, (begin, end) in enumerate(bounds, start=1):
            if sum(sizes[begin:end]) > 2 * MAX_WORDS:
                errors.append(f"sources/{sid}.txt: фрагмент {number:02} длиннее {2 * MAX_WORDS} слов — "
                              "в тексте не хватает концов предложений")
            fragments.append((f"{sid}/{number:02}.txt", sid, "".join(parts[begin:end]).strip() + "\n"))
    for (first, second), count in sorted(overlaps.items()):
        if count >= SHARED_SENTENCES:
            errors.append(f"sources/{first}.txt и sources/{second}.txt: общих предложений — {count}; "
                          "один текст не должен попасть в корпус дважды")
    if errors:
        return [], errors

    fragment_dir = corpus / "fragments"
    stale = 0
    if fragment_dir.is_dir():
        fresh = {path for path, _, _ in fragments}
        for path in sorted(fragment_dir.glob("*/*.txt")):
            rel = path.relative_to(fragment_dir).as_posix()
            if FRAGMENT_FILE.fullmatch(path.name) and rel not in fresh:
                path.unlink()
                stale += 1
                if not any(path.parent.iterdir()):
                    path.parent.rmdir()
    with (manifest / "fragments.csv").open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f, lineterminator="\n")
        writer.writerow(["path", "source_id", "sha256"])
        for rel, sid, text in fragments:
            data = text.encode("utf-8")
            target = fragment_dir / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            writer.writerow([rel, sid, hashlib.sha256(data).hexdigest()])

    report = coverage(rows, Counter(sid for _, sid, _ in fragments))
    if stale:
        report.append(f"Удалено устаревших фрагментов: {stale}")
    return report, []


def coverage(rows, per_source):
    """Отчёт: сколько источников и фрагментов собрано на каждый уровень."""
    lines = [f"Источников: {len(rows)}, фрагментов: {sum(per_source.values())}", "",
             "уровень  подбор: источников  фрагментов   проверка: источников  фрагментов"]
    for grade in GRADES:
        cells = []
        for split in ("fit", "test"):
            ids = [r["source_id"] for r in rows if int(r["grade"]) == grade and r["split"] == split]
            cells += [len(ids), sum(per_source[sid] for sid in ids)]
        short = grade in SCHOOL_GRADES and (cells[0] < TARGET_SOURCES or cells[1] < TARGET_FRAGMENTS)
        lines.append(f"{grade:>7}  {cells[0]:>18}  {cells[1]:>10}   {cells[2]:>20}  {cells[3]:>10}"
                     + ("   мало" if short else ""))
    lines += ["", f"Цель для уровней {SCHOOL_GRADES[0]}–{SCHOOL_GRADES[-1]} в подборе: от {TARGET_FRAGMENTS} "
                  f"фрагментов из {TARGET_SOURCES} источников."]
    total = sum(per_source.values())
    if total:
        fiction = sum(per_source[r["source_id"]] for r in rows if r["genre"] == "fiction")
        lines.append(f"Художественных фрагментов: {fiction / total:.0%} "
                     f"(цель — не больше {FICTION_SHARE:.0%}).")
    return lines


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("corpus", type=Path, help="каталог корпуса с подкаталогом sources")
    parser.add_argument("--manifest", type=Path, default=ROOT / "graded",
                        help="каталог с sources.csv, туда же пишется fragments.csv")
    args = parser.parse_args(argv)
    if not args.corpus.is_dir():
        parser.error(f"нет каталога {args.corpus}")
    report, errors = build(args.corpus, args.manifest)
    for line in errors:
        print(line, file=sys.stderr)
    if errors:
        return 1
    print("\n".join(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
