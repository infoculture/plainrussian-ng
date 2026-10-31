# plainrussian-ng

Новая версия [plainrussian.ru](https://plainrussian.ru): оценка простоты (читаемости)
русскоязычных текстов. Расчёт идёт на стороне пользователя (WebAssembly в браузере,
нативно в настольном приложении), поэтому текст никуда не отправляется.

## Состояние

Фаза 6: проверки ясного языка с морфологическим анализом — на сайте, в приложении,
командной строке, Python и MCP.

- `crates/pr-core` — ядро:
  - `readability` — текущий алгоритм: предложения по правилам razdel, слова с дефисами,
    слоги по гласным любого регистра, пять формул с новыми коэффициентами и их среднее
    как главная оценка; версия калибровки входит в каждый результат;
  - `sentenize` — перенос `razdel.sentenize` на Rust (лицензия MIT, см. `third_party/razdel`);
  - `clarity` (feature `clarity`) — проверки ясного языка на морфологии
    [rumorph](https://github.com/SergeyShk/ruMorph) (см. `third_party/rumorph`);
  - `legacy` — старый plainrussian.ru один в один: те же счётчики, формулы и формат ответа
    `/api/1.0/ru/measure/`, вместе с ошибками подсчёта.
- `crates/pr-cli` — команда `plainrussian`: оценка файлов, каталогов и стандартного ввода,
  вывод сводкой, JSON или CSV.
- `crates/pr-py` — Python-пакет `plainrussian` (PyO3) с MCP-сервером `plainrussian-mcp`;
  заменяет старые `textmetric` и `plainrussian-mcp`, spaCy больше не нужен.
- `crates/pr-wasm` — обёртка для браузера: `analyze` (оценка и разметка для подсветки)
  и `measureLegacy` (ответ в формате старого API) возвращают JSON.
- `crates/pr-wasm-clarity` — отдельный модуль проверок ясного языка для браузера: загружается
  вместе со словарём, только когда проверку включают.
- `apps/web` — веб-приложение (Vite, Svelte 5, TypeScript): оценка, подсветка сложных
  слов и длинных предложений, импорт txt, md, html, docx и pdf, офлайн, оценка с помощью
  LLM (`src/lib/llm`, подключается на странице настроек), страница «О проекте».
- `apps/desktop` — настольное приложение (Tauri 2) с тем же интерфейсом: открытие страниц
  по ссылке, ключи API в системном хранилище, любой OpenAI-совместимый LLM-сервис.
- `eval/llm` — бенчмарк оценки LLM на размеченном корпусе.
- `calibration` — корпус с экспертными оценками, признаки текстов, подбор коэффициентов
  и отчёт `REPORT.md` с ошибками формул.
- `tools` — генераторы эталонов из Python-оригиналов (старый `metric.py` и razdel).

## Проверка

```bash
cargo test --workspace
```

Для сборки `crates/pr-py` нужен Python 3.9 или новее: pyo3 ищет его при компиляции.

На GitHub то же самое выполняет `.github/workflows/ci.yml`: Rust на Linux, macOS и Windows,
сборка сайта (артефакт `site`), Python-пакет с MCP-сервером и настольное приложение для Linux
(артефакт `desktop-linux` с `.deb` и `.AppImage`). Golden-тесты по корпусу там пропускаются:
текстов корпуса в репозитории нет.

Golden-тесты сравнивают Rust с Python-оригиналами без допуска: старый алгоритм —
с `metric.py`, деление на предложения — с razdel 0.5.0. Эталоны лежат в
`crates/pr-core/tests/golden`.

Тексты корпуса защищены авторским правом и в репозиторий не входят: в эталонах есть
только путь, SHA-256 и ожидаемый результат. Тесты ищут тексты в соседнем репозитории
`../plainrussian/api/textmetric/textsbygrade` или в каталоге из переменной
`PLAINRUSSIAN_LEGACY_CORPUS`. Если корпус не найден, такие тесты пропускаются с сообщением.

Пересоздать эталоны (нужен соседний репозиторий `plainrussian`, для razdel — venv
с `pip install razdel==0.5.0`):

```bash
python3 -I tools/legacy-golden/generate.py
```

```bash
path/to/venv/bin/python -I tools/razdel-golden/generate.py
```

## Проверки ясного языка

Помимо оценки сервис находит приёмы, которые утяжеляют текст:

- цепочки из трёх и больше существительных в родительном падеже;
- отглагольные существительные («проведение проверки» вместо «проверить»);
- канцеляризмы: «в целях», «в рамках», «осуществлять», «являться», «мероприятие»;
- страдательные причастия («утверждённый министерством»);
- аббревиатуры (по словарю; набранные заглавными обычные слова не в счёт).

Части речи и падежи определяет rumorph со словарём из OpenCorpora и Universal Dependencies
(CC BY-SA 4.0); падеж выбирается с учётом соседних слов. Словарь — 16,5 МБ, 7,1 МБ в gzip.
В программах (CLI, Python) он встроен; сайт и настольное приложение раздают его файлом
`dict/ru.dict.gz`, который скачивается при включении проверки и дальше берётся из кеша
service worker. Скрипт `apps/web/scripts/build-wasm.sh` кладёт его в `apps/web/public/dict`
из исходников крейта; в git файла нет.

Проверки различают тексты разной сложности. Находок на 100 слов по корпусу калибровки
(`cargo run -p pr-core --features clarity --example clarity_stats -- <textsbygrade>`):

| Уровень | Отглагольные | Страдательный | Цепочки | Канцеляризмы | Аббревиатуры |
|---|---:|---:|---:|---:|---:|
| 1–4 класс | 0,36 | 0,64 | 0,01 | 0,02 | 0,00 |
| 5–9 класс | 0,95 | 0,80 | 0,04 | 0,02 | 0,02 |
| 10–12 | 1,45 | 1,43 | 0,03 | 0,19 | 0,20 |
| 15–17 (законы, госдокументы) | 8,72 | 3,26 | 1,17 | 2,24 | 0,58 |

## Командная строка

```bash
cargo install --path crates/pr-cli
```

```bash
plainrussian статья.txt
```

```bash
plainrussian --format csv тексты/ > оценки.csv
```

```bash
cat статья.txt | plainrussian --format json -
```

```bash
plainrussian --clarity статья.txt
```

В каталогах берутся все `.txt` и `.md`. Файлы не в UTF-8 читаются как windows-1251.
Сводка показывает оценку в пределах шкалы («меньше 1 года», «17 и больше лет»), в JSON
и CSV — сырые значения формул.

## Python и MCP

Пакет собирается [maturin](https://www.maturin.rs/) в одно колесо (abi3) для Python 3.9+:

```bash
cd crates/pr-py && maturin develop --release
```

```python
import plainrussian
plainrussian.measure(text)["indexes"]["average"]   # оценка, лет обучения, и уровень
plainrussian.details(text)["complex_words"]          # сложные слова строками
plainrussian.check_clarity(text)["findings"]        # находки проверок ясного языка
plainrussian.sentenize(text)                         # предложения по правилам razdel
plainrussian.measure_legacy(text)                    # формат старого API
```

MCP-сервер ставится с дополнением `mcp` (`pip install "plainrussian[mcp]"`) и даёт
инструменты `measure_readability`, `find_hard_parts` и `check_plain_language`. Запуск по stdio — `plainrussian-mcp`,
по Streamable HTTP — `plainrussian-mcp --http 127.0.0.1:9000`. Подробнее в
`crates/pr-py/README.md`.

Тесты пакета и сервера:

```bash
cd crates/pr-py && pytest tests
```

## Калибровка

Коэффициенты подбираются на текстах из `calibration/corpus.csv` (колонка `fit`) по их
счётчикам в `calibration/features.jsonl`, поэтому сами тексты для подбора не нужны.
Порядок, если меняется подсчёт:

1. Пересчитать признаки по корпусу:

```bash
cargo run -p pr-core --example corpus_features -- ../plainrussian/api/textmetric/textsbygrade > calibration/features.jsonl
```

2. Подобрать коэффициенты под новой версией (нужен numpy):

```bash
python3 calibration/fit.py --version 2026.2
```

3. Перенести коэффициенты и `average_error` из `calibration/2026.2.json` в `CALIBRATION`
   в `crates/pr-core/src/readability.rs`. Тест `calibration_matches_json` проверяет,
   что они совпадают с файлом.

## Сборка WebAssembly

Нужен rustup с целью `wasm32-unknown-unknown`:

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo build -p pr-wasm --target wasm32-unknown-unknown --release
```

Если `cargo` на `PATH` установлен не через rustup (например, Homebrew), он не видит
цель wasm32. Тогда запускайте cargo из `$(dirname "$(rustup which rustc)")`.

## Веб-приложение

Нужны Node.js и `wasm-bindgen-cli` той же версии, что крейт `wasm-bindgen` в
`crates/pr-wasm/Cargo.toml`:

```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

```bash
cd apps/web && npm install
```

```bash
npm run dev
```

`npm run build` собирает ядро, проверяет типы и кладёт статический сайт в
`apps/web/dist`. Его можно разместить на любом статическом хостинге в любом каталоге.

Устройство:
- ядро работает в Web Worker, текст не покидает браузер;
- разборщики docx (mammoth) и pdf (pdf.js) загружаются только при открытии такого файла;
- service worker кеширует всё приложение, после первой загрузки оно работает без сети;
- в сборке действует Content Security Policy: только свои скрипты, никаких внешних
  запросов, никакой аналитики.

## Настольное приложение

Оболочка Tauri 2 над тем же веб-интерфейсом. Оценку считает то же ядро на WebAssembly,
поэтому результаты совпадают с сайтом. Rust-часть (`apps/desktop/src-tauri`) добавляет то,
чего у страницы в браузере нет:

- **открыть по ссылке**: страница загружается без ограничений CORS, кодировка
  определяется по заголовку, `<meta>` или содержимому (старые сайты в windows-1251),
  основной текст выделяет Readability, как в режиме чтения Firefox;
- **ключи API в системном хранилище**: Keychain в macOS, Credential Manager в Windows,
  Secret Service в Linux, а не localStorage;
- **любой OpenAI-совместимый LLM-сервис**: запросы делает Rust, поэтому CORS и CSP не мешают;
  Ollama работает без настройки `OLLAMA_ORIGINS`.

Нужны rustup, Node.js, `wasm-bindgen-cli` (см. «Веб-приложение») и системные зависимости
Tauri для вашей ОС: https://tauri.app/start/prerequisites/

```bash
cd apps/desktop && npm install
```

```bash
npm run dev
```

`npm run build` собирает установщики для текущей ОС: `.app` и `.dmg` в macOS, `.msi` и
`.exe` в Windows, `.deb`, `.rpm` и `.AppImage` в Linux. Подпись приложений пока не настроена,
поэтому macOS и Windows предупредят о неизвестном разработчике.

Веб-часть для приложения собирается в режиме `desktop` (`npm run build:desktop` в `apps/web`):
без service worker и без meta-CSP — политику задаёт `tauri.conf.json`, и она строже
сайтовой: напрямую из окна можно обращаться только к API Anthropic.

## Оценка с помощью LLM

Языковая модель оценивает текст целиком по той же шкале лет обучения, перечисляет
главные препятствия для понимания с цитатами и советует, как упростить. Ключ API
принадлежит пользователю; запрос идёт из браузера напрямую провайдеру, без нашего сервера.

| Провайдер | Как подключается | Текст уходит |
|---|---|---|
| Anthropic (Claude) | `@anthropic-ai/sdk`, структурированный вывод по схеме | в Anthropic |
| OpenAI, OpenRouter | OpenAI-совместимый API, `response_format: json_schema` | провайдеру |
| Ollama, LM Studio | OpenAI-совместимый API на `localhost` | никуда |

- Для моделей Claude, у которых он есть, включён `fallbacks: "default"`: если модель
  откажется оценивать текст по соображениям безопасности, API само повторит запрос на
  рекомендованной модели.
- На сайте CSP разрешает соединения только с адресами из таблицы и с `localhost`.
  Любой другой OpenAI-совместимый сервис доступен в настольном приложении. GigaChat
  (сертификат Минцифры, своя авторизация) пока не поддерживается.
- Ключ хранится в браузере, только если отмечено «Запомнить ключ».
- Чтобы опубликованный сайт мог обращаться к Ollama, добавьте его адрес в переменную
  `OLLAMA_ORIGINS`; для `localhost` это не нужно.
- LM Studio по умолчанию не отвечает браузеру заголовками CORS, даже на `localhost`:
  включите Enable CORS в настройках сервера (вкладка Developer) или запустите его
  командой `lms server start --cors`. Настольному приложению это не нужно.
- Цитаты модели сверяются с текстом: если модель пересказала вместо цитаты, это видно.

Бенчмарк прогоняет тот же промпт по 55 текстам калибровки и сравнивает ошибку модели
с ошибкой формул. Облачный провайдер тратит деньги, поэтому без `--yes` скрипт только
показывает объём:

```bash
ANTHROPIC_API_KEY=... node eval/llm/benchmark.ts --provider anthropic --model claude-opus-5-5 --yes
```

```bash
node eval/llm/benchmark.ts --provider ollama --model <модель>
```

Результаты пишутся в `eval/llm/results/` и в git не попадают: в них цитаты из текстов
корпуса.

## Лицензия

CC0 1.0, как у исходного проекта. Исключение — перенос razdel в
`crates/pr-core/src/sentenize.rs`: он распространяется по лицензии MIT, её текст в
`third_party/razdel/LICENSE`. Морфологический анализатор rumorph — MIT, его словарь —
CC BY-SA 4.0 (`third_party/rumorph`).
