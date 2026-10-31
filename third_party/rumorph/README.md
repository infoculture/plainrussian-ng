# rumorph

Проверки ясного языка (`crates/pr-core/src/clarity.rs`) используют морфологический
анализатор [rumorph](https://github.com/SergeyShk/ruMorph) (код — MIT, `LICENSE.txt`) и его
словарь из крейта `rumorph-dict-ru` (CC BY-SA 4.0, `LICENSE-DICTIONARY.txt`).

Словарь распространяется вместе с программами: он встроен в `plainrussian` (CLI) и в
Python-пакет, а сайт и настольное приложение раздают его файлом `dict/ru.dict.gz`.
Скрипт `apps/web/scripts/build-wasm.sh` берёт его из исходников крейта; в репозитории
файла нет.
