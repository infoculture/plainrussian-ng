# plainrussian-ng

Новая версия [plainrussian.ru](https://plainrussian.ru): оценка простоты (читаемости)
русскоязычных текстов. Расчёт идёт на стороне пользователя (WebAssembly в браузере,
нативно в настольном приложении), поэтому текст никуда не отправляется.

## Состояние

Фаза 0: алгоритм старого plainrussian.ru перенесён в Rust один в один.

- `crates/pr-core` — ядро. Модуль `legacy` повторяет `api/textmetric/metric.py`
  из старого репозитория: те же счётчики, пять формул (Flesch-Kincaid, Coleman-Liau,
  «Dale-Chale», SMOG, ARI) и формат ответа `/api/1.0/ru/measure/`.
- `crates/pr-wasm` — обёртка для браузера, функция `measureLegacy(text)` возвращает JSON.
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

## Лицензия

CC0 1.0, как у исходного проекта. Словари и корпуса, которые появятся позже,
могут иметь свои лицензии.
