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

Новый корпус (calibration/graded, см. build_corpus.py) подбирается с ключом --graded,
школьная шкала — так:
    python3 calibration/fit.py --version school-2026.1 --graded --balance

Вход — sources.csv, fragments.csv и features.jsonl из calibration/graded. Коэффициенты
подбираются на фрагментах с split=fit. Проверка — исключение по источникам: формула
подбирается без всех фрагментов источника и проверяется на них, иначе соседние
фрагменты одной книги занижают ошибку. Фрагменты с split=test в подборе не участвуют,
ошибка на них считается отдельно. Для сравнения на тех же фрагментах считается ошибка
прежней калибровки (--baseline). С ключом --balance каждый класс весит в подборе одинаково,
сколько бы фрагментов в нём ни было: иначе подбор подстраивается под самые частые классы.
Отчёт пишется в calibration/<версия>.md, REPORT.md старого корпуса не меняется.
"""

import argparse
import csv
import datetime
import json
import sys
from math import sqrt
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent
CEILING = 17
DIGITS = 4
# Действующая калибровка: с ней сравнивается подбор по новому корпусу.
BASELINE = "2026.1"

FORMULAS = {
    "fk": ("Flesch-Kincaid", ["слов в предложении", "слогов в слове"]),
    "cl": ("Coleman-Liau", ["букв на 100 слов", "предложений на 100 слов"]),
    "smog": ("SMOG", ["√(сложных слов на 30 предложений)"]),
    "dc": ("Dale-Chall (упрощённая)", ["сложных слов на 100 слов", "слов в предложении"]),
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


def fit_lad(X, y, iterations=500, weights=None):
    """Минимум средней абсолютной ошибки с учётом потолка (IRLS); weights — веса текстов."""
    w0 = np.ones(len(y)) if weights is None else weights
    theta = np.linalg.lstsq(X * np.sqrt(w0)[:, None], y * np.sqrt(w0), rcond=None)[0]
    for _ in range(iterations):
        pred = X @ theta
        active = ~((y >= CEILING) & (pred >= CEILING))
        w = np.where(active, w0 / np.maximum(np.abs(y - pred), 1e-6), 0.0)
        sw = np.sqrt(w)
        new = np.linalg.lstsq(X * sw[:, None], y * sw, rcond=None)[0]
        if np.allclose(new, theta, atol=1e-10):
            break
        theta = new
    return theta


def signed(y, pred):
    """Ошибка со знаком: плюс — формула завышает уровень."""
    diff = pred - y
    diff[(y >= CEILING) & (pred >= CEILING)] = 0.0
    return diff


def loo_predictions(X, y, groups, weights=None):
    """Прогноз для каждого текста формулой, подобранной без текстов его группы."""
    pred = np.empty_like(y)
    for group in np.unique(groups):
        held = groups == group
        w = None if weights is None else weights[~held]
        pred[held] = X[held] @ fit_lad(X[~held], y[~held], weights=w)
    return pred


def balanced(y):
    """Веса, при которых каждый уровень весит одинаково; в среднем вес равен 1."""
    levels, inverse, n = np.unique(y, return_inverse=True, return_counts=True)
    w = 1.0 / n[inverse]
    return w * len(y) / w.sum()


def summary(err):
    return {"mae": round(float(err.mean()), 3), "max": round(float(err.max()), 3),
            "within_2": round(float((err <= 2).mean()), 3)}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--graded", nargs="?", type=Path, const=ROOT / "graded", metavar="КАТАЛОГ",
                        help="подбор по новому корпусу; каталог с sources.csv, fragments.csv "
                             "и features.jsonl, по умолчанию calibration/graded")
    parser.add_argument("--baseline", default=BASELINE,
                        help="версия калибровки для сравнения на новом корпусе")
    parser.add_argument("--balance", action="store_true",
                        help="с --graded: каждый класс весит в подборе одинаково")
    parser.add_argument("--out", type=Path, default=ROOT, help="куда писать <версия>.json и отчёт")
    args = parser.parse_args(argv)
    if args.balance and not args.graded:
        parser.error("--balance работает только с --graded")

    result = fit_graded(args) if args.graded else fit_legacy(args)
    out = args.out / f"{args.version}.json"
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.graded:
        write_graded_report(result, args.out / f"{args.version}.md")
    else:
        write_report(result, args.out / "REPORT.md")
    print(json.dumps(result["formulas"], ensure_ascii=False, indent=1))
    print("average", result["average"])


def fit_legacy(args):
    with (ROOT / "corpus.csv").open(encoding="utf-8") as f:
        corpus = {r["path"]: r for r in csv.DictReader(f)}
    rows = [json.loads(line) for line in (ROOT / "features.jsonl").read_text(encoding="utf-8").splitlines()]
    rows = [r for r in rows if corpus[r["path"]]["fit"] == "1"]
    y = np.array([float(corpus[r["path"]]["grade"]) for r in rows])
    # Каждый текст — своя группа: проверка с исключением по одному.
    groups = np.arange(len(y))

    result = {"version": args.version, "created": datetime.date.today().isoformat(),
              "texts": len(rows), "ceiling": CEILING, "formulas": {}}
    preds = {"fit": [], "loo": [], "legacy": [], "legacy_refit_loo": []}
    for key, (title, names) in FORMULAS.items():
        X = np.array([features(r["counts"], key) for r in rows])
        coef = [round(float(v), DIGITS) for v in fit_lad(X, y)]
        X_old = np.array([features(r["legacy"], key) for r in rows])
        pred = {
            "fit": X @ np.array(coef),
            "loo": loo_predictions(X, y, groups),
            "legacy": X_old @ np.array(LEGACY_COEF[key]),
            "legacy_refit_loo": loo_predictions(X_old, y, groups),
        }
        result["formulas"][key] = {"title": title, "features": names, "coef": coef}
        for name, p in pred.items():
            preds[name].append(p)
            result["formulas"][key][name] = summary(errors(y, p))
    # Главная оценка нового алгоритма — среднее пяти формул.
    result["average"] = {"title": "Среднее пяти формул"}
    for name, p in preds.items():
        result["average"][name] = summary(errors(y, np.mean(p, axis=0)))
    return result


def load_graded(manifest):
    """Фрагменты нового корпуса: пары (счётчики, строка источника из sources.csv)."""
    for name in ("sources.csv", "fragments.csv", "features.jsonl"):
        if not (manifest / name).is_file():
            sys.exit(f"нет файла {manifest / name}: порядок сбора корпуса описан в build_corpus.py")
    with (manifest / "sources.csv").open(encoding="utf-8-sig", newline="") as f:
        sources = {r["source_id"]: r for r in csv.DictReader(f)}
    with (manifest / "fragments.csv").open(encoding="utf-8", newline="") as f:
        fragments = {r["path"]: r["source_id"] for r in csv.DictReader(f)}
    rows = [json.loads(line) for line in (manifest / "features.jsonl").read_text(encoding="utf-8").splitlines()]
    if {r["path"] for r in rows} != set(fragments):
        sys.exit("features.jsonl не совпадает с fragments.csv: пересчитайте признаки фрагментов")
    return [(r["counts"], sources[fragments[r["path"]]]) for r in rows]


def breakdown(keys, groups, y, preds):
    """Ошибка и смещение оценок по частям корпуса: уровням или типам источников."""
    rows = []
    for key in sorted(set(keys)):
        mask = np.array([k == key for k in keys])
        row = {"key": key, "sources": len(set(groups[mask])), "texts": int(mask.sum())}
        for name, pred in preds.items():
            diff = signed(y[mask], pred[mask])
            row[name] = {"mae": round(float(np.abs(diff).mean()), 3), "bias": round(float(diff.mean()), 3)}
        rows.append(row)
    return rows


def fit_graded(args):
    data = load_graded(args.graded)
    baseline = json.loads((ROOT / f"{args.baseline}.json").read_text(encoding="utf-8"))["formulas"]
    parts = {}
    for split in ("fit", "test"):
        chosen = [(counts, source) for counts, source in data if source["split"] == split]
        parts[split] = ([counts for counts, _ in chosen], [source for _, source in chosen],
                        np.array([float(source["grade"]) for _, source in chosen]))
    counts, sources, y = parts["fit"]
    test_counts, test_sources, test_y = parts["test"]
    groups = np.array([source["source_id"] for source in sources])
    if len(set(groups)) < 2:
        sys.exit("для подбора нужны фрагменты хотя бы двух источников с split=fit")
    weights = balanced(y) if args.balance else None

    result = {"version": args.version, "created": datetime.date.today().isoformat(),
              "corpus": "graded", "texts": len(y), "sources": len(set(groups)), "balanced": args.balance,
              "test_texts": len(test_y), "ceiling": CEILING, "baseline": args.baseline, "formulas": {}}
    preds = {"fit": [], "loo": [], "baseline": []}
    test_preds = {"test": [], "baseline_test": []}
    for key, (title, names) in FORMULAS.items():
        X = np.array([features(c, key) for c in counts])
        coef = [round(float(v), DIGITS) for v in fit_lad(X, y, weights=weights)]
        old = baseline[key]["coef"]
        entry = {"title": title, "features": names, "coef": coef}
        pred = {"fit": X @ np.array(coef), "loo": loo_predictions(X, y, groups, weights),
                "baseline": X @ np.array(old)}
        for name, p in pred.items():
            preds[name].append(p)
            entry[name] = summary(errors(y, p))
        if len(test_y):
            X_test = np.array([features(c, key) for c in test_counts])
            for name, c in (("test", coef), ("baseline_test", old)):
                test_preds[name].append(X_test @ np.array(c))
                entry[name] = summary(errors(test_y, test_preds[name][-1]))
        result["formulas"][key] = entry
    # Главная оценка — среднее пяти формул.
    mean = {name: np.mean(p, axis=0) for name, p in preds.items()}
    result["average"] = {"title": "Среднее пяти формул"}
    for name, p in mean.items():
        result["average"][name] = summary(errors(y, p))
    if len(test_y):
        for name, p in test_preds.items():
            result["average"][name] = summary(errors(test_y, np.mean(p, axis=0)))
    checked = {"loo": mean["loo"], "baseline": mean["baseline"]}
    result["levels"] = breakdown([int(v) for v in y], groups, y, checked)
    result["source_types"] = breakdown([s["source_type"] for s in sources], groups, y, checked)
    result["genres"] = breakdown([s["genre"] for s in sources], groups, y, checked)
    if len(test_y):
        held = {"loo": np.mean(test_preds["test"], axis=0), "baseline": np.mean(test_preds["baseline_test"], axis=0)}
        test_groups = np.array([s["source_id"] for s in test_sources])
        result["test_groups"] = breakdown([f"{s['source_type']}, {s['grade']} кл." for s in test_sources],
                                          test_groups, test_y, held)
    return result


def write_graded_report(r, path):
    f = r["formulas"]
    test = r["test_texts"] > 0
    lines = [
        f"# Калибровка {r['version']}",
        "",
        f"Создана {r['created']} скриптом `calibration/fit.py --graded` на {r['texts']} фрагментах",
        f"из {r['sources']} источников нового корпуса (`calibration/graded`, split=fit).",
        f"Ошибка — разница с уровнем источника в годах обучения; уровень {r['ceiling']} означает",
        f"«{r['ceiling']} и выше», прогноз от {r['ceiling']} для такого текста считается верным.",
        "",
        "## Средняя ошибка, лет обучения",
        "",
        "Проверка — исключение по источникам: формула подбирается без всех фрагментов источника",
        "и проверяется на них. Это честная оценка на новых текстах.",
        f"Калибровка {r['baseline']} — прежние коэффициенты на тех же фрагментах.",
    ]
    if r.get("balanced"):
        lines.append("Каждый класс весил в подборе одинаково (`--balance`); ошибки — без весов, по фрагментам.")
    if test:
        lines.append(f"Отложенные — {r['test_texts']} фрагментов с split=test, в подборе их нет.")
    head = [f"Калибровка {r['baseline']}", "Новая, проверка", "Новая, на обучении", "Новая, макс. в проверке"]
    names = ["baseline", "loo", "fit"]
    if test:
        head += [f"Отложенные, {r['baseline']}", "Отложенные, новая"]
    lines += ["", "| Формула | " + " | ".join(head) + " |", "|---|" + "---:|" * len(head)]
    for v in [*f.values(), r["average"]]:
        cells = [v[name]["mae"] for name in names] + [v["loo"]["max"]]
        if test:
            cells += [v["baseline_test"]["mae"], v["test"]["mae"]]
        lines.append(f"| {v['title']} | " + " | ".join(str(c) for c in cells) + " |")
    tables = [("По уровням", "Уровень", r["levels"], "в проверке по источникам"),
              ("По типам источников", "Тип", r["source_types"], "в проверке по источникам"),
              ("По жанрам", "Жанр", r.get("genres", []), "в проверке по источникам")]
    if r.get("test_groups"):
        tables.append(("Отложенные тексты", "Тип и класс", r["test_groups"], "на отложенных текстах"))
    for title, first, rows, where in tables:
        lines += [
            "",
            f"## {title}",
            "",
            f"Среднее пяти формул {where}. Смещение — средняя разница оценки",
            "и уровня: плюс значит, что оценка завышена.",
            "",
            f"| {first} | Источников | Фрагментов | Ошибка {r['baseline']} | Смещение {r['baseline']} "
            "| Ошибка новой | Смещение новой |",
            "|---|---:|---:|---:|---:|---:|---:|",
        ]
        for row in rows:
            lines.append(f"| {row['key']} | {row['sources']} | {row['texts']} | {row['baseline']['mae']} | "
                         f"{row['baseline']['bias']:+} | {row['loo']['mae']} | {row['loo']['bias']:+} |")
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
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


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
