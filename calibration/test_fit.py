"""Тесты fit.py (нужен numpy). Запуск из корня репозитория:
    python3 -m unittest discover -s calibration
"""

import contextlib
import csv
import io
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

import fit
from build_corpus import COLUMNS


def run(*argv):
    with contextlib.redirect_stdout(io.StringIO()):
        fit.main(list(argv))


def counts(grade, shift):
    """Счётчики фрагмента, у которого все признаки формул растут с уровнем."""
    sentences = 30 + shift
    words = sentences * (5 + grade)
    return {"sentences": sentences, "words": words, "syllables": round(words * (1.8 + 0.05 * grade)),
            "letters": round(words * (4 + 0.1 * grade)), "complex_words": round(words * 0.02 * grade)}


class LeaveOneSourceOut(unittest.TestCase):
    def test_fragments_of_a_source_are_held_out_together(self):
        # Пять источников лежат на прямой y = 2x + 1, шестой — шесть фрагментов мимо неё.
        x = np.array([0, 1, 2, 3, 4, 10, 11, 12, 13, 14, 15], dtype=float)
        y = np.array([1, 3, 5, 7, 9, 0, 0, 0, 0, 0, 0], dtype=float)
        X = np.column_stack([x, np.ones_like(x)])
        groups = np.array(["a", "b", "c", "d", "e"] + ["off"] * 6)
        by_source = fit.loo_predictions(X, y, groups)
        np.testing.assert_allclose(by_source[5:], 2 * x[5:] + 1, atol=1e-6)
        # Если исключать по одному фрагменту, остальные пять подсказывают ответ.
        by_fragment = fit.loo_predictions(X, y, np.arange(len(y)))
        self.assertGreater(np.abs(by_fragment[5:] - (2 * x[5:] + 1)).min(), 5)


class Graded(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.manifest = Path(tmp.name) / "graded"
        self.out = Path(tmp.name) / "out"
        self.manifest.mkdir()
        self.out.mkdir()
        sources = [(f"book-{grade}", grade, "fit", "textbook") for grade in (2, 4, 6, 8, 10, 12)]
        sources.append(("exam-9", 9, "test", "exam"))
        with (self.manifest / "sources.csv").open("w", encoding="utf-8", newline="") as f:
            writer = csv.DictWriter(f, COLUMNS, restval="", lineterminator="\n")
            writer.writeheader()
            for sid, grade, split, kind in sources:
                writer.writerow({"source_id": sid, "grade": grade, "split": split, "source_type": kind,
                                 "genre": "educational", "title": sid})
        fragments = [(f"{sid}/{n:02}.txt", sid, grade, n) for sid, grade, _, _ in sources for n in (1, 2, 3)]
        with (self.manifest / "fragments.csv").open("w", encoding="utf-8", newline="") as f:
            writer = csv.writer(f, lineterminator="\n")
            writer.writerow(["path", "source_id", "sha256"])
            writer.writerows([path, sid, ""] for path, sid, _, _ in fragments)
        self.features = [json.dumps({"path": path, "counts": counts(grade, n)}) for path, _, grade, n in fragments]
        (self.manifest / "features.jsonl").write_text("\n".join(self.features) + "\n", encoding="utf-8")

    def test_fits_on_fit_split_and_reports_the_rest(self):
        run("--version", "test", "--graded", str(self.manifest), "--out", str(self.out))
        result = json.loads((self.out / "test.json").read_text(encoding="utf-8"))
        self.assertEqual((result["texts"], result["sources"], result["test_texts"]), (18, 6, 3))
        self.assertEqual([len(f["coef"]) for f in result["formulas"].values()], [3, 3, 2, 3, 3])
        # Признаки линейны по уровню, поэтому новые формулы почти не ошибаются.
        for name in ("fit", "loo", "test"):
            self.assertLess(result["average"][name]["mae"], 0.3, name)
        self.assertGreater(result["average"]["baseline"]["mae"], 1)
        self.assertEqual([row["key"] for row in result["levels"]], [2, 4, 6, 8, 10, 12])
        self.assertEqual([(row["key"], row["sources"], row["texts"]) for row in result["source_types"]],
                         [("textbook", 6, 18)])
        report = (self.out / "REPORT.md").read_text(encoding="utf-8")
        for part in ("на 18 фрагментах", "из 6 источников", "## По уровням", "## По типам источников",
                     "Отложенные — 3 фрагментов", "## Коэффициенты"):
            self.assertIn(part, report)

    def test_rejects_features_that_do_not_match_fragments(self):
        (self.manifest / "features.jsonl").write_text("\n".join(self.features[1:]) + "\n", encoding="utf-8")
        with self.assertRaises(SystemExit) as raised:
            run("--version", "test", "--graded", str(self.manifest), "--out", str(self.out))
        self.assertIn("features.jsonl не совпадает с fragments.csv", str(raised.exception))


class Legacy(unittest.TestCase):
    def test_old_corpus_still_gives_the_current_calibration(self):
        with tempfile.TemporaryDirectory() as out:
            run("--version", fit.BASELINE, "--out", out)
            new = json.loads((Path(out) / f"{fit.BASELINE}.json").read_text(encoding="utf-8"))
        old = json.loads((fit.ROOT / f"{fit.BASELINE}.json").read_text(encoding="utf-8"))
        for key in fit.FORMULAS:
            np.testing.assert_allclose(new["formulas"][key]["coef"], old["formulas"][key]["coef"], atol=1e-3)
        self.assertAlmostEqual(new["average"]["loo"]["mae"], old["average"]["loo"]["mae"], delta=1e-3)


if __name__ == "__main__":
    unittest.main()
