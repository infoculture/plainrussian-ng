//! Ядро plainrussian: оценка простоты (читаемости) русскоязычных текстов.
//!
//! - [`readability`] — текущий алгоритм: предложения по razdel, новая калибровка формул.
//! - [`legacy`] — алгоритм старого plainrussian.ru один в один, для совместимости.
//! - [`sentenize`] — деление текста на предложения по правилам razdel.
//! - `clarity` (feature `clarity`) — проверки ясного языка с морфологией rumorph.

#[cfg(feature = "clarity")]
pub mod clarity;
pub mod legacy;
mod pychar;
pub mod readability;
pub mod sentenize;
