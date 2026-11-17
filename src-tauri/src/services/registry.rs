//! Internal registries (section 22; ADR 0027, decision 1).
//!
//! A [`Registry`] lists the entries of one kind – document formats,
//! exporters, checking engines – and the code chooses through it (by id or
//! another key of the entries) instead of through hard-wired case
//! distinctions.
//!
//! A registry is immutable: it is built once, from the built-in entries
//! listed in [`crate::services::extensions::Extensions::builtin`], and has
//! no method that adds, replaces or removes an entry afterwards. There is
//! no plugin folder and no command that reads code: version 1.0 runs no
//! code of third parties (section 22). A malformed or repeated id is a
//! programming error and stops the build of the registry with a panic.

use std::fmt;

/// An entry of a registry: anything with a stable id.
pub trait RegistryEntry {
    /// The id, unique in its registry, of the form `[a-z][a-z0-9.-]*`. It
    /// is stable: the frontend and stored data may name it.
    fn id(&self) -> &'static str;
}

impl<T: RegistryEntry + ?Sized> RegistryEntry for Box<T> {
    fn id(&self) -> &'static str {
        (**self).id()
    }
}

impl<T: RegistryEntry + ?Sized> RegistryEntry for &T {
    fn id(&self) -> &'static str {
        (**self).id()
    }
}

/// An immutable list of entries with unique ids, in registration order.
pub struct Registry<T> {
    /// What the registry holds, for messages (`documentFormats`).
    kind: &'static str,
    entries: Vec<T>,
}

impl<T: RegistryEntry> Registry<T> {
    /// Builds a registry from `entries`, kept in this order.
    ///
    /// # Panics
    ///
    /// If an id does not have the form `[a-z][a-z0-9.-]*` or appears twice.
    /// Both are mistakes in the list of built-in entries, never input.
    pub fn new(kind: &'static str, entries: impl IntoIterator<Item = T>) -> Self {
        let entries: Vec<T> = entries.into_iter().collect();
        for (index, entry) in entries.iter().enumerate() {
            let id = entry.id();
            assert!(
                is_valid_id(id),
                "registry {kind}: the id {id:?} does not have the form [a-z][a-z0-9.-]*"
            );
            assert!(
                entries[..index].iter().all(|other| other.id() != id),
                "registry {kind}: the id {id:?} is registered twice"
            );
        }
        Self { kind, entries }
    }

    /// What the registry holds.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// The entry with `id`.
    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.iter().find(|entry| entry.id() == id)
    }

    /// The first entry that `matches`, in registration order.
    pub fn find(&self, matches: impl Fn(&T) -> bool) -> Option<&T> {
        self.entries.iter().find(|entry| matches(entry))
    }

    /// All entries in registration order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.entries.iter()
    }

    /// The ids in registration order.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(RegistryEntry::id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<'a, T> IntoIterator for &'a Registry<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<T> fmt::Debug for Registry<T>
where
    T: RegistryEntry,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("kind", &self.kind)
            .field("ids", &self.entries.iter().map(T::id).collect::<Vec<_>>())
            .finish()
    }
}

/// `[a-z][a-z0-9.-]*`.
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Entry(&'static str, u32);

    impl RegistryEntry for Entry {
        fn id(&self) -> &'static str {
            self.0
        }
    }

    #[test]
    fn entries_are_found_by_id_and_kept_in_order() {
        let registry = Registry::new("test", [Entry("b", 1), Entry("a", 2), Entry("c.d-2", 3)]);
        assert_eq!(registry.kind(), "test");
        assert_eq!(registry.len(), 3);
        assert!(!registry.is_empty());
        assert_eq!(registry.get("a"), Some(&Entry("a", 2)));
        assert_eq!(registry.get("c.d-2").map(|entry| entry.1), Some(3));
        assert_eq!(registry.get("x"), None);
        assert_eq!(registry.ids().collect::<Vec<_>>(), ["b", "a", "c.d-2"]);
        assert_eq!(
            registry.iter().map(|entry| entry.1).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(registry.find(|entry| entry.1 > 1), Some(&Entry("a", 2)));
        assert_eq!((&registry).into_iter().count(), 3);
    }

    #[test]
    fn trait_objects_are_entries() {
        trait Named: RegistryEntry {}
        impl Named for Entry {}
        let registry: Registry<Box<dyn Named>> =
            Registry::new("boxed", [Box::new(Entry("one", 1)) as Box<dyn Named>]);
        assert_eq!(registry.get("one").map(|entry| entry.id()), Some("one"));
        assert_eq!(
            format!("{registry:?}"),
            r#"Registry { kind: "boxed", ids: ["one"] }"#
        );
    }

    #[test]
    fn an_empty_registry_is_allowed() {
        let registry: Registry<Entry> = Registry::new("empty", []);
        assert!(registry.is_empty());
        assert_eq!(registry.iter().count(), 0);
    }

    #[test]
    #[should_panic(expected = "the id \"twice\" is registered twice")]
    fn a_repeated_id_stops_the_build() {
        let _ = Registry::new(
            "test",
            [Entry("twice", 1), Entry("other", 2), Entry("twice", 3)],
        );
    }

    #[test]
    #[should_panic(expected = "does not have the form")]
    fn an_upper_case_id_stops_the_build() {
        let _ = Registry::new("test", [Entry("Txt", 1)]);
    }

    #[test]
    #[should_panic(expected = "does not have the form")]
    fn an_empty_id_stops_the_build() {
        let _ = Registry::new("test", [Entry("", 1)]);
    }

    #[test]
    fn ids_have_the_documented_form() {
        for valid in ["a", "txt", "md", "language-tool", "x.y-1", "a0"] {
            assert!(is_valid_id(valid), "{valid}");
        }
        for invalid in ["", "1a", "-a", ".a", "A", "aB", "a_b", "a b", "ä", "a/b"] {
            assert!(!is_valid_id(invalid), "{invalid}");
        }
    }
}
