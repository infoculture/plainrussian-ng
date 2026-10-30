# plainrussian-ng

Новая версия [plainrussian.ru](https://plainrussian.ru): оценка простоты (читаемости)
русскоязычных текстов. Расчёт идёт на стороне пользователя (WebAssembly в браузере,
нативно в настольном приложении), поэтому текст никуда не отправляется.

## Состояние

Фаза 2: новый алгоритм с перекалибровкой; старый доступен для сравнения.

- `crates/pr-core` — ядро:
  - `readability` — текущий алгоритм: предложения по правилам razdel, слова с дефисами,
    слоги по гласным любого регистра, пять формул с новыми коэффициентами и их среднее
    как главная оценка; версия калибровки входит в каждый результат;
  - `sentenize` — перенос `razdel.sentenize` на Rust (лицензия MIT, см. `third_party/razdel`);
  - `legacy` — старый plainrussian.ru один в один: те же счётчики, формулы и формат ответа
    `/api/1.0/ru/measure/`, вместе с ошибками подсчёта.
- `crates/pr-wasm` — обёртка для браузера: `analyze`, `analyzeLegacy`, `measureLegacy`
  возвращают JSON.
- `apps/web` — веб-приложение (Vite, Svelte 5, TypeScript): оценка, подсветка сложных
  слов и длинных предложений, выбор алгоритма, импорт txt, md, html, docx и pdf, офлайн.
- `calibration` — корпус с экспертными оценками, признаки текстов, подбор коэффициентов
  и отчёт `REPORT.md` с ошибками формул.
- `tools` — генераторы эталонов из Python-оригиналов (старый `metric.py` и razdel).

## Проверка

```bash
cargo test --workspace
```

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

## Лицензия

CC0 1.0, как у исходного проекта. Исключение — перенос razdel в
`crates/pr-core/src/sentenize.rs`: он распространяется по лицензии MIT, её текст в
`third_party/razdel/LICENSE`.
