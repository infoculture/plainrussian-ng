//! Плотность находок проверок ясного языка по уровням корпуса: на 100 слов.
//! Проверяет, что проверки отличают детские тексты от канцелярита.
//!
//!     cargo run -p pr-core --features clarity --example clarity_stats -- <каталог textsbygrade>

use std::collections::BTreeMap;
use std::path::PathBuf;

use pr_core::clarity::{Dictionary, check};

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("укажите каталог корпуса"));
    let dictionary = Dictionary::bundled().expect("словарь rumorph");
    // Группа уровней → [слова, отглагольные, страдательные, цепочки, канцеляризмы, аббревиатуры].
    let mut groups: BTreeMap<&str, [u64; 6]> = BTreeMap::new();
    let started = std::time::Instant::now();
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        let grade: u32 = dir.file_name().unwrap().to_str().unwrap().parse().unwrap();
        let group = match grade {
            1..=4 => "1–4",
            5..=9 => "5–9",
            10..=12 => "10–12",
            _ => "15–17",
        };
        for file in std::fs::read_dir(&dir).unwrap() {
            let text = std::fs::read_to_string(file.unwrap().path()).unwrap();
            let r = check(&dictionary, &text);
            let g = groups.entry(group).or_default();
            let c = &r.counts;
            for (slot, n) in g.iter_mut().zip([
                r.words,
                c.nominalization,
                c.passive,
                c.genitive_chain,
                c.bureaucratese,
                c.abbreviation,
            ]) {
                *slot += n;
            }
        }
    }
    println!("уровни\tслов\tотглаг.\tстрад.\tцепочки\tканц.\tаббр.  (на 100 слов)");
    for (group, g) in &groups {
        let per = |n: u64| 100.0 * n as f64 / g[0] as f64;
        println!(
            "{group}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
            g[0],
            per(g[1]),
            per(g[2]),
            per(g[3]),
            per(g[4]),
            per(g[5])
        );
    }
    eprintln!("время: {:?}", started.elapsed());
}
