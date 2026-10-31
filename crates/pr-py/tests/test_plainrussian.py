"""Проверка Python-обёртки над pr-core."""

import plainrussian

TEXT = "Мама мыла раму. Папа читал газету. Высокопревосходительство приехало на заседание."


def test_measure():
    m = plainrussian.measure(TEXT)
    assert m["counts"]["words"] == 10
    assert m["counts"]["sentences"] == 3
    assert m["counts"]["complex_words"] == 2
    assert m["calibration"]["version"] == "2026.1"
    assert 0 < m["indexes"]["average"]["value"] < 30
    assert set(m["indexes"]) == {"average", "fk", "cl", "smog", "dc", "ari"}


def test_text_without_words():
    m = plainrussian.measure("123 ... 456")
    assert m["counts"]["words"] == 0
    assert m["indexes"] is None and m["features"] is None


def test_details_are_strings_even_with_emoji():
    d = plainrussian.details("Привет 👋 мир! Высокопревосходительство спит.")
    assert [s["text"] for s in d["sentences"]] == ["Привет 👋 мир!", "Высокопревосходительство спит."]
    assert d["complex_words"] == [{"text": "Высокопревосходительство", "syllables": 9}]
    assert d["measurement"]["counts"]["words"] == 4


def test_sentenize_keeps_abbreviations():
    assert plainrussian.sentenize("См. ст. 5 т.е. А.С. Пушкин писал... Что?! Да.") == [
        "См. ст. 5 т.е. А.С. Пушкин писал...",
        "Что?!",
        "Да.",
    ]


def test_measure_legacy_keeps_old_api_shape():
    m = plainrussian.measure_legacy(TEXT)
    assert set(m) == {"metrics", "indexes"}
    assert {"index_SMOG", "grade_SMOG", "index_fk"} <= set(m["indexes"])
    # Старый алгоритм считает каждую точку концом предложения и не видит заглавных.
    assert m["metrics"]["n_sentences"] == 3


def test_version():
    assert plainrussian.__version__ == "0.1.0"


def test_check_clarity():
    text = "В целях обеспечения реализации мероприятий осуществляется проведение мониторинга в МВД."
    r = plainrussian.check_clarity(text)
    assert r["counts"]["genitive_chain"] == 1
    assert r["counts"]["abbreviation"] == 1
    for f in r["findings"]:
        assert text[f["start"]:f["end"]] == f["text"]


def test_check_clarity_offsets_with_emoji():
    text = "👋 Данный документ."
    f = plainrussian.check_clarity(text)["findings"][0]
    assert text[f["start"]:f["end"]] == "Данный"

