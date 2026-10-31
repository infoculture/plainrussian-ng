"""Проверка MCP-сервера через клиент SDK внутри процесса."""

import pytest

pytest.importorskip("mcp")

from mcp import Client  # noqa: E402

from plainrussian.mcp_server import server  # noqa: E402

TEXT = (
    "В целях обеспечения реализации мероприятий по повышению эффективности деятельности органов "
    "исполнительной власти субъектов Российской Федерации в сфере предоставления государственных "
    "и муниципальных услуг осуществляется проведение мониторинга качества и доступности указанных услуг. "
    "Это просто."
)


def structured(result):
    assert not result.is_error, result
    return result.structured_content


@pytest.mark.anyio
async def test_tools_are_listed_as_read_only():
    async with Client(server) as client:
        tools = {t.name: t for t in (await client.list_tools()).tools}
    assert set(tools) == {"measure_readability", "find_hard_parts", "check_plain_language"}
    assert all(t.annotations.read_only_hint for t in tools.values())


@pytest.mark.anyio
async def test_measure_readability():
    async with Client(server) as client:
        data = structured(await client.call_tool("measure_readability", {"text": TEXT}))
    assert data["counts"]["sentences"] == 2
    assert 12 < data["grade_years"] <= 17
    assert data["grade_years_raw"] >= data["grade_years"]
    assert set(data["formulas"]) == {"Flesch-Kincaid", "Coleman-Liau", "SMOG", "Dale-Chale", "ARI"}
    assert data["calibration"] == "2026.1"


@pytest.mark.anyio
async def test_find_hard_parts():
    async with Client(server) as client:
        data = structured(await client.call_tool("find_hard_parts", {"text": TEXT, "max_items": 3}))
    assert data["long_sentences_total"] == 1
    assert data["long_sentences"][0]["words"] >= 20
    assert len(data["complex_words"]) == 3
    assert all(w["syllables"] > 4 and w["count"] >= 1 for w in data["complex_words"])


@pytest.mark.anyio
async def test_empty_text():
    async with Client(server) as client:
        data = structured(await client.call_tool("measure_readability", {"text": "..."}))
    assert "grade_years" not in data
    assert data["note"]


@pytest.mark.anyio
async def test_check_plain_language():
    async with Client(server) as client:
        data = structured(await client.call_tool("check_plain_language", {"text": TEXT}))
    kinds = {k["kind"]: k for k in data["kinds"]}
    assert kinds["genitive_chain"]["count"] >= 1
    assert "эффективности деятельности органов исполнительной власти" in kinds["genitive_chain"]["examples"][1]
    assert kinds["bureaucratese"]["examples"][0] == "В целях"
    assert data["per_100_words"] > 10


@pytest.fixture
def anyio_backend():
    return "asyncio"
