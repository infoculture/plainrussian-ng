//! Ключи API в системном хранилище: Keychain в macOS, Credential Manager в
//! Windows, Secret Service в Linux. В файлах и в хранилище WebView их нет.

const SERVICE: &str = "ru.plainrussian.desktop";

/// Имя записи — идентификатор провайдера из `providers.ts`.
fn entry(name: &str) -> Result<keyring::Entry, String> {
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
        return Err(format!("недопустимое имя ключа: {name:?}"));
    }
    keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn secret_get(name: String) -> Result<Option<String>, String> {
    match entry(&name)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn secret_set(name: String, value: String) -> Result<(), String> {
    entry(&name)?
        .set_password(&value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn secret_delete(name: String) -> Result<(), String> {
    match entry(&name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unexpected_names() {
        assert!(entry("").is_err());
        assert!(entry("../other").is_err());
        assert!(entry("Anthropic").is_err());
    }
}
