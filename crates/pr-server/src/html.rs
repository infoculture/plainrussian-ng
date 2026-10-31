//! Текст веб-страницы: основная часть (article, main) без меню, скриптов и подвала.
//! Абзацы разделяются пустой строкой, пункты списков и строки таблиц — переводом строки,
//! ячейки таблиц — пробелом.

use scraper::{ElementRef, Html, Node, Selector};

const SKIP: &[&str] = &[
    "script", "style", "noscript", "template", "nav", "header", "footer", "aside", "form",
];
const LINE: &[&str] = &["br", "li", "tr", "dt", "dd", "caption", "figcaption"];
const CELL: &[&str] = &["td", "th"];
const PARAGRAPH: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "blockquote",
    "pre",
    "table",
    "ul",
    "ol",
    "dl",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
];

pub fn to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let root = ["article", "main", "[role=main]", "body"]
        .iter()
        .find_map(|s| doc.select(&Selector::parse(s).expect("селектор")).next())
        .unwrap_or_else(|| doc.root_element());
    let mut out = String::new();
    walk(root, false, &mut out);
    normalize(&out)
}

fn walk(element: ElementRef, pre: bool, out: &mut String) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => {
                if pre {
                    out.push_str(text);
                } else {
                    // Пробельные символы в HTML схлопываются в один пробел.
                    let mut last_space = out.ends_with(' ');
                    for c in text.chars() {
                        if c.is_whitespace() {
                            if !last_space {
                                out.push(' ');
                            }
                            last_space = true;
                        } else {
                            out.push(c);
                            last_space = false;
                        }
                    }
                }
            }
            Node::Element(e) => {
                let name = e.name();
                if SKIP.contains(&name) {
                    continue;
                }
                // Абзац отделяется пустой строкой с обеих сторон, строка списка — переводом
                // строки после себя, чтобы пункты не разделялись пустыми строками.
                let (before, after) = if PARAGRAPH.contains(&name) {
                    ("\n\n", "\n\n")
                } else if LINE.contains(&name) {
                    ("", "\n")
                } else if CELL.contains(&name) {
                    ("", " ")
                } else {
                    ("", "")
                };
                out.push_str(before);
                if let Some(child) = ElementRef::wrap(child) {
                    walk(child, pre || name == "pre", out);
                }
                out.push_str(after);
            }
            _ => {}
        }
    }
}

/// Пробелы вокруг переводов строк убираются, больше одной пустой строки подряд — нет.
fn normalize(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = String::new();
    let mut blank = 0;
    for line in lines {
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        out.push_str(line);
        blank = 0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_article_without_menus_and_scripts() {
        let html = r#"<html><head><title>T</title><script>var x = 1;</script></head><body>
            <nav>Главная Новости</nav>
            <article><h1>Заголовок</h1><p>Первый   абзац
            текста.</p><ul><li>Один</li><li>Два</li></ul><script>alert(1)</script></article>
            <footer>Подвал</footer></body></html>"#;
        assert_eq!(
            to_text(html),
            "Заголовок\n\nПервый абзац текста.\n\nОдин\nДва"
        );
    }

    #[test]
    fn separates_table_cells() {
        assert_eq!(
            to_text(
                "<table><tr><th>Имя</th><th>Фамилия</th></tr><tr><td>Иван</td><td>Петров</td></tr></table>"
            ),
            "Имя Фамилия\nИван Петров"
        );
    }

    #[test]
    fn falls_back_to_body() {
        assert_eq!(
            to_text("<body><p>Раз.</p><p>Два.</p></body>"),
            "Раз.\n\nДва."
        );
    }
}
