//! Классы символов и строковые проверки так, как их понимает Python 3.
//!
//! Нужны, чтобы перенос Python-кода (старый алгоритм, razdel) вёл себя так же.
//! Для русского и латиницы совпадение точное. Расхождения возможны на редких
//! символах: `\d` здесь — только ASCII и полноширинные цифры, а буквами
//! считается Unicode-свойство Alphabetic, а не категории L*.

/// `\s` и `str.isspace()`: Unicode White_Space плюс разделители `\x1c`–`\x1f`.
pub(crate) fn is_space(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '\x1c'..='\x1f')
}

/// `\d`: десятичная цифра.
pub(crate) fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, '０'..='９')
}

/// `\w`: буква, цифра или подчёркивание.
pub(crate) fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// `[^\W\d]`: символ слова, но не цифра.
pub(crate) fn is_letter_like(ch: char) -> bool {
    is_word(ch) && !is_digit(ch)
}

/// `str.isalpha()`.
pub(crate) fn str_is_alpha(s: &[char]) -> bool {
    !s.is_empty() && s.iter().all(|c| c.is_alphabetic())
}

/// `str.isdigit()`.
pub(crate) fn str_is_digit(s: &[char]) -> bool {
    !s.is_empty() && s.iter().all(|&c| is_digit(c))
}

/// `str.islower()`: есть буквы с регистром, и все они строчные.
pub(crate) fn str_is_lower(s: &[char]) -> bool {
    s.iter().any(|c| c.is_lowercase()) && !s.iter().any(|c| c.is_uppercase())
}

/// `str.isupper()`: есть буквы с регистром, и все они заглавные.
pub(crate) fn str_is_upper(s: &[char]) -> bool {
    s.iter().any(|c| c.is_uppercase()) && !s.iter().any(|c| c.is_lowercase())
}

/// `str.lower()`.
pub(crate) fn str_lower(s: &[char]) -> String {
    s.iter().flat_map(|c| c.to_lowercase()).collect()
}
