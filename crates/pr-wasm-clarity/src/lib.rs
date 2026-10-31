//! Проверки ясного языка в браузере. Отдельный модуль: морфология добавляет к ядру
//! около 400 КБ, а нужна только тем, кто включил проверку, — вместе со словарём.

use pr_core::clarity::{self, Dictionary};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Clarity {
    dictionary: Dictionary,
}

#[wasm_bindgen]
impl Clarity {
    /// Словарь rumorph — байты файла `ru.dict` (уже распакованные).
    #[wasm_bindgen(constructor)]
    pub fn new(dictionary: &[u8]) -> Result<Clarity, JsError> {
        Dictionary::from_bytes(dictionary)
            .map(|dictionary| Clarity { dictionary })
            .map_err(|e| JsError::new(&format!("Словарь не читается: {e}")))
    }

    /// Находки в JSON: `{"words": ..., "counts": {...}, "findings": [...]}`.
    pub fn check(&self, text: &str) -> String {
        serde_json::to_string(&clarity::check(&self.dictionary, text))
            .expect("результат всегда сериализуется в JSON")
    }
}
