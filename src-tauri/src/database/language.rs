//! Rows of the language check (section 14, ADR 0025): own dictionaries,
//! their words and targets, rejected autocorrections and the language
//! profiles of documents. Static SQL only.

use sqlx::{Row, SqliteConnection, SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryRow {
    pub id: String,
    pub name: String,
    pub for_all: bool,
    pub words: i64,
    pub documents: Vec<String>,
    pub subnets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRow {
    pub word: String,
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    pub languages: String,
    pub autocorrect: bool,
}

/// Every own dictionary with its word count and targets, by name.
pub async fn dictionaries(pool: &SqlitePool) -> Result<Vec<DictionaryRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT d.id, d.name, d.for_all, \
                (SELECT count(*) FROM user_dictionary_words w WHERE w.dictionary_id = d.id) \
         FROM user_dictionaries d ORDER BY d.name_key, d.id",
    )
    .fetch_all(pool)
    .await?;
    let targets = sqlx::query(
        "SELECT dictionary_id, document_id, subnet_id FROM user_dictionary_targets \
         ORDER BY dictionary_id, document_id, subnet_id",
    )
    .fetch_all(pool)
    .await?;
    let mut result: Vec<DictionaryRow> = rows
        .iter()
        .map(|row| {
            Ok(DictionaryRow {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
                for_all: row.try_get::<i64, _>(2)? != 0,
                words: row.try_get(3)?,
                documents: Vec::new(),
                subnets: Vec::new(),
            })
        })
        .collect::<Result<_, sqlx::Error>>()?;
    for target in &targets {
        let dictionary: String = target.try_get(0)?;
        let Some(entry) = result.iter_mut().find(|d| d.id == dictionary) else {
            continue;
        };
        if let Some(document) = target.try_get::<Option<String>, _>(1)? {
            entry.documents.push(document);
        }
        if let Some(subnet) = target.try_get::<Option<String>, _>(2)? {
            entry.subnets.push(subnet);
        }
    }
    Ok(result)
}

pub async fn dictionary_exists(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let found = sqlx::query("SELECT 1 FROM user_dictionaries WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?;
    Ok(found.is_some())
}

/// The id of the dictionary with this name key, if any.
pub async fn dictionary_named(
    connection: &mut SqliteConnection,
    name_key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM user_dictionaries WHERE name_key = ?1")
        .bind(name_key)
        .fetch_optional(&mut *connection)
        .await
}

/// The first dictionary for all documents, by name: where "add to
/// dictionary" puts a word.
pub async fn first_for_all(
    connection: &mut SqliteConnection,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM user_dictionaries WHERE for_all = 1 ORDER BY created_at, id LIMIT 1",
    )
    .fetch_optional(&mut *connection)
    .await
}

pub async fn insert_dictionary(
    connection: &mut SqliteConnection,
    id: &str,
    name: &str,
    name_key: &str,
    for_all: bool,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_dictionaries (id, name, name_key, for_all, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
    )
    .bind(id)
    .bind(name)
    .bind(name_key)
    .bind(i64::from(for_all))
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn rename_dictionary(
    connection: &mut SqliteConnection,
    id: &str,
    name: &str,
    name_key: &str,
    now: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE user_dictionaries SET name = ?2, name_key = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(id)
    .bind(name)
    .bind(name_key)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn delete_dictionary(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("DELETE FROM user_dictionaries WHERE id = ?1")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Replaces the targets of a dictionary.
pub async fn set_targets(
    connection: &mut SqliteConnection,
    id: &str,
    for_all: bool,
    documents: &[String],
    subnets: &[String],
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE user_dictionaries SET for_all = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(id)
        .bind(i64::from(for_all))
        .bind(now)
        .execute(&mut *connection)
        .await?;
    sqlx::query("DELETE FROM user_dictionary_targets WHERE dictionary_id = ?1")
        .bind(id)
        .execute(&mut *connection)
        .await?;
    for document in documents {
        sqlx::query(
            "INSERT INTO user_dictionary_targets (dictionary_id, document_id) VALUES (?1, ?2)",
        )
        .bind(id)
        .bind(document)
        .execute(&mut *connection)
        .await?;
    }
    for subnet in subnets {
        sqlx::query(
            "INSERT INTO user_dictionary_targets (dictionary_id, subnet_id) VALUES (?1, ?2)",
        )
        .bind(id)
        .bind(subnet)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// How many of the ids are documents (not in the trash's final delete).
pub async fn count_documents(
    connection: &mut SqliteConnection,
    ids: &[String],
) -> Result<usize, sqlx::Error> {
    let mut found = 0;
    for id in ids {
        let row = sqlx::query("SELECT 1 FROM documents WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *connection)
            .await?;
        found += usize::from(row.is_some());
    }
    Ok(found)
}

pub async fn count_subnets(
    connection: &mut SqliteConnection,
    ids: &[String],
) -> Result<usize, sqlx::Error> {
    let mut found = 0;
    for id in ids {
        let row = sqlx::query("SELECT 1 FROM subnets WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *connection)
            .await?;
        found += usize::from(row.is_some());
    }
    Ok(found)
}

pub async fn words(pool: &SqlitePool, id: &str) -> Result<Vec<WordRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT word, category FROM user_dictionary_words WHERE dictionary_id = ?1 \
         ORDER BY lower(word), word",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(WordRow {
                word: row.try_get(0)?,
                category: row.try_get(1)?,
            })
        })
        .collect()
}

/// Adds or updates a word; returns whether it was new.
pub async fn upsert_word(
    connection: &mut SqliteConnection,
    id: &str,
    word: &str,
    category: &str,
    now: &str,
) -> Result<bool, sqlx::Error> {
    let existed =
        sqlx::query("SELECT 1 FROM user_dictionary_words WHERE dictionary_id = ?1 AND word = ?2")
            .bind(id)
            .bind(word)
            .fetch_optional(&mut *connection)
            .await?
            .is_some();
    sqlx::query(
        "INSERT INTO user_dictionary_words (dictionary_id, word, category, created_at) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT (dictionary_id, word) DO UPDATE SET category = excluded.category",
    )
    .bind(id)
    .bind(word)
    .bind(category)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    sqlx::query("UPDATE user_dictionaries SET updated_at = ?2 WHERE id = ?1")
        .bind(id)
        .bind(now)
        .execute(&mut *connection)
        .await?;
    Ok(!existed)
}

pub async fn delete_word(
    connection: &mut SqliteConnection,
    id: &str,
    word: &str,
) -> Result<bool, sqlx::Error> {
    let done =
        sqlx::query("DELETE FROM user_dictionary_words WHERE dictionary_id = ?1 AND word = ?2")
            .bind(id)
            .bind(word)
            .execute(&mut *connection)
            .await?;
    Ok(done.rows_affected() == 1)
}

/// The own words that apply to a document: dictionaries for all, of the
/// document, of its subnets and of their parents. Without a document
/// (an external file): the dictionaries for all.
pub async fn words_for(
    pool: &SqlitePool,
    document: Option<&str>,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "WITH RECURSIVE scopes (id) AS ( \
             SELECT subnet_id FROM subnet_memberships WHERE document_id = ?1 \
             UNION \
             SELECT s.parent_id FROM subnets s JOIN scopes ON s.id = scopes.id WHERE s.parent_id IS NOT NULL \
         ), \
         chosen (id) AS ( \
             SELECT id FROM user_dictionaries WHERE for_all = 1 \
             UNION SELECT dictionary_id FROM user_dictionary_targets WHERE document_id = ?1 \
             UNION SELECT dictionary_id FROM user_dictionary_targets WHERE subnet_id IN (SELECT id FROM scopes) \
         ) \
         SELECT DISTINCT word FROM user_dictionary_words WHERE dictionary_id IN (SELECT id FROM chosen)",
    )
    .bind(document)
    .fetch_all(pool)
    .await
}

pub async fn is_rejected(
    pool: &SqlitePool,
    word: &str,
    correction: &str,
) -> Result<bool, sqlx::Error> {
    let found =
        sqlx::query("SELECT 1 FROM autocorrect_rejections WHERE word = ?1 AND correction = ?2")
            .bind(word)
            .bind(correction)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// The corrections of a word the user took back.
pub async fn rejected_corrections(
    pool: &SqlitePool,
    word: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT correction FROM autocorrect_rejections WHERE word = ?1")
        .bind(word)
        .fetch_all(pool)
        .await
}

pub async fn reject(
    connection: &mut SqliteConnection,
    word: &str,
    correction: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO autocorrect_rejections (word, correction, created_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (word, correction) DO NOTHING",
    )
    .bind(word)
    .bind(correction)
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn rejections(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT word, correction FROM autocorrect_rejections ORDER BY lower(word), correction",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
        .collect()
}

pub async fn forget_rejection(
    connection: &mut SqliteConnection,
    word: &str,
    correction: &str,
) -> Result<bool, sqlx::Error> {
    let done =
        sqlx::query("DELETE FROM autocorrect_rejections WHERE word = ?1 AND correction = ?2")
            .bind(word)
            .bind(correction)
            .execute(&mut *connection)
            .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn profile(pool: &SqlitePool, document: &str) -> Result<Option<ProfileRow>, sqlx::Error> {
    let row =
        sqlx::query("SELECT languages, autocorrect FROM document_language WHERE document_id = ?1")
            .bind(document)
            .fetch_optional(pool)
            .await?;
    row.map(|row| {
        Ok(ProfileRow {
            languages: row.try_get(0)?,
            autocorrect: row.try_get::<i64, _>(1)? != 0,
        })
    })
    .transpose()
}

pub async fn set_profile(
    connection: &mut SqliteConnection,
    document: &str,
    languages: &str,
    autocorrect: bool,
    now: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO document_language (document_id, languages, autocorrect, updated_at) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT (document_id) DO UPDATE SET languages = excluded.languages, \
             autocorrect = excluded.autocorrect, updated_at = excluded.updated_at",
    )
    .bind(document)
    .bind(languages)
    .bind(i64::from(autocorrect))
    .bind(now)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn document_exists(
    connection: &mut SqliteConnection,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let found = sqlx::query("SELECT 1 FROM documents WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?;
    Ok(found.is_some())
}
