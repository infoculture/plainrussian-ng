//! Деление текста на предложения: перенос `razdel.sentenize` 0.5.0
//! (<https://github.com/natasha/razdel>, лицензия MIT, текст в `third_party/razdel`).
//!
//! Алгоритм: найти кандидатов в концы предложений (`.?!…;`, кавычки, закрывающие
//! скобки, смайлики), для каждого посмотреть по 10 символов слева и справа и
//! правилами решить, склеивать или резать. Правила, списки сокращений и размеры
//! окон повторяют оригинал вместе с его странностями. Совпадение с Python
//! проверяют golden-тесты в `tests/sentenize_golden.rs`.

use std::ops::Range;

use crate::pychar::{
    is_digit, is_letter_like, is_space, is_word, str_is_alpha, str_is_digit, str_is_lower,
    str_is_upper, str_lower,
};

const ENDINGS: &str = ".?!…";
const DASHES: &str = "‑–—−-";
const CLOSE_QUOTES: &str = "»”’";
const GENERIC_QUOTES: &str = "\"„'";
const QUOTES: &str = "«“‘»”’\"„'";
const CLOSE_BRACKETS: &str = ")]}";
const DELIMITERS: &str = ".?!…;\"„'»”’)]}";
const BULLET_CHARS: &str = "§абвгдеabcdef";
const BULLET_BOUNDS: &str = ".)";
const BULLET_SIZE: usize = 20;
const WINDOW: usize = 10;

// Списки сокращений сгенерированы из `razdel.segmenters.sokr` (множества, отсортированы).
// Одно слово может быть сразу в нескольких списках, например «г» — и «год», и «город».

/// Точка после них — конец предложения, только если дальше не строчное слово и не число.
const TAIL_SOKRS: &[&str] = &[
    "al", "co", "corp", "ed", "inc", "барр", "в", "вв", "г", "га", "гг", "дес", "дол", "долл",
    "др", "изд", "кв", "км", "коп", "куб", "мин", "млн", "млрд", "проц", "р", "руб", "с", "сек",
    "см", "стр", "тыс", "час",
];

/// После этих сокращений предложение не заканчивается никогда.
const HEAD_SOKRS: &[&str] = &[
    "bd",
    "chap",
    "dr",
    "mr",
    "mrs",
    "ms",
    "no",
    "ps",
    "upd",
    "vs",
    "а",
    "абз",
    "акад",
    "англ",
    "араб",
    "арх",
    "ауд",
    "букв",
    "венг",
    "внутр",
    "г",
    "гл",
    "гор",
    "гос",
    "гр",
    "греч",
    "д",
    "дифф",
    "домовлад",
    "доп",
    "евр",
    "зав",
    "зам",
    "им",
    "исп",
    "итал",
    "к",
    "каб",
    "кат",
    "кит",
    "кн",
    "ком",
    "комн",
    "корп",
    "корр",
    "лат",
    "латв",
    "лит",
    "напр",
    "нач",
    "нем",
    "о",
    "обл",
    "обр",
    "оз",
    "ок",
    "откр",
    "отм",
    "оф",
    "п",
    "пер",
    "пл",
    "пол",
    "пом",
    "пос",
    "пп",
    "пр",
    "просп",
    "проф",
    "пс",
    "пт",
    "р",
    "ред",
    "рп",
    "рус",
    "русск",
    "с",
    "сб",
    "св",
    "слав",
    "словацк",
    "см",
    "ср",
    "ст",
    "стр",
    "т",
    "тел",
    "тов",
    "трад",
    "укр",
    "ул",
    "ум",
    "физ",
    "фр",
    "х",
    "хорв",
    "ч",
    "чл",
    "чч",
    "ш",
    "эт",
    "юр",
    "яп",
];

const OTHER_SOKRS: &[&str] = &["искл", "прим", "рис", "сокр", "устар", "шутл", "яз"];

const TAIL_PAIR_SOKRS: &[(&str, &str)] = &[
    ("a", "m"),
    ("p", "m"),
    ("з", "д"),
    ("л", "с"),
    ("н", "э"),
    ("р", "х"),
    ("с", "г"),
    ("с", "ш"),
    ("т", "д"),
    ("т", "п"),
    ("у", "е"),
    ("ч", "т"),
];

/// После этих парных сокращений («т. е.») предложение не заканчивается никогда.
const HEAD_PAIR_SOKRS: &[(&str, &str)] = &[
    ("и", "о"),
    ("к", "н"),
    ("к", "п"),
    ("к", "т"),
    ("л", "д"),
    ("п", "н"),
    ("т", "е"),
    ("т", "к"),
    ("т", "н"),
];

// В оригинале между 'жен р' и 'муж р' пропущена запятая: Python склеил строки
// в тройку, которая никогда не совпадает с парой. Эти две пары не действуют.
const OTHER_PAIR_SOKRS: &[(&str, &str)] = &[("ед", "ч"), ("мн", "ч"), ("повел", "накл")];

const INITIALS: &[&str] = &["ed", "вс", "дж"];

fn is_sokr_word(word: &str) -> bool {
    TAIL_SOKRS.contains(&word) || HEAD_SOKRS.contains(&word) || OTHER_SOKRS.contains(&word)
}

fn is_pair_sokr(pair: (&str, &str)) -> bool {
    TAIL_PAIR_SOKRS.contains(&pair)
        || HEAD_PAIR_SOKRS.contains(&pair)
        || OTHER_PAIR_SOKRS.contains(&pair)
}

/// Проверка `token in "..."` из Python: для строк это поиск подстроки.
fn token_in(token: &[char], set: &str) -> bool {
    let token: String = token.iter().collect();
    set.contains(token.as_str())
}

/// Делит текст на предложения. Возвращает байтовые диапазоны без пробелов по краям;
/// пустые предложения отбрасываются.
pub fn sentenize(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<char> = text.chars().collect();
    let mut byte_at = Vec::with_capacity(chars.len() + 1);
    byte_at.extend(text.char_indices().map(|(i, _)| i));
    byte_at.push(text.len());
    sentenize_chars(&chars)
        .into_iter()
        .map(|r| byte_at[r.start]..byte_at[r.end])
        .collect()
}

/// То же, что [`sentenize`], но в индексах символов.
pub(crate) fn sentenize_chars(chars: &[char]) -> Vec<Range<usize>> {
    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    for delimiter in find_delimiters(chars) {
        let split = Split {
            left: &chars[delimiter.start.saturating_sub(WINDOW)..delimiter.start],
            delimiter: &chars[delimiter.clone()],
            right: &chars[delimiter.end..(delimiter.end + WINDOW).min(chars.len())],
            buffer: &chars[chunk_start..delimiter.start],
        };
        if !split.join() {
            chunks.push(chunk_start..delimiter.end);
            chunk_start = delimiter.end;
        }
    }
    chunks.push(chunk_start..chars.len());

    chunks
        .into_iter()
        .filter_map(|r| {
            let mut start = r.start;
            let mut end = r.end;
            while start < end && is_space(chars[start]) {
                start += 1;
            }
            while end > start && is_space(chars[end - 1]) {
                end -= 1;
            }
            (start < end).then_some(start..end)
        })
        .collect()
}

/// Кандидаты в концы предложений: `([=:;]-?[)(]{1,3}|[DELIMITERS])`.
fn find_delimiters(chars: &[char]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(len) = smile_len(&chars[i..]) {
            found.push(i..i + len);
            i += len;
        } else if DELIMITERS.contains(chars[i]) {
            found.push(i..i + 1);
            i += 1;
        } else {
            i += 1;
        }
    }
    found
}

/// Длина смайлика `[=:;]-?[)(]{1,3}` в начале строки.
fn smile_len(s: &[char]) -> Option<usize> {
    if !matches!(s.first(), Some('=' | ':' | ';')) {
        return None;
    }
    let mut i = 1;
    if s.get(i) == Some(&'-') {
        i += 1;
    }
    let brackets = s[i.min(s.len())..]
        .iter()
        .take(3)
        .take_while(|&&c| c == ')' || c == '(')
        .count();
    (brackets > 0).then_some(i + brackets)
}

/// Длина токена `[^\W\d]+|\d+|[^\w\s]`, который начинается с `s[0]`.
fn token_len_forward(s: &[char]) -> usize {
    let first = s[0];
    if is_letter_like(first) {
        s.iter().take_while(|&&c| is_letter_like(c)).count()
    } else if is_digit(first) {
        s.iter().take_while(|&&c| is_digit(c)).count()
    } else {
        1
    }
}

/// Все токены строки (`TOKEN.findall`).
fn tokens(s: &[char]) -> Vec<&[char]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if is_space(s[i]) {
            i += 1;
            continue;
        }
        let len = token_len_forward(&s[i..]);
        out.push(&s[i..i + len]);
        i += len;
    }
    out
}

struct Split<'a> {
    left: &'a [char],
    delimiter: &'a [char],
    right: &'a [char],
    buffer: &'a [char],
}

impl<'a> Split<'a> {
    /// Первый токен справа (`^\s*(TOKEN)`).
    fn right_token(&self) -> Option<&'a [char]> {
        let start = self.right.iter().position(|&c| !is_space(c))?;
        let rest = &self.right[start..];
        Some(&rest[..token_len_forward(rest)])
    }

    /// Последний токен слева (`(TOKEN)\s*$`).
    fn left_token(&self) -> Option<&'a [char]> {
        let end = self.left.iter().rposition(|&c| !is_space(c))? + 1;
        let last = self.left[end - 1];
        let start = if is_letter_like(last) {
            end - self.left[..end]
                .iter()
                .rev()
                .take_while(|&&c| is_letter_like(c))
                .count()
        } else if is_digit(last) {
            end - self.left[..end]
                .iter()
                .rev()
                .take_while(|&&c| is_digit(c))
                .count()
        } else {
            end - 1
        };
        Some(&self.left[start..end])
    }

    /// Парное сокращение слева: `(\w)\s*\.\s*(\w)\s*$`, например «т. е».
    fn left_pair_sokr(&self) -> Option<(char, char)> {
        let mut visible = self.left.iter().rev().filter(|&&c| !is_space(c));
        let b = *visible.next()?;
        let dot = *visible.next()?;
        let a = *visible.next()?;
        (is_word(a) && dot == '.' && is_word(b)).then_some((a, b))
    }

    /// Первое слово или число справа (`[^\W\d]+|\d+`).
    fn right_word(&self) -> Option<&'a [char]> {
        let start = self.right.iter().position(|&c| is_word(c))?;
        let rest = &self.right[start..];
        Some(&rest[..token_len_forward(rest)])
    }

    fn delimiter_is(&self, ch: char) -> bool {
        self.delimiter == [ch]
    }

    fn delimiter_in(&self, set: &str) -> bool {
        token_in(self.delimiter, set)
    }

    /// Правила проверяются по порядку; первое сработавшее склеивает предложения.
    /// Если не сработало ни одно, текст режется.
    fn join(&self) -> bool {
        let (Some(left), Some(right)) = (self.left_token(), self.right_token()) else {
            return true; // empty_side
        };
        if !self.right.first().is_some_and(|&c| is_space(c)) {
            return true; // no_space_prefix
        }
        if is_lower_alpha(right) {
            return true; // lower_right
        }
        if self.delimiter_right(right)
            || self.sokr_left(left, right)
            || self.inside_pair_sokr(left, right)
            || self.initials_left(left)
            || self.list_item()
            || self.close_quote(left)
            || self.close_bracket(left)
            || self.dash_right(right)
        {
            return true;
        }
        false
    }

    fn delimiter_right(&self, right: &[char]) -> bool {
        if token_in(right, GENERIC_QUOTES) {
            return false;
        }
        if token_in(right, DELIMITERS) {
            return true;
        }
        let start = self.right.iter().take_while(|&&c| is_space(c)).count();
        smile_len(&self.right[start..]).is_some()
    }

    fn sokr_left(&self, left: &[char], right: &[char]) -> bool {
        if !self.delimiter_is('.') {
            return false;
        }
        if let Some((a, b)) = self.left_pair_sokr() {
            let pair = (str_lower(&[a]), str_lower(&[b]));
            let pair = (pair.0.as_str(), pair.1.as_str());
            if HEAD_PAIR_SOKRS.contains(&pair) {
                return true;
            }
            if is_pair_sokr(pair) {
                // В оригинале здесь `return`: дальше это правило не смотрит.
                return is_sokr(right);
            }
        }
        let left = str_lower(left);
        if HEAD_SOKRS.contains(&left.as_str()) {
            return true;
        }
        is_sokr_word(&left) && is_sokr(right)
    }

    fn inside_pair_sokr(&self, left: &[char], right: &[char]) -> bool {
        let (left, right) = (str_lower(left), str_lower(right));
        self.delimiter_is('.') && is_pair_sokr((left.as_str(), right.as_str()))
    }

    fn initials_left(&self, left: &[char]) -> bool {
        if !self.delimiter_is('.') {
            return false;
        }
        (str_is_upper(left) && left.len() == 1) || INITIALS.contains(&str_lower(left).as_str())
    }

    fn list_item(&self) -> bool {
        if !self.delimiter_in(BULLET_BOUNDS) || self.buffer.len() > BULLET_SIZE {
            return false;
        }
        tokens(self.buffer).into_iter().all(is_bullet)
    }

    fn close_quote(&self, left: &[char]) -> bool {
        if !self.delimiter_in(QUOTES) {
            return false;
        }
        if self.delimiter_in(CLOSE_QUOTES) {
            return close_bound(left);
        }
        if self.delimiter_in(GENERIC_QUOTES) {
            let left_space_suffix = self.left.last().is_some_and(|&c| is_space(c));
            return left_space_suffix || close_bound(left);
        }
        false
    }

    fn close_bracket(&self, left: &[char]) -> bool {
        self.delimiter_in(CLOSE_BRACKETS) && close_bound(left)
    }

    fn dash_right(&self, right: &[char]) -> bool {
        token_in(right, DASHES) && self.right_word().is_some_and(is_lower_alpha)
    }
}

fn is_lower_alpha(token: &[char]) -> bool {
    str_is_alpha(token) && str_is_lower(token)
}

/// Может ли токен после точки продолжать сокращение: число, знак или строчное слово.
fn is_sokr(token: &[char]) -> bool {
    if str_is_digit(token) {
        return true;
    }
    if !str_is_alpha(token) {
        return true;
    }
    str_is_lower(token)
}

fn close_bound(left: &[char]) -> bool {
    !token_in(left, ENDINGS)
}

/// Похож ли токен на маркер списка: «1», «IV», «а», «§», «.», «)».
fn is_bullet(token: &[char]) -> bool {
    str_is_digit(token)
        || token_in(token, BULLET_BOUNDS)
        || (token.len() == 1 && BULLET_CHARS.contains(str_lower(token).as_str()))
        || token
            .iter()
            .all(|c| matches!(c, 'I' | 'V' | 'X' | 'M' | 'L'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<&str> {
        sentenize(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn abbreviations_and_initials() {
        assert_eq!(
            split("См. ст. 5 т.е. А.С. Пушкин писал... Что?! Да."),
            ["См. ст. 5 т.е. А.С. Пушкин писал...", "Что?!", "Да."]
        );
        // «г» в razdel — ещё и «город» (HEAD_SOKRS), поэтому после «г.» не режется никогда.
        assert_eq!(
            split("Выручка выросла на 3.5% в 2024 г. И это всё."),
            ["Выручка выросла на 3.5% в 2024 г. И это всё."]
        );
    }

    #[test]
    fn empty_and_spaces() {
        assert!(split("").is_empty());
        assert!(split("   \n").is_empty());
        assert_eq!(split("  Привет.  "), ["Привет."]);
    }
}
