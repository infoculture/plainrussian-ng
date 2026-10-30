"""Подбирает коэффициенты формул читаемости и пишет калибровку и отчёт.

Вход:
- calibration/corpus.csv — тексты и экспертные уровни (лет обучения), колонка fit;
- calibration/features.jsonl — счётчики текстов от Rust:
  cargo run -p pr-core --example corpus_features -- <textsbygrade> > calibration/features.jsonl

Выход:
- calibration/<версия>.json — коэффициенты и ошибки, их же содержит
  crates/pr-core/src/readability.rs (проверяет тест);
- calibration/REPORT.md — отчёт для людей.

Метод. Все формулы линейны по коэффициентам, поэтому подбор — линейная регрессия.
Минимизируется средняя абсолютная ошибка (IRLS), а не квадратичная: она устойчивее
к отдельным странным текстам и понятнее («в среднем ошибаемся на N лет»).
Уровень 17 в корпусе означает «17 и выше» (законы, госдокументы), поэтому, как и
в старом коде, прогноз от 17 для такого текста считается верным. Качество
оценивается перекрёстной проверкой с исключением по одному (LOO): формула
подбирается без текста и проверяется на нём.

Запуск из корня репозитория:
    python3 calibration/fit.py --version 2026.1
"""

import argparse
import csv
import datetime
import json
from math import sqrt
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent
CEILING = 17
DIGITS = 4

FORMULAS = {
    "fk": ("Flesch-Kincaid", ["слов в предложении", "слогов в слове"]),
    "cl": ("Coleman-Liau", ["букв на 100 слов", "предложений на 100 слов"]),
    "smog": ("SMOG", ["√(сложных слов на 30 предложений)"]),
    "dc": ("Dale-Chale (упрощённая)", ["сложных слов на 100 слов", "слов в предложении"]),
    "ari": ("ARI", ["букв в слове", "слов в предложении"]),
}

# Коэффициенты старого plainrussian.ru в тех же признаках (legacy.rs).
LEGACY_COEF = {
    "fk": [0.318, 14.2, -30.5],
    "cl": [0.055, -0.35, -20.33],
    "smog": [1.1 * sqrt(64.6 / 30), 0.05],
    "dc": [0.552, 0.273, 0.0],
    "ari": [6.26, 0.2805, -31.04],
}


def features(c, formula):
    """Признаки формулы; последний столбец — свободный член. Как в readability.rs."""
    words, sentences = c["words"], c["sentences"]
    wps = words / sentences
    return {
        "fk": [wps, c["syllables"] / words],
        "cl": [100 * c["letters"] / words, 100 * sentences / words],
        "smog": [sqrt(30 * c["complex_words"] / sentences)],
        "dc": [100 * c["complex_words"] / words, wps],
        "ari": [c["letters"] / words, wps],
    }[formula] + [1.0]


def errors(y, pred):
    err = np.abs(y - pred)
    err[(y >= CEILING) & (pred >= CEILING)] = 0.0
    return err


def fit_lad(X, y, iterations=500):
    """Минимум средней абсолютной ошибки с учётом потолка (IRLS)."""
    theta = np.linalg.lstsq(X, y, rcond=None)[0]
    for _ in range(iterations):
        pred = X @ theta
        active = ~((y >= CEILING) & (pred >= CEILING))
        w = np.where(active, 1.0 / np.maximum(np.abs(y - pred), 1e-6), 0.0)
        sw = np.sqrt(w)
        new = np.linalg.lstsq(X * sw[:, None], y * sw, rcond=None)[0]
        if np.allclose(new, theta, atol=1e-10):
            break
        theta = new
    return theta


def loo_predictions(X, y):
    pred = np.empty_like(y)
    for i in range(len(y)):
        mask = np.arange(len(y)) != i
        pred[i] = X[i] @ fit_lad(X[mask], y[mask])
    return pred


def summary(err):
    return {"mae": round(float(err.mean()), 3), "max": round(float(err.max()), 3),
            "within_2": round(float((err <= 2).mean()), 3)}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    args = parser.parse_args()

    with (ROOT / "corpus.csv").open(encoding="utf-8") as f:
        corpus = {r["path"]: r for r in csv.DictReader(f)}
    rows = [json.loads(line) for line in (ROOT / "features.jsonl").open(encoding="utf-8")]
    rows = [r for r in rows if corpus[r["path"]]["fit"] == "1"]
    y = np.array([float(corpus[r["path"]]["grade"]) for r in rows])

    result = {"version": args.version, "created": datetime.date.today().isoformat(),
              "texts": len(rows), "ceiling": CEILING, "formulas": {}}
    preds = {"fit": [], "loo": [], "legacy": [], "legacy_refit_loo": []}
    for key, (title, names) in FORMULAS.items():
        X = np.array([features(r["counts"], key) for r in rows])
        coef = [round(float(v), DIGITS) for v in fit_lad(X, y)]
        X_old = np.array([features(r["legacy"], key) for r in rows])
        pred = {
            "fit": X @ np.array(coef),
            "loo": loo_predictions(X, y),
            "legacy": X_old @ np.array(LEGACY_COEF[key]),
            "legacy_refit_loo": loo_predictions(X_old, y),
        }
        result["formulas"][key] = {"title": title, "features": names, "coef": coef}
        for name, p in pred.items():
            preds[name].append(p)
            result["formulas"][key][name] = summary(errors(y, p))
    # Главная оценка нового алгоритма — среднее пяти формул.
    result["average"] = {"title": "Среднее пяти формул"}
    for name, p in preds.items():
        result["average"][name] = summary(errors(y, np.mean(p, axis=0)))

    out = ROOT / f"{args.version}.json"
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    write_report(result, ROOT / "REPORT.md")
    print(json.dumps(result["formulas"], ensure_ascii=False, indent=1))
    print("average", result["average"])


def write_report(r, path):
    f = r["formulas"]
    lines = [
        f"# Калибровка {r['version']}",
        "",
        f"Создана {r['created']} скриптом `calibration/fit.py` на {r['texts']} текстах корпуса",
        "(колонка `fit` в `corpus.csv` — тот же набор, на котором подбирались старые коэффициенты).",
        f"Ошибка — разница с экспертной оценкой в годах обучения; уровень {r['ceiling']} означает",
        f"«{r['ceiling']} и выше», прогноз от {r['ceiling']} для такого текста считается верным.",
        "",
        "## Средняя ошибка, лет обучения",
        "",
        "LOO — формула подбирается без проверяемого текста, это честная оценка на новых текстах.",
        "Старые коэффициенты подбирались на этих же текстах, поэтому их ошибка занижена.",
        "",
        "| Формула | Старый сайт | Старый подсчёт, LOO | Новый подсчёт, LOO | Новый, на обучении | Новый, макс. LOO |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for v in [*f.values(), r["average"]]:
        lines.append(f"| {v['title']} | {v['legacy']['mae']} | {v['legacy_refit_loo']['mae']} | "
                     f"{v['loo']['mae']} | {v['fit']['mae']} | {v['loo']['max']} |")
    lines += [
        "",
        "Главная оценка нового алгоритма — среднее пяти формул: по средней ошибке оно близко",
        "к лучшей формуле, а по максимальной — лучше любой из них.",
        "",
        "Новый подсчёт на этом корпусе точнее не стал: корпус состоит из длинных литературных",
        "и официальных текстов, где сокращений, заглавных и чисел мало. Его выигрыш — в",
        "устойчивости: текст заглавными буквами, «т.е.», «3.5» и инициалы больше не ломают оценку.",
        "Например, если перевести тексты корпуса в заглавные буквы, старая оценка уезжает на",
        "годы, а новая почти не меняется (тест `crates/pr-core/tests/robustness.rs`).",
    ]
    lines += [
        "",
        "## Коэффициенты",
        "",
        "Каждая формула: `коэффициенты × признаки + свободный член`.",
        "",
        "| Формула | Признаки | Коэффициенты | Свободный член |",
        "|---|---|---|---:|",
    ]
    for v in f.values():
        lines.append(f"| {v['title']} | {', '.join(v['features'])} | "
                     f"{', '.join(str(c) for c in v['coef'][:-1])} | {v['coef'][-1]} |")
    lines += [
        "",
        "## Ограничения",
        "",
        f"- Текстов мало ({r['texts']}), а уровни распределены неравномерно: 15 текстов уровня 17,",
        "  по одному-три текста на большинство школьных классов, уровней 12–14 и 16 нет вовсе.",
        "- Изъяны разметки: `11/text_msterms.txt` — копия `17/text_msterms.txt` (в подборе только",
        "  вторая); «Горячий камень» Гайдара размечен и как 1-й класс (`1/gaidar_kamen.txt`), и как",
        "  3-й (`3/gaidar_stone.txt`), оба в подборе, как и в старом коде.",
        "- Нужен новый размеченный корпус: без него точнее стать нельзя.",
        "",
    ]
    path.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
