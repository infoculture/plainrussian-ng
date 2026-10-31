#!/usr/bin/env bash
# Готовит подпись настольного приложения в release.yml по секретам и переменным
# репозитория. Без них macOS-приложение получает ad-hoc подпись (tauri.conf.json),
# а установщик для Windows остаётся неподписанным.
#
# Tauri проверяет переменные подписи на наличие, а не на непустое значение, поэтому
# незаданный секрет нельзя передавать пустой строкой: в окружение сборки попадают
# только заданные (через $GITHUB_ENV). Значения секретов GitHub скрывает в журнале.
#
# macOS, подпись сертификатом Developer ID:
#   APPLE_CERTIFICATE (.p12 в base64), APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY
# macOS, нотаризация (один из двух наборов, только вместе с сертификатом):
#   APPLE_ID, APPLE_PASSWORD (пароль приложения), APPLE_TEAM_ID
#   APPLE_API_KEY (ID ключа), APPLE_API_ISSUER, APPLE_API_PRIVATE_KEY (содержимое .p8)
# Windows (переменные репозитория, не секреты):
#   WINDOWS_SIGN_COMMAND — команда подписи с %1 на месте файла (signCommand Tauri);
#   WINDOWS_SIGN_INSTALL — необязательная команда, ставящая инструмент подписи;
#   секреты AZURE_CLIENT_ID, AZURE_TENANT_ID, AZURE_CLIENT_SECRET передаются, если заданы.
#
# Пишет в $GITHUB_OUTPUT `args` — дополнительные аргументы для `tauri build`.
set -euo pipefail

fail() {
  echo "::error::$1"
  exit 1
}

# Передаёт переменную в следующие шаги, только если она задана.
export_env() {
  local name=$1 value=${!1:-}
  [ -n "$value" ] || return 0
  local delimiter="EOF_$RANDOM$RANDOM"
  {
    echo "$name<<$delimiter"
    echo "$value"
    echo "$delimiter"
  } >>"$GITHUB_ENV"
}

args=''

case "${RUNNER_OS:-}" in
macOS)
  if [ -z "${APPLE_CERTIFICATE:-}" ]; then
    if [ -n "${APPLE_ID:-}${APPLE_API_KEY:-}" ]; then
      fail 'Для нотаризации нужен сертификат: задайте APPLE_CERTIFICATE и APPLE_CERTIFICATE_PASSWORD.'
    fi
    echo 'macOS: ad-hoc подпись, без нотаризации.'
  else
    [ -n "${APPLE_CERTIFICATE_PASSWORD:-}" ] || fail 'Задан APPLE_CERTIFICATE, но нет APPLE_CERTIFICATE_PASSWORD.'
    # Без него Tauri сверял бы сертификат с ad-hoc подписью «-» из tauri.conf.json.
    [ -n "${APPLE_SIGNING_IDENTITY:-}" ] || fail 'Задан APPLE_CERTIFICATE, но нет APPLE_SIGNING_IDENTITY.'
    export_env APPLE_CERTIFICATE
    export_env APPLE_CERTIFICATE_PASSWORD
    export_env APPLE_SIGNING_IDENTITY
    if [ -n "${APPLE_ID:-}" ]; then
      [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] ||
        fail 'Для нотаризации по Apple ID нужны APPLE_ID, APPLE_PASSWORD и APPLE_TEAM_ID.'
      export_env APPLE_ID
      export_env APPLE_PASSWORD
      export_env APPLE_TEAM_ID
      echo 'macOS: подпись Developer ID, нотаризация по Apple ID.'
    elif [ -n "${APPLE_API_KEY:-}" ]; then
      [ -n "${APPLE_API_ISSUER:-}" ] && [ -n "${APPLE_API_PRIVATE_KEY:-}" ] ||
        fail 'Для нотаризации по ключу API нужны APPLE_API_KEY, APPLE_API_ISSUER и APPLE_API_PRIVATE_KEY.'
      key_path="$RUNNER_TEMP/AuthKey_$APPLE_API_KEY.p8"
      printf '%s\n' "$APPLE_API_PRIVATE_KEY" >"$key_path"
      chmod 600 "$key_path"
      # shellcheck disable=SC2034 # читает export_env через ${!name}
      APPLE_API_KEY_PATH=$key_path
      export_env APPLE_API_KEY
      export_env APPLE_API_ISSUER
      export_env APPLE_API_KEY_PATH
      echo 'macOS: подпись Developer ID, нотаризация по ключу App Store Connect API.'
    else
      echo '::warning::macOS: подпись Developer ID без нотаризации — Gatekeeper всё равно предупредит.'
    fi
  fi
  ;;
Windows)
  if [ -z "${WINDOWS_SIGN_COMMAND:-}" ]; then
    echo 'Windows: установщик не подписан.'
  else
    [[ $WINDOWS_SIGN_COMMAND == *%1* ]] || fail 'В WINDOWS_SIGN_COMMAND нет %1 — места для пути к файлу.'
    if [ -n "${WINDOWS_SIGN_INSTALL:-}" ]; then
      echo 'Windows: ставлю инструмент подписи.'
      bash -c "$WINDOWS_SIGN_INSTALL"
    fi
    export_env AZURE_CLIENT_ID
    export_env AZURE_TENANT_ID
    export_env AZURE_CLIENT_SECRET
    # signCommand задаётся отдельным файлом конфигурации, который Tauri сливает с основным.
    # Путь с прямыми слешами: обратные tauri-action принял бы за экранирование.
    config="$(cygpath -m "$RUNNER_TEMP")/windows-sign.json"
    node -e 'require("fs").writeFileSync(process.argv[1],
      JSON.stringify({ bundle: { windows: { signCommand: process.argv[2] } } }))' \
      "$config" "$WINDOWS_SIGN_COMMAND"
    args="--config $config"
    echo 'Windows: установщик подписывается командой из WINDOWS_SIGN_COMMAND.'
  fi
  ;;
*)
  echo "${RUNNER_OS:-?}: подпись не нужна."
  ;;
esac

echo "args=$args" >>"$GITHUB_OUTPUT"
