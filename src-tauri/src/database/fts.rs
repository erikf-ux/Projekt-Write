//! SQLite FTS5 helpers (section 19).
//!
//! Phase 1 only verifies that FTS5 works in the bundled SQLite. The search
//! index itself (`search_words`, `search_parts`) is created by a migration in
//! Phase 9; the definitions below mirror section 19 so the probe tests exactly
//! the tokenizers the index will use.

use sqlx::SqlitePool;

/// Column and option list of `search_words` (section 19).
pub const SEARCH_WORDS_DEFINITION: &str =
    "title, body, tags, subnets, meta, tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'";

/// Column and option list of `search_parts` (section 19).
pub const SEARCH_PARTS_DEFINITION: &str = "title, body, tokenize = 'trigram remove_diacritics 1'";

/// Quotes one search term as an FTS5 string: wraps it in `"` and doubles every
/// `"` inside. Without this, input such as `C++` or a lone `"` is a syntax
/// error in a `MATCH` expression (section 19).
pub fn quote_term(term: &str) -> String {
    let mut quoted = String::with_capacity(term.len() + 2);
    quoted.push('"');
    for ch in term.chars() {
        if ch == '"' {
            quoted.push('"');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    quoted
}

/// Outcome of [`probe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtsProbe {
    /// `pragma_compile_options` lists `ENABLE_FTS5`.
    pub compiled_in: bool,
    /// A table with the `search_words` definition could be created.
    pub words_table: bool,
    /// A table with the `search_parts` definition could be created.
    pub parts_table: bool,
    /// The quoted query `"C++"` runs and finds the test row.
    pub quoted_query: bool,
    /// The trigram table finds `haus` inside `Krankenhaus`.
    pub part_query: bool,
}

impl FtsProbe {
    pub fn passed(&self) -> bool {
        self.compiled_in && self.words_table && self.parts_table && self.quoted_query && self.part_query
    }
}

/// Checks FTS5 with real (temporary) tables. Nothing is persisted: the tables
/// live in the connection's `temp` schema inside a transaction that is rolled
/// back.
pub async fn probe(writer: &SqlitePool) -> Result<FtsProbe, sqlx::Error> {
    let compiled_in = super::diagnostics::compile_options(writer)
        .await?
        .iter()
        .any(|option| option == "ENABLE_FTS5");

    let mut tx = writer.begin().await?;
    let words_table = sqlx::query(&format!(
        "CREATE VIRTUAL TABLE temp.pw_probe_words USING fts5({SEARCH_WORDS_DEFINITION})"
    ))
    .execute(&mut *tx)
    .await
    .is_ok();
    let parts_table = sqlx::query(&format!(
        "CREATE VIRTUAL TABLE temp.pw_probe_parts USING fts5({SEARCH_PARTS_DEFINITION})"
    ))
    .execute(&mut *tx)
    .await
    .is_ok();

    let mut quoted_query = false;
    let mut part_query = false;
    if words_table && parts_table {
        let body = "Programmieren in C++ im Krankenhaus";
        sqlx::query("INSERT INTO temp.pw_probe_words (title, body, tags, subnets, meta) VALUES ('Probe', ?1, '', '', '')")
            .bind(body)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO temp.pw_probe_parts (title, body) VALUES ('Probe', ?1)")
            .bind(body)
            .execute(&mut *tx)
            .await?;

        let hits: i64 = sqlx::query_scalar("SELECT count(*) FROM temp.pw_probe_words WHERE pw_probe_words MATCH ?1")
            .bind(quote_term("C++"))
            .fetch_one(&mut *tx)
            .await?;
        quoted_query = hits == 1;

        let hits: i64 = sqlx::query_scalar("SELECT count(*) FROM temp.pw_probe_parts WHERE pw_probe_parts MATCH ?1")
            .bind(quote_term("haus"))
            .fetch_one(&mut *tx)
            .await?;
        part_query = hits == 1;
    }
    tx.rollback().await?;

    Ok(FtsProbe {
        compiled_in,
        words_table,
        parts_table,
        quoted_query,
        part_query,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_terms() {
        assert_eq!(quote_term("C++"), "\"C++\"");
        assert_eq!(quote_term("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_term("\""), "\"\"\"\"");
        assert_eq!(quote_term(""), "\"\"");
        assert_eq!(quote_term("Straße"), "\"Straße\"");
    }
}
