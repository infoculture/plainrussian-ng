"""Строит calibration/corpus.csv: список текстов корпуса с экспертными уровнями.

Источник — TEXT_LIST из старого api/textmetric/metric.py: название, ссылка и
уровень (лет обучения). Строки, закомментированные в TEXT_LIST, и файлы, которых
в нём нет, попадают в список с fit=0: старые коэффициенты подбирались без них.

Запуск из корня репозитория (нужен соседний репозиторий plainrussian):
    python3 -I calibration/make_corpus_list.py
"""

import ast
import csv
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
LEGACY = REPO_ROOT.parent / "plainrussian" / "api" / "textmetric"

# Известные изъяны разметки; см. REPORT.md.
NOTES = {
    "11/text_msterms.txt": "копия 17/text_msterms.txt",
    "1/gaidar_kamen.txt": "тот же рассказ, что 3/gaidar_stone.txt, с другой оценкой",
    "3/gaidar_stone.txt": "тот же рассказ, что 1/gaidar_kamen.txt, с другой оценкой",
}


def text_list(source, commented):
    """Строки TEXT_LIST вида ['file.txt', grade, 'url', u'Название']."""
    prefix = r"#\s*" if commented else r"(?<!#)\s{4}"
    pattern = re.compile(r"^" + prefix + r"(\['[^']+\.txt',\s*\d+,.*\]),?\s*$", re.M)
    for match in pattern.finditer(source):
        filename, grade, url, title = ast.literal_eval(match.group(1).replace("u'", "'"))
        yield filename, grade, url, title


def main():
    source = (LEGACY / "metric.py").read_text(encoding="utf-8")
    start = source.index("TEXT_LIST = [")
    end = source.index("\n]\n", start)
    block = source[start:end]

    known = {}
    for fit, commented in ((1, False), (0, True)):
        for filename, grade, url, title in text_list(block, commented):
            known[f"{grade}/{filename}"] = (grade, title, url, fit)

    rows = []
    for path in sorted((LEGACY / "textsbygrade").rglob("*.txt")):
        rel = path.relative_to(LEGACY / "textsbygrade").as_posix()
        grade, title, url, fit = known.get(rel, (int(rel.split("/")[0]), "", "", 0))
        rows.append([rel, grade, fit, title, url, NOTES.get(rel, "")])

    out = REPO_ROOT / "calibration" / "corpus.csv"
    with out.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(["path", "grade", "fit", "title", "url", "note"])
        writer.writerows(rows)
    print(f"{out}: {len(rows)} текстов, для подбора: {sum(r[2] for r in rows)}")


if __name__ == "__main__":
    main()
