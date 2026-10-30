//! WebAssembly-обёртка над `pr-core`. Текст не покидает браузер.

use pr_core::legacy;
use wasm_bindgen::prelude::*;

/// Оценка по алгоритму старого plainrussian.ru. Возвращает JSON в формате
/// ответа `/api/1.0/ru/measure/`: `{"metrics": {...}, "indexes": {...}}`.
#[wasm_bindgen(js_name = measureLegacy)]
pub fn measure_legacy(text: &str) -> String {
    serde_json::to_string(&legacy::measure(text)).expect("Measurement всегда сериализуется в JSON")
}
