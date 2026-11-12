//! Automatic grouping (section 15, ADR 0022, decision 8): suggestions only.
//!
//! The analysis compares documents and media by shared tags, title
//! similarity (trigrams of the normalised titles), content similarity
//! (TF-IDF with cosine similarity, local text statistics), links, shared
//! import folders and existing relations (shared subnets). The score is a
//! weighted sum; a pair becomes a suggestion from a threshold (0.35 to
//! start with). Every suggestion keeps its reasons.
//!
//! Comparing every pair of 15,000 elements would be 112 million pairs, so
//! only pairs that share something are looked at: a tag, a rare title
//! trigram, a rare important word, a link target, a folder or a subnet.
//!
//! The analysis reads; it never writes `mesh_connections`. It skips pairs
//! that are connected or were suggested before, whatever the decision: a
//! rejected suggestion never comes back.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Section 15: "Startwert 0,35".
pub const DEFAULT_THRESHOLD: f64 = 0.35;
/// New suggestions one run stores at most, the best first.
pub const MAX_NEW_SUGGESTIONS: usize = 1000;
/// Weights of the parts; they add up to 1.
pub const WEIGHTS: Weights = Weights {
    tags: 0.25,
    title: 0.15,
    content: 0.25,
    links: 0.15,
    folder: 0.10,
    relations: 0.10,
};
/// Groups larger than this make no candidates (a tag on half the library
/// says little about two of its elements).
const MAX_GROUP: usize = 300;
/// Important words kept per document.
const TOP_TERMS: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub tags: f64,
    pub title: f64,
    pub content: f64,
    pub links: f64,
    pub folder: f64,
    pub relations: f64,
}

/// Why a pair was suggested; stored as JSON in `mesh_suggestions.reason`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuggestionReason {
    /// Tags both have.
    pub shared_tags: u32,
    /// Trigram similarity of the titles, 0 to 1.
    pub title: f64,
    /// Cosine similarity of the texts, 0 to 1.
    pub content: f64,
    /// One links to the other.
    pub linked: bool,
    /// Link targets both have.
    pub shared_links: u32,
    /// Imported from the same folder.
    pub same_folder: bool,
    /// Subnets both are in.
    pub shared_subnets: u32,
}

/// One element the analysis looks at.
#[derive(Debug, Clone, Default)]
pub struct AnalysisElement {
    pub id: String,
    pub title: String,
    /// The text of a document (empty for media).
    pub text: String,
    pub tags: Vec<String>,
    pub subnets: Vec<String>,
    /// Link targets and media used (documents).
    pub links: Vec<String>,
    pub import_folder: Option<String>,
}

/// A pair worth suggesting, ends sorted.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub node_a: String,
    pub node_b: String,
    pub score: f64,
    pub reason: SuggestionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AnalysisStep {
    Reading,
    Comparing,
    Saving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalysisProgress {
    pub step: AnalysisStep,
    pub done: u32,
    pub total: u32,
}

/// What a run did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalysisReport {
    pub elements: u32,
    pub pairs_compared: u32,
    pub created: u32,
}

/// Lower case words of three or more letters, not only digits, without the
/// most common German and English words.
pub fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 3 && !word.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_lowercase)
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
}

const STOP_WORDS: &[&str] = &[
    "and", "are", "but", "for", "from", "has", "have", "not", "that", "the", "this", "was", "were",
    "with", "you", "als", "auch", "auf", "aus", "bei", "bis", "das", "dass", "dem", "den", "der",
    "des", "die", "ein", "eine", "einem", "einen", "einer", "eines", "für", "hat", "ich", "ist",
    "mit", "nach", "nicht", "noch", "oder", "sich", "sie", "sind", "und", "uns", "von", "vor",
    "war", "wie", "wir", "wird", "zum", "zur", "über",
];

/// Trigrams of a title: lower case, letters and digits, padded.
pub fn trigrams(title: &str) -> HashSet<String> {
    let normal: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let normal = normal.split_whitespace().collect::<Vec<_>>().join(" ");
    let padded: Vec<char> = format!("  {normal} ").chars().collect();
    padded
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .collect()
}

/// Shared elements over the smaller set (overlap coefficient): two of three
/// tags in common count as much as two of two.
fn overlap<T: Eq + std::hash::Hash>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    let smaller = a.len().min(b.len());
    if smaller == 0 {
        0.0
    } else {
        a.intersection(b).count() as f64 / smaller as f64
    }
}

/// The smallest sum of weights a score is divided by: one similar title
/// alone is no suggestion.
const MIN_APPLICABLE_WEIGHT: f64 = 0.5;

fn jaccard<T: Eq + std::hash::Hash>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    let shared = a.intersection(b).count();
    let union = a.len() + b.len() - shared;
    if union == 0 {
        0.0
    } else {
        shared as f64 / union as f64
    }
}

/// Sparse unit vectors of the most important words per element.
fn tf_idf(elements: &[AnalysisElement]) -> Vec<Vec<(u32, f64)>> {
    let mut vocabulary: HashMap<String, u32> = HashMap::new();
    let mut counts: Vec<HashMap<u32, f64>> = Vec::with_capacity(elements.len());
    let mut document_frequency: HashMap<u32, u32> = HashMap::new();
    for element in elements {
        let mut tf: HashMap<u32, f64> = HashMap::new();
        for word in words(&element.text) {
            let next = u32::try_from(vocabulary.len()).unwrap_or(u32::MAX);
            let id = *vocabulary.entry(word).or_insert(next);
            *tf.entry(id).or_default() += 1.0;
        }
        for &id in tf.keys() {
            *document_frequency.entry(id).or_default() += 1;
        }
        counts.push(tf);
    }
    let n = elements.len().max(1) as f64;
    counts
        .into_iter()
        .map(|tf| {
            let mut weighted: Vec<(u32, f64)> = tf
                .into_iter()
                .map(|(id, count)| {
                    let df = f64::from(document_frequency.get(&id).copied().unwrap_or(1));
                    (id, (1.0 + count.ln()) * (n / df).ln())
                })
                .filter(|(_, w)| *w > 0.0)
                .collect();
            weighted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            weighted.truncate(TOP_TERMS);
            let norm = weighted.iter().map(|(_, w)| w * w).sum::<f64>().sqrt();
            if norm > 0.0 {
                for entry in &mut weighted {
                    entry.1 /= norm;
                }
            }
            weighted.sort_by_key(|entry| entry.0);
            weighted
        })
        .collect()
}

fn cosine(a: &[(u32, f64)], b: &[(u32, f64)]) -> f64 {
    let (mut i, mut j, mut sum) = (0, 0, 0.0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                sum += a[i].1 * b[j].1;
                i += 1;
                j += 1;
            }
        }
    }
    sum.clamp(0.0, 1.0)
}

/// Pairs of elements that share a key of `groups` (small groups only).
fn pairs_of_groups(groups: impl Iterator<Item = Vec<usize>>, into: &mut HashSet<(usize, usize)>) {
    for mut group in groups {
        if group.len() < 2 || group.len() > MAX_GROUP {
            continue;
        }
        group.sort_unstable();
        group.dedup();
        for (n, &a) in group.iter().enumerate() {
            for &b in &group[n + 1..] {
                into.insert((a, b));
            }
        }
    }
}

fn group_by<'a, K: std::hash::Hash + Eq + 'a>(
    keys: impl Iterator<Item = (usize, K)>,
) -> impl Iterator<Item = Vec<usize>> {
    let mut groups: HashMap<K, Vec<usize>> = HashMap::new();
    for (element, key) in keys {
        groups.entry(key).or_default().push(element);
    }
    groups.into_values()
}

/// Compares the elements; `known` pairs (sorted ids) are skipped. Returns
/// the pairs from `threshold`, best first, at most [`MAX_NEW_SUGGESTIONS`].
pub fn analyze(
    elements: &[AnalysisElement],
    known: &HashSet<(String, String)>,
    threshold: f64,
    cancel: &AtomicBool,
    progress: &dyn Fn(AnalysisProgress),
) -> Option<(Vec<Candidate>, u32)> {
    let vectors = tf_idf(elements);
    let titles: Vec<HashSet<String>> = elements.iter().map(|e| trigrams(&e.title)).collect();
    let tags: Vec<HashSet<&str>> = elements
        .iter()
        .map(|e| e.tags.iter().map(String::as_str).collect())
        .collect();
    let subnets: Vec<HashSet<&str>> = elements
        .iter()
        .map(|e| e.subnets.iter().map(String::as_str).collect())
        .collect();
    let links: Vec<HashSet<&str>> = elements
        .iter()
        .map(|e| e.links.iter().map(String::as_str).collect())
        .collect();
    let index: HashMap<&str, usize> = elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();

    // Candidates: pairs that share something.
    let mut candidates: HashSet<(usize, usize)> = HashSet::new();
    pairs_of_groups(
        group_by(
            tags.iter()
                .enumerate()
                .flat_map(|(i, set)| set.iter().map(move |t| (i, *t))),
        ),
        &mut candidates,
    );
    pairs_of_groups(
        group_by(
            subnets
                .iter()
                .enumerate()
                .flat_map(|(i, set)| set.iter().map(move |s| (i, *s))),
        ),
        &mut candidates,
    );
    pairs_of_groups(
        group_by(
            links
                .iter()
                .enumerate()
                .flat_map(|(i, set)| set.iter().map(move |l| (i, *l))),
        ),
        &mut candidates,
    );
    pairs_of_groups(
        group_by(
            elements
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.import_folder.as_deref().map(|f| (i, f))),
        ),
        &mut candidates,
    );
    // Rare trigrams and rare important words only: common ones would pair
    // everything with everything.
    pairs_of_groups(
        group_by(
            titles
                .iter()
                .enumerate()
                .flat_map(|(i, set)| set.iter().map(move |t| (i, t.as_str()))),
        )
        .filter(|group| group.len() <= 30),
        &mut candidates,
    );
    pairs_of_groups(
        group_by(
            vectors
                .iter()
                .enumerate()
                .flat_map(|(i, vector)| vector.iter().take(10).map(move |(term, _)| (i, *term))),
        )
        .filter(|group| group.len() <= 100),
        &mut candidates,
    );
    // Direct links.
    for (from, set) in links.iter().enumerate() {
        for target in set {
            if let Some(&to) = index.get(target)
                && from != to
            {
                candidates.insert((from.min(to), from.max(to)));
            }
        }
    }

    let mut pairs: Vec<(usize, usize)> = candidates.into_iter().collect();
    pairs.sort_unstable();
    let total = u32::try_from(pairs.len()).unwrap_or(u32::MAX);
    let mut found = Vec::new();
    for (n, &(a, b)) in pairs.iter().enumerate() {
        if n % 2048 == 0 {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            progress(AnalysisProgress {
                step: AnalysisStep::Comparing,
                done: u32::try_from(n).unwrap_or(u32::MAX),
                total,
            });
        }
        let (ea, eb) = (&elements[a], &elements[b]);
        let key = if ea.id < eb.id {
            (ea.id.clone(), eb.id.clone())
        } else {
            (eb.id.clone(), ea.id.clone())
        };
        if known.contains(&key) {
            continue;
        }
        let shared_tags = tags[a].intersection(&tags[b]).count();
        let linked = links[a].contains(eb.id.as_str()) || links[b].contains(ea.id.as_str());
        let shared_links = links[a].intersection(&links[b]).count();
        let same_folder = ea.import_folder.is_some() && ea.import_folder == eb.import_folder;
        let shared_subnets = subnets[a].intersection(&subnets[b]).count();
        let reason = SuggestionReason {
            shared_tags: u32::try_from(shared_tags).unwrap_or(u32::MAX),
            title: round(jaccard(&titles[a], &titles[b])),
            content: round(cosine(&vectors[a], &vectors[b])),
            linked,
            shared_links: u32::try_from(shared_links).unwrap_or(u32::MAX),
            same_folder,
            shared_subnets: u32::try_from(shared_subnets).unwrap_or(u32::MAX),
        };
        let link_part = if linked {
            1.0
        } else {
            jaccard(&links[a], &links[b])
        };
        // The weighted sum of the parts that apply to the pair, over their
        // weights: a medium without text, or two elements without tags, are
        // not unlike for that reason (ADR 0022, decision 8).
        let parts = [
            (
                !tags[a].is_empty() || !tags[b].is_empty(),
                WEIGHTS.tags,
                overlap(&tags[a], &tags[b]),
            ),
            (true, WEIGHTS.title, reason.title),
            (
                !ea.text.is_empty() && !eb.text.is_empty(),
                WEIGHTS.content,
                reason.content,
            ),
            (
                !links[a].is_empty() || !links[b].is_empty(),
                WEIGHTS.links,
                link_part,
            ),
            (
                ea.import_folder.is_some() && eb.import_folder.is_some(),
                WEIGHTS.folder,
                f64::from(u8::from(same_folder)),
            ),
            (
                !subnets[a].is_empty() || !subnets[b].is_empty(),
                WEIGHTS.relations,
                overlap(&subnets[a], &subnets[b]),
            ),
        ];
        let weight: f64 = parts.iter().filter(|p| p.0).map(|p| p.1).sum();
        let sum: f64 = parts.iter().filter(|p| p.0).map(|p| p.1 * p.2).sum();
        let score = round((sum / weight.max(MIN_APPLICABLE_WEIGHT)).clamp(0.0, 1.0));
        if score >= threshold {
            found.push(Candidate {
                node_a: key.0,
                node_b: key.1,
                score,
                reason,
            });
        }
    }
    found.sort_by(|x, y| {
        y.score
            .total_cmp(&x.score)
            .then_with(|| (&x.node_a, &x.node_b).cmp(&(&y.node_a, &y.node_b)))
    });
    found.truncate(MAX_NEW_SUGGESTIONS);
    Some((found, total))
}

fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// A running analysis, so a second one is refused and it can be cancelled.
#[derive(Debug, Default)]
pub struct AnalysisRegistry {
    running: std::sync::Mutex<Option<Arc<AtomicBool>>>,
}

impl AnalysisRegistry {
    /// The cancel flag of a new run, or `None` if one is running.
    pub fn start(&self) -> Option<Arc<AtomicBool>> {
        let mut running = self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if running.is_some() {
            return None;
        }
        let flag = Arc::new(AtomicBool::new(false));
        *running = Some(flag.clone());
        Some(flag)
    }

    pub fn finish(&self) {
        *self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Cancels a running analysis; `false` if none runs.
    pub fn cancel(&self) -> bool {
        match &*self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
}

/// The reason in words of the UI keys, for tests and logs.
pub fn summary(reason: &SuggestionReason) -> BTreeMap<&'static str, String> {
    let mut parts = BTreeMap::new();
    if reason.shared_tags > 0 {
        parts.insert("sharedTags", reason.shared_tags.to_string());
    }
    if reason.content > 0.0 {
        parts.insert("content", format!("{:.2}", reason.content));
    }
    if reason.title > 0.0 {
        parts.insert("title", format!("{:.2}", reason.title));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(id: &str, title: &str, text: &str, tags: &[&str]) -> AnalysisElement {
        AnalysisElement {
            id: id.to_owned(),
            title: title.to_owned(),
            text: text.to_owned(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            ..AnalysisElement::default()
        }
    }

    fn library() -> Vec<AnalysisElement> {
        vec![
            element(
                "a",
                "Brückenbau im Hafen",
                "Stahlträger und Pfeiler tragen die Brücke über den Hafen. Stahlträger rosten.",
                &["bau", "hafen", "stahl"],
            ),
            element(
                "b",
                "Brückenbau am Fluss",
                "Pfeiler aus Beton und Stahlträger: die Brücke über den Fluss.",
                &["bau", "stahl", "fluss"],
            ),
            element(
                "c",
                "Rezept Apfelkuchen",
                "Äpfel, Zucker, Mehl und Butter verrühren.",
                &["kochen"],
            ),
            element(
                "d",
                "Kuchen backen",
                "Butter und Zucker schaumig schlagen, Mehl dazu.",
                &["kochen"],
            ),
        ]
    }

    fn run(elements: &[AnalysisElement], known: &HashSet<(String, String)>) -> Vec<Candidate> {
        analyze(
            elements,
            known,
            DEFAULT_THRESHOLD,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap()
        .0
    }

    #[test]
    fn the_weights_add_up_to_one() {
        let sum = WEIGHTS.tags
            + WEIGHTS.title
            + WEIGHTS.content
            + WEIGHTS.links
            + WEIGHTS.folder
            + WEIGHTS.relations;
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn similar_elements_are_suggested_with_their_reasons() {
        let found = run(&library(), &HashSet::new());
        let pairs: Vec<_> = found
            .iter()
            .map(|c| (c.node_a.as_str(), c.node_b.as_str()))
            .collect();
        assert!(pairs.contains(&("a", "b")), "{found:?}");
        let ab = found.iter().find(|c| c.node_a == "a").unwrap();
        assert_eq!(ab.reason.shared_tags, 2);
        assert!(ab.reason.content > 0.1);
        assert!(ab.reason.title > 0.2);
        assert!(ab.score >= DEFAULT_THRESHOLD);
        // The two cakes too; nothing between baking and bridges.
        assert!(pairs.contains(&("c", "d")));
        assert!(!pairs.contains(&("a", "c")) && !pairs.contains(&("b", "d")));
        assert!(!pairs.contains(&("a", "d")) && !pairs.contains(&("b", "c")));
    }

    #[test]
    fn known_pairs_are_never_suggested_again() {
        let known: HashSet<_> = [("a".to_owned(), "b".to_owned())].into_iter().collect();
        let found = run(&library(), &known);
        assert!(found.iter().all(|c| !(c.node_a == "a" && c.node_b == "b")));
    }

    #[test]
    fn links_folders_and_subnets_count() {
        let mut elements = vec![
            element("x", "Eins", "", &[]),
            element("y", "Zwei", "", &[]),
            element("z", "Drei", "", &[]),
        ];
        elements[0].links = vec!["y".to_owned()];
        elements[0].subnets = vec!["s".to_owned()];
        elements[1].subnets = vec!["s".to_owned()];
        elements[1].import_folder = Some("Fotos".to_owned());
        elements[2].import_folder = Some("Fotos".to_owned());
        let all = analyze(
            &elements,
            &HashSet::new(),
            0.0,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap()
        .0;
        let xy = all
            .iter()
            .find(|c| c.node_a == "x" && c.node_b == "y")
            .unwrap();
        assert!(xy.reason.linked);
        assert_eq!(xy.reason.shared_subnets, 1);
        // Links, subnets and title apply (0.4 < 0.5): (0.15 + 0.10) / 0.5.
        assert!((xy.score - 0.5).abs() < 0.01, "{xy:?}");
        let yz = all
            .iter()
            .find(|c| c.node_a == "y" && c.node_b == "z")
            .unwrap();
        assert!(yz.reason.same_folder);
    }

    #[test]
    fn a_similar_title_alone_is_no_suggestion() {
        let elements = vec![
            element("p", "IMG_0001", "", &[]),
            element("q", "IMG_0002", "", &[]),
        ];
        assert!(run(&elements, &HashSet::new()).is_empty());
    }

    #[test]
    fn a_cancelled_analysis_returns_nothing() {
        assert!(
            analyze(
                &library(),
                &HashSet::new(),
                0.35,
                &AtomicBool::new(true),
                &|_| {}
            )
            .is_none()
        );
    }

    #[test]
    fn words_and_trigrams_are_normalised() {
        let found: Vec<_> = words("Der Hafen, die STAHL-Träger; 2026 und ab").collect();
        assert_eq!(found, vec!["hafen", "stahl", "träger"]);
        let grams = trigrams("Brücke!");
        assert!(grams.contains("brü") && grams.contains("ke "));
    }

    #[test]
    fn only_one_analysis_runs() {
        let registry = AnalysisRegistry::default();
        let flag = registry.start().unwrap();
        assert!(registry.start().is_none());
        assert!(registry.cancel());
        assert!(flag.load(Ordering::Relaxed));
        registry.finish();
        assert!(registry.start().is_some());
    }
}
