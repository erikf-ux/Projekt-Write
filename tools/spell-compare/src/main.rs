//! Compares two spell checkers on the corpus of Phase 12 (ADR 0023):
//! how many correct words are flagged, how many errors are found, and
//! where the intended correction ranks among the suggestions; with times.

use std::time::{Duration, Instant};

const CORRECT: &str = include_str!("../../../src-tauri/tests/data/language/de-richtig.txt");
const ERRORS: &str = include_str!("../../../src-tauri/tests/data/language/de-fehler.txt");

trait Checker {
    fn name(&self) -> &'static str;
    fn check(&self, word: &str) -> bool;
    fn suggest(&self, word: &str) -> Vec<String>;
}

struct Spellbook(spellbook::Dictionary);

impl Checker for Spellbook {
    fn name(&self) -> &'static str {
        "spellbook 0.4.2"
    }
    fn check(&self, word: &str) -> bool {
        self.0.check(word)
    }
    fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.0.suggest(word, &mut out);
        out
    }
}

/// spellbook with the reordering of section 14: umlaut and ß variants of
/// the word first (`ue`→`ü`, `ss`→`ß`, also `u`→`ü` and the like), if the
/// dictionary knows them; then the rest, words without a space or hyphen
/// before those with one. At most five.
struct Reordered(Spellbook);

/// Spellings with umlauts and ß, up to two changes.
fn variants(word: &str) -> Vec<String> {
    const PAIRS: [(&str, &str); 10] = [
        ("ae", "ä"), ("oe", "ö"), ("ue", "ü"), ("Ae", "Ä"), ("Oe", "Ö"), ("Ue", "Ü"),
        ("ss", "ß"), ("a", "ä"), ("o", "ö"), ("u", "ü"),
    ];
    let mut level = vec![word.to_owned()];
    let mut all = Vec::new();
    for _ in 0..2 {
        let mut next = Vec::new();
        for current in &level {
            for (from, to) in PAIRS {
                let mut start = 0;
                while let Some(found) = current[start..].find(from) {
                    let at = start + found;
                    let changed = format!("{}{}{}", &current[..at], to, &current[at + from.len()..]);
                    if !all.contains(&changed) && changed != word {
                        all.push(changed.clone());
                        next.push(changed);
                    }
                    start = at + from.len();
                }
            }
        }
        level = next;
    }
    all
}

impl Checker for Reordered {
    fn name(&self) -> &'static str {
        "spellbook 0.4.2, umsortiert nach Abschnitt 14"
    }
    fn check(&self, word: &str) -> bool {
        self.0.check(word)
    }
    fn suggest(&self, word: &str) -> Vec<String> {
        let mut out: Vec<String> = variants(word).into_iter().filter(|v| self.0.check(v)).collect();
        let rest = self.0.suggest(word);
        let (single, joined): (Vec<String>, Vec<String>) =
            rest.into_iter().partition(|s| !s.contains(' ') && !s.contains('-'));
        for s in single.into_iter().chain(joined) {
            if !out.contains(&s) {
                out.push(s);
            }
        }
        out.truncate(5);
        out
    }
}

struct Hunspell(hunspell_rs::Hunspell);

impl Checker for Hunspell {
    fn name(&self) -> &'static str {
        "Hunspell 1.7 (hunspell-rs 0.4.0)"
    }
    fn check(&self, word: &str) -> bool {
        matches!(self.0.check(word), hunspell_rs::CheckResult::FoundInDictionary)
    }
    fn suggest(&self, word: &str) -> Vec<String> {
        self.0.suggest(word)
    }
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn percentile(values: &mut [Duration], share: f64) -> Duration {
    values.sort();
    let index = ((values.len() as f64 * share).ceil() as usize).clamp(1, values.len()) - 1;
    values[index]
}

fn run(checker: &dyn Checker) {
    let correct: Vec<&str> = lines(CORRECT).collect();
    let errors: Vec<(&str, &str, &str)> = lines(ERRORS)
        .filter_map(|l| {
            let mut parts = l.split('\t');
            Some((parts.next()?, parts.next()?, parts.next()?))
        })
        .collect();
    let mut check_times = Vec::new();
    let flagged: Vec<&str> = correct
        .iter()
        .copied()
        .filter(|word| {
            let start = Instant::now();
            let ok = checker.check(word);
            check_times.push(start.elapsed());
            !ok
        })
        .collect();
    let missed: Vec<&str> = errors
        .iter()
        .filter(|(wrong, _, _)| checker.check(wrong))
        .map(|(wrong, _, _)| *wrong)
        .collect();
    let mut suggest_times = Vec::new();
    let (mut first, mut top5, mut none) = (0, 0, 0);
    let mut ranks: Vec<String> = Vec::new();
    for (wrong, right, kind) in &errors {
        let start = Instant::now();
        let suggestions = checker.suggest(wrong);
        suggest_times.push(start.elapsed());
        match suggestions.iter().position(|s| s == right) {
            Some(0) => first += 1,
            Some(n) if n < 5 => top5 += 1,
            Some(n) => ranks.push(format!("{wrong}→{right} ({kind}): Platz {}", n + 1)),
            None => {
                none += 1;
                ranks.push(format!(
                    "{wrong}→{right} ({kind}): fehlt; vorne {:?}",
                    suggestions.iter().take(3).collect::<Vec<_>>()
                ));
            }
        }
    }
    println!("== {}", checker.name());
    println!(
        "richtige Wörter: {} von {} markiert ({:.1} %): {:?}",
        flagged.len(),
        correct.len(),
        100.0 * flagged.len() as f64 / correct.len() as f64,
        flagged
    );
    println!(
        "Fehler: {} von {} erkannt ({:.1} %); nicht erkannt: {:?}",
        errors.len() - missed.len(),
        errors.len(),
        100.0 * (errors.len() - missed.len()) as f64 / errors.len() as f64,
        missed
    );
    println!(
        "Vorschläge: richtige Form an erster Stelle {first}, unter den ersten fünf {top5}, weiter hinten {}, gar nicht {none}",
        errors.len() - first - top5 - none
    );
    println!(
        "Zeit je Prüfung: Median {:?}, 95 % {:?}; je Vorschlagsliste: Median {:?}, 95 % {:?}, max {:?}",
        percentile(&mut check_times.clone(), 0.5),
        percentile(&mut check_times, 0.95),
        percentile(&mut suggest_times.clone(), 0.5),
        percentile(&mut suggest_times.clone(), 0.95),
        percentile(&mut suggest_times, 1.0),
    );
    for line in ranks {
        println!("  {line}");
    }
}

fn main() {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/usr/share/hunspell/de_DE".to_owned());
    let aff = std::fs::read_to_string(format!("{base}.aff")).expect("aff");
    let dic = std::fs::read_to_string(format!("{base}.dic")).expect("dic");
    let start = Instant::now();
    let spellbook = Spellbook(spellbook::Dictionary::new(&aff, &dic).expect("spellbook dictionary"));
    println!("spellbook geladen in {:?}", start.elapsed());
    let start = Instant::now();
    let hunspell = Hunspell(hunspell_rs::Hunspell::new(&format!("{base}.aff"), &format!("{base}.dic")));
    println!("Hunspell geladen in {:?}", start.elapsed());
    run(&spellbook);
    run(&hunspell);
    let reordered = Reordered(Spellbook(spellbook::Dictionary::new(&aff, &dic).expect("spellbook dictionary")));
    run(&reordered);
}
