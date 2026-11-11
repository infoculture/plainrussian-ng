"""Тесты build_corpus.py. Запуск из корня репозитория:
    python3 -m unittest discover -s calibration
"""

import contextlib
import csv
import hashlib
import io
import tempfile
import unittest
from pathlib import Path

import build_corpus
from build_corpus import COLUMNS, MAX_WORDS, MIN_WORDS, PER_SOURCE, build, cut, paragraphs


def text(paragraph_count, words_each, tag):
    """Текст из разных абзацев по words_each слов; tag отличает источники."""
    return "\n\n".join(
        " ".join(f"{tag}{chr(ord('а') + n % 30)}{chr(ord('а') + w % 30)}" for w in range(words_each)) + f" {tag}{'я' * (n + 1)}."
        for n in range(paragraph_count)
    )


class Paragraphs(unittest.TestCase):
    def test_blank_lines_separate_and_wrapped_lines_join(self):
        self.assertEqual(paragraphs("Первая\nстрока.\r\n\r\n  Вторая.\n \nТретья.\n"),
                         ["Первая строка.", "Вторая.", "Третья."])

    def test_indent_starts_a_paragraph(self):
        self.assertEqual(paragraphs("   Первая\nстрока.\n     Вторая\nстрока.\n\n     Третья.\n"),
                         ["Первая строка.", "Вторая строка.", "Третья."])

    def test_without_blank_lines_and_indents_every_line_is_a_paragraph(self):
        self.assertEqual(paragraphs("Первая.\nВторая.\n"), ["Первая.", "Вторая."])
        self.assertEqual(paragraphs(""), [])


class Cut(unittest.TestCase):
    def test_short_text_is_one_fragment(self):
        self.assertEqual(cut([120, 100]), [(0, 2)])
        self.assertEqual(cut([MAX_WORDS]), [(0, 1)])

    def test_too_short_text_gives_nothing(self):
        self.assertEqual(cut([40, 30]), [])

    def test_long_text_is_cut_evenly(self):
        sizes = [50] * 400  # 20 000 слов
        bounds = cut(sizes)
        self.assertEqual(len(bounds), PER_SOURCE)
        for begin, end in bounds:
            self.assertTrue(MIN_WORDS <= sum(sizes[begin:end]) <= MAX_WORDS)
        # Не пересекаются и стоят в каждой пятой части текста.
        for (_, end), (begin, _) in zip(bounds, bounds[1:]):
            self.assertLessEqual(end, begin)
        self.assertEqual([begin * PER_SOURCE // len(sizes) for begin, _ in bounds], list(range(PER_SOURCE)))

    def test_fewer_fragments_from_medium_text(self):
        self.assertEqual(len(cut([60] * 20)), 2)  # 1200 слов

    def test_large_paragraph_is_kept_whole(self):
        # 280 слов мало, а со следующим абзацем выходит больше MAX_WORDS: абзац не режется.
        self.assertEqual(cut([200, 280, 400]), [(1, 3)])

    def test_text_without_paragraphs_gives_nothing(self):
        self.assertEqual(cut([3000]), [])


class Build(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.corpus = Path(tmp.name) / "corpus"
        self.manifest = Path(tmp.name) / "manifest"
        (self.corpus / "sources").mkdir(parents=True)
        self.manifest.mkdir()

    def sources(self, *rows):
        with (self.manifest / "sources.csv").open("w", encoding="utf-8", newline="") as f:
            writer = csv.DictWriter(f, COLUMNS, restval="", lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)

    def source(self, sid, grade=5, body=None, **extra):
        if body is not None:
            (self.corpus / "sources" / f"{sid}.txt").write_text(body, encoding="utf-8")
        return {"source_id": sid, "grade": grade, "split": "fit", "source_type": "textbook",
                "genre": "educational", "title": sid, **extra}

    def manifest_rows(self):
        with (self.manifest / "fragments.csv").open(encoding="utf-8", newline="") as f:
            return list(csv.DictReader(f))

    def test_writes_fragments_and_manifest(self):
        self.sources(self.source("history-5", body=text(40, 50, "и")),
                     self.source("vpr-4", grade=4, body=text(4, 50, "в"), split="test", source_type="exam"))
        report, errors = build(self.corpus, self.manifest)
        self.assertEqual(errors, [])
        rows = self.manifest_rows()
        self.assertEqual([r["path"] for r in rows],
                         ["history-5/01.txt", "history-5/02.txt", "history-5/03.txt", "history-5/04.txt",
                          "vpr-4/01.txt"])
        for row in rows:
            data = (self.corpus / "fragments" / row["path"]).read_bytes()
            self.assertEqual(row["sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(row["source_id"], row["path"].split("/")[0])
        whole = (self.corpus / "fragments" / "vpr-4" / "01.txt").read_text(encoding="utf-8")
        self.assertEqual(whole, "\n".join(paragraphs(text(4, 50, "в"))) + "\n")
        self.assertIn("Источников: 2, фрагментов: 5", report)
        self.assertEqual(build(self.corpus, self.manifest)[1], [])
        self.assertEqual(self.manifest_rows(), rows)

    def test_removes_stale_fragments_only(self):
        self.sources(self.source("a", body=text(40, 50, "а")), self.source("b", body=text(40, 50, "б")))
        build(self.corpus, self.manifest)
        notes = self.corpus / "fragments" / "a" / "notes.md"
        notes.write_text("свои заметки", encoding="utf-8")
        (self.corpus / "sources" / "a.txt").write_text(text(4, 50, "а"), encoding="utf-8")
        (self.corpus / "sources" / "b.txt").unlink()
        self.sources(self.source("a"))
        report, errors = build(self.corpus, self.manifest)
        self.assertEqual(errors, [])
        self.assertEqual(sorted(p.relative_to(self.corpus).as_posix() for p in self.corpus.rglob("*") if p.is_file()),
                         ["fragments/a/01.txt", "fragments/a/notes.md", "sources/a.txt"])
        self.assertFalse((self.corpus / "fragments" / "b").exists())
        self.assertIn("Удалено устаревших фрагментов: 7", report)

    def test_errors_write_nothing(self):
        same = text(6, 50, "ё")
        other_edition = same.replace("ё", "е").upper().replace(".", "!")
        self.sources(self.source("first", body=same), self.source("second", grade=7, body=other_edition),
                     self.source("missing"), self.source("tiny", body="Три коротких слова."))
        (self.corpus / "sources" / "extra.txt").write_text(same, encoding="utf-8")
        report, errors = build(self.corpus, self.manifest)
        self.assertEqual(report, [])
        self.assertEqual(len(errors), 4, errors)
        joined = "\n".join(errors)
        self.assertIn("sources/extra.txt: источника нет в sources.csv", joined)
        self.assertIn("sources/missing.txt: файла нет", joined)
        self.assertIn("sources/tiny.txt: нет ни одного фрагмента", joined)
        self.assertIn("sources/first.txt и sources/second.txt: общих абзацев — 6", joined)
        self.assertFalse((self.manifest / "fragments.csv").exists())
        self.assertFalse((self.corpus / "fragments").exists())

    def test_rejects_bad_rows(self):
        self.sources(self.source("Плохой id"), self.source("ok"), self.source("ok", grade=18),
                     self.source("x", split="train", source_type="book", genre="prose", title=" "))
        _, errors = build(self.corpus, self.manifest)
        self.assertEqual(len(errors), 7, errors)

    def test_rejects_wrong_columns(self):
        (self.manifest / "sources.csv").write_text("source_id,grade\na,1\n", encoding="utf-8")
        _, errors = build(self.corpus, self.manifest)
        self.assertEqual(len(errors), 1)
        self.assertIn("колонки должны быть", errors[0])

    def test_coverage_marks_short_grades(self):
        self.sources(self.source("a", grade=3, body=text(40, 50, "а"), genre="fiction", source_type="fiction"))
        report, _ = build(self.corpus, self.manifest)
        grade3 = next(line for line in report if line.split()[:1] == ["3"])
        self.assertEqual(grade3.split(), ["3", "1", "4", "0", "0", "мало"])
        self.assertIn("Художественных фрагментов: 100% (цель — не больше 20%).", report)
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(build_corpus.main([str(self.corpus), "--manifest", str(self.manifest)]), 0)
        self.assertEqual(out.getvalue(), "\n".join(report) + "\n")


if __name__ == "__main__":
    unittest.main()
