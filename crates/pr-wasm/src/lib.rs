//! WebAssembly-обёртка над `pr-core`. Текст не покидает браузер.
//!
//! Результаты отдаются строкой JSON: `JSON.parse` на стороне JS быстрее,
//! чем поштучное создание объектов через границу WASM.

use pr_core::{legacy, readability};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Оценка по текущему алгоритму и разметка для подсветки:
/// `{"measurement": {...}, "annotation": {"words": [...], "sentences": [...]}}`.
/// В `measurement.calibration` — версия калибровки и её коэффициенты.
#[wasm_bindgen]
pub fn analyze(text: &str) -> String {
    to_json(&readability::analyze(text))
}

/// Оценка по алгоритму старого plainrussian.ru. Возвращает JSON в формате
/// ответа `/api/1.0/ru/measure/`: `{"metrics": {...}, "indexes": {...}}`.
#[wasm_bindgen(js_name = measureLegacy)]
pub fn measure_legacy(text: &str) -> String {
    to_json(&legacy::measure(text))
}

#[derive(Serialize)]
struct Analysis {
    measurement: legacy::Measurement,
    annotation: legacy::Annotation,
}

/// Оценка и разметка для подсветки:
/// `{"measurement": {...}, "annotation": {"words": [...], "sentences": [...]}}`.
/// Смещения в разметке — в единицах UTF-16, их можно передавать в `String.slice`.
#[wasm_bindgen(js_name = analyzeLegacy)]
pub fn analyze_legacy(text: &str) -> String {
    to_json(&Analysis {
        measurement: legacy::measure(text),
        annotation: legacy::annotate(text),
    })
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("результат всегда сериализуется в JSON")
}
