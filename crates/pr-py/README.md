# plainrussian

Оценка простоты (читаемости) русскоязычных текстов: сколько лет обучения нужно,
чтобы понять текст, и что мешает пониманию. Ядро на Rust — то же, что на сайте
и в настольном приложении «Простой русский», поэтому оценки совпадают.
Текст обрабатывается локально.

```python
import plainrussian

m = plainrussian.measure(text)
m["indexes"]["average"]["value"]   # главная оценка, лет обучения
m["indexes"]["average"]["grade"]   # уровень словами
m["counts"]                         # предложения, слова, слоги, сложные слова, числа

d = plainrussian.details(text)
d["sentences"]       # [{"text": ..., "words": ...}, ...]
d["complex_words"]   # [{"text": ..., "syllables": ...}, ...]

plainrussian.check_clarity(text)    # канцелярит: цепочки родительных, отглагольные
                                    # существительные, канцеляризмы, пассив, аббревиатуры
plainrussian.sentenize(text)        # предложения по правилам razdel
plainrussian.measure_legacy(text)   # ответ в формате старого API plainrussian.ru
```

## MCP-сервер

```bash
pip install "plainrussian[mcp]"
```

Инструменты `measure_readability` (оценка и показатели), `find_hard_parts`
(длинные предложения и сложные слова) и `check_plain_language` (приёмы канцелярита
с примерами и советами). Для Claude Desktop:

```json
{
  "mcpServers": {
    "plainrussian": { "command": "plainrussian-mcp" }
  }
}
```

`plainrussian-mcp --http 127.0.0.1:9000` запускает сервер по Streamable HTTP.

Лицензия CC0 1.0; деление на предложения перенесено из razdel (MIT). Для проверок
ясного языка в пакет встроен морфологический словарь rumorph (OpenCorpora и Universal
Dependencies, CC BY-SA 4.0).
