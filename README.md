# plainrussian-ng

Новая версия [plainrussian.ru](https://plainrussian.ru): оценка простоты (читаемости)
русскоязычных текстов. Расчёт идёт на стороне пользователя (WebAssembly в браузере,
нативно в настольном приложении), поэтому текст никуда не отправляется.

## Состояние

Фаза 1: веб-приложение на ядре, перенесённом из старого plainrussian.ru один в один.

- `crates/pr-core` — ядро. Модуль `legacy` повторяет `api/textmetric/metric.py`
  из старого репозитория: те же счётчики, пять формул (Flesch-Kincaid, Coleman-Liau,
  «Dale-Chale», SMOG, ARI) и формат ответа `/api/1.0/ru/measure/`. Функция
  `annotate` размечает слова и предложения для подсветки по тем же правилам.
- `crates/pr-wasm` — обёртка для браузера: `measureLegacy(text)` и `analyzeLegacy(text)`
  возвращают JSON.
- `apps/web` — веб-приложение (Vite, Svelte 5, TypeScript): оценка, подсветка сложных
  слов и длинных предложений, импорт txt, md, html, docx и pdf, работа офлайн.
- `tools/legacy-golden` — генератор эталонных результатов из исходного Python-кода.

Ошибки старого подсчёта сохранены намеренно: заглавные буквы не считаются,
каждый символ `.`, `?`, `!` закрывает предложение. Коэффициенты формул подобраны
под такой подсчёт, поэтому исправленный алгоритм появится отдельно, с новой калибровкой.

## Проверка

```bash
cargo test --workspace
```

Golden-тесты сравнивают результат Rust с Python без допуска. Граничные случаи
хранятся в `crates/pr-core/tests/golden/legacy_edge_cases.json`.

Тексты корпуса защищены авторским правом и в репозиторий не входят: в
`legacy_corpus.json` есть только путь, SHA-256 и ожидаемый результат. Тест ищет
тексты в соседнем репозитории `../plainrussian/api/textmetric/textsbygrade`
или в каталоге из переменной `PLAINRUSSIAN_LEGACY_CORPUS`. Если корпус не найден,
тест пропускается с сообщением.

Пересоздать эталоны (нужен соседний репозиторий `plainrussian`):

```bash
python3 -I tools/legacy-golden/generate.py
```

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

CC0 1.0, как у исходного проекта. Словари и корпуса, которые появятся позже,
могут иметь свои лицензии.
