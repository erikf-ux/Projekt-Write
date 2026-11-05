//! Subnets, memberships and tags of one open library (sections 13, 16 and
//! 18; ADR 0017), with the undo journal for structural actions (section 8).
//!
//! Every structural action runs in one transaction. Before and after the
//! change it takes a snapshot of what the action touches: the subnet and
//! tag rows by id, and memberships, tags and trash/archive state of the
//! documents involved. Undo writes the snapshot before, redo the one after,
//! once the current state is checked to equal the expected one.
//!
//! Writing a whole snapshot cannot rely on the order of single updates:
//! moving subnets back could pass through a cycle or a name two siblings
//! share for a moment. So the rows involved are first detached (no parent,
//! a name key no row can have), then written as they were. Every state in
//! between holds only edges and names of the target state, which is a
//! valid tree, so neither the cycle trigger nor a unique index fires.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use sqlx::SqliteConnection;
use ts_rs::TS;
use uuid::Uuid;

use crate::clock::Generators;
use crate::database::Database;
use crate::database::structure::{
    self as rows, DocumentState, DocumentTagRow, MembershipRow, SubnetRow, TagRow,
};
use crate::services::themes::contrast_ratio;

pub const MAX_SUBNET_NAME_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 2000;
pub const MAX_TAG_NAME_CHARS: usize = 100;
/// Steps the undo journal keeps.
pub const UNDO_LIMIT: usize = 100;
/// The cluster colours of section 11, stored by token name.
pub const CLUSTER_COLORS: [&str; 5] = ["blue", "green", "violet", "orange", "grey"];
/// Writing surface and side bar of the dark and the light design.
const BACKGROUNDS: [&str; 4] = ["#1e1e1e", "#252526", "#ffffff", "#f3f3f3"];
/// Section 6: graphical elements reach 3:1.
pub const MIN_COLOR_CONTRAST: f64 = 3.0;

#[derive(Debug, thiserror::Error)]
pub enum StructureError {
    #[error("subnet not found")]
    SubnetNotFound,
    #[error("tag not found")]
    TagNotFound,
    #[error("document not found")]
    DocumentNotFound,
    #[error("document is in the trash")]
    InTrash,
    #[error("document is not in the trash")]
    NotInTrash,
    #[error("a subnet cannot lie below itself")]
    Cycle,
    #[error("a sibling subnet has this name")]
    SubnetNameTaken { existing: String },
    #[error("a tag has this name")]
    TagNameTaken { existing: String },
    #[error("invalid name: {0}")]
    InvalidName(&'static str),
    #[error("invalid description: {0}")]
    InvalidDescription(&'static str),
    #[error("invalid colour: {0}")]
    InvalidColor(&'static str),
    #[error("the document is not in this subnet")]
    NotMember,
    #[error("a tag cannot be merged into itself")]
    SameTag,
    #[error("the structure changed since this step")]
    Changed,
    #[error("database error: {0}")]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for StructureError {
    fn from(error: sqlx::Error) -> Self {
        // The trigger of migration 0007 backs up the check in Rust.
        if let sqlx::Error::Database(database) = &error
            && database.message().contains("subnet cycle")
        {
            return StructureError::Cycle;
        }
        StructureError::Database(error)
    }
}

/// A structural action, as the undo journal names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StructureAction {
    SubnetCreate,
    SubnetUpdate,
    SubnetMove,
    SubnetDelete,
    DocumentAssign,
    DocumentUnassign,
    DocumentMove,
    DocumentPrimary,
    TagCreate,
    TagRename,
    TagMerge,
    TagDelete,
    DocumentTag,
    DocumentUntag,
    DocumentDelete,
    DocumentRestore,
    DocumentArchive,
    DocumentUnarchive,
}

/// A step of the journal: what was done, to what (a name or title).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UndoStep {
    pub action: StructureAction,
    pub subject: String,
}

/// The next step to undo and to redo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UndoHistory {
    pub undo: Option<UndoStep>,
    pub redo: Option<UndoStep>,
}

/// Subnets and tags of one document: the primary subnet first, then the
/// others by age; tags in the order they were given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentStructure {
    pub id: String,
    pub subnet_ids: Vec<String>,
    pub tag_ids: Vec<String>,
}

/// What an action did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The step, if anything changed.
    pub step: Option<UndoStep>,
    /// Documents whose structure or state may have changed.
    pub documents: Vec<String>,
    /// The subnet or tag the action created or worked on.
    pub subject_id: Option<String>,
}

/// Rows a step touches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Keys {
    subnets: BTreeSet<String>,
    tags: BTreeSet<String>,
    documents: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocumentSnapshot {
    memberships: Vec<MembershipRow>,
    tags: Vec<DocumentTagRow>,
    state: DocumentState,
}

/// The rows of [`Keys`] at one moment (`None`: the row does not exist).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Snapshot {
    subnets: BTreeMap<String, Option<SubnetRow>>,
    tags: BTreeMap<String, Option<TagRow>>,
    documents: BTreeMap<String, Option<DocumentSnapshot>>,
}

#[derive(Debug, Clone)]
struct Change {
    step: UndoStep,
    keys: Keys,
    before: Snapshot,
    after: Snapshot,
}

/// Undo and redo stacks of one open library; in memory only (ADR 0017).
#[derive(Debug, Default)]
pub struct Journal {
    undo: Vec<Change>,
    redo: Vec<Change>,
}

impl Journal {
    fn history(&self) -> UndoHistory {
        UndoHistory {
            undo: self.undo.last().map(|change| change.step.clone()),
            redo: self.redo.last().map(|change| change.step.clone()),
        }
    }

    fn record(&mut self, change: Change) {
        self.undo.push(change);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Unicode lower case: names that differ only in case are the same.
pub fn name_key(name: &str) -> String {
    name.to_lowercase()
}

/// Trims a subnet name and checks length and characters.
pub fn validate_subnet_name(name: &str) -> Result<String, StructureError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StructureError::InvalidName("empty"));
    }
    if name.chars().count() > MAX_SUBNET_NAME_CHARS {
        return Err(StructureError::InvalidName("too long"));
    }
    if name.chars().any(char::is_control) {
        return Err(StructureError::InvalidName("control characters"));
    }
    Ok(name.to_owned())
}

/// Trims a tag name; tags have no commas (they separate tags when typing).
pub fn validate_tag_name(name: &str) -> Result<String, StructureError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StructureError::InvalidName("empty"));
    }
    if name.chars().count() > MAX_TAG_NAME_CHARS {
        return Err(StructureError::InvalidName("too long"));
    }
    if name.chars().any(|c| c.is_control() || c == ',') {
        return Err(StructureError::InvalidName("control characters or comma"));
    }
    Ok(name.to_owned())
}

/// Line breaks and tabs are allowed in a description, other control
/// characters not.
pub fn validate_description(description: &str) -> Result<String, StructureError> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(StructureError::InvalidDescription("too long"));
    }
    if description
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(StructureError::InvalidDescription("control characters"));
    }
    Ok(description.to_owned())
}

/// A cluster token, or `#rrggbb` that reaches 3:1 against writing surface
/// and side bar of both designs (ADR 0017, decision 9).
pub fn validate_color(color: &str) -> Result<String, StructureError> {
    let color = color.trim().to_ascii_lowercase();
    if CLUSTER_COLORS.contains(&color.as_str()) {
        return Ok(color);
    }
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(StructureError::InvalidColor("format"));
    }
    if BACKGROUNDS
        .iter()
        .any(|background| contrast_ratio(&color, background) < MIN_COLOR_CONTRAST)
    {
        return Err(StructureError::InvalidColor("contrast"));
    }
    Ok(color)
}

fn id_text(id: Uuid) -> String {
    id.hyphenated().to_string()
}

/// The structure of one open library.
#[derive(Debug, Clone)]
pub struct Structure {
    database: Database,
    generators: Generators,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    journal: Arc<Mutex<Journal>>,
}

impl Structure {
    pub(crate) fn new(
        database: Database,
        generators: Generators,
        write_lock: Arc<tokio::sync::Mutex<()>>,
        journal: Arc<Mutex<Journal>>,
    ) -> Self {
        Self {
            database,
            generators,
            write_lock,
            journal,
        }
    }

    fn journal(&self) -> std::sync::MutexGuard<'_, Journal> {
        self.journal.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The next steps to undo and redo.
    pub fn history(&self) -> UndoHistory {
        self.journal().history()
    }

    pub async fn subnets(&self) -> Result<Vec<SubnetRow>, StructureError> {
        Ok(rows::list_subnets(self.database.reader()).await?)
    }

    pub async fn tags(&self) -> Result<Vec<TagRow>, StructureError> {
        Ok(rows::list_tags(self.database.reader()).await?)
    }

    /// Subnets and tags of every document that has any.
    pub async fn all_documents(&self) -> Result<Vec<DocumentStructure>, StructureError> {
        let mut by_document: BTreeMap<String, (Vec<MembershipRow>, Vec<String>)> = BTreeMap::new();
        for membership in rows::all_memberships(self.database.reader()).await? {
            by_document
                .entry(membership.document_id.clone())
                .or_default()
                .0
                .push(membership);
        }
        for tag in rows::all_document_tags(self.database.reader()).await? {
            by_document
                .entry(tag.document_id)
                .or_default()
                .1
                .push(tag.tag_id);
        }
        Ok(by_document
            .into_iter()
            .map(|(id, (memberships, tag_ids))| DocumentStructure {
                id,
                subnet_ids: ordered_subnets(&memberships),
                tag_ids,
            })
            .collect())
    }

    /// Subnets and tags of one document.
    pub async fn document(&self, id: &str) -> Result<DocumentStructure, StructureError> {
        let mut connection = self.database.reader().acquire().await?;
        let memberships = rows::memberships_of(&mut connection, id).await?;
        let tags = rows::tags_of(&mut connection, id).await?;
        Ok(DocumentStructure {
            id: id.to_owned(),
            subnet_ids: ordered_subnets(&memberships),
            tag_ids: tags.into_iter().map(|tag| tag.tag_id).collect(),
        })
    }

    // --- Subnets ---------------------------------------------------------------

    /// A new subnet below `parent` (or on the top level). Without a colour
    /// it takes the parent's, on the top level the next cluster colour.
    pub async fn create_subnet(
        &self,
        name: &str,
        parent: Option<Uuid>,
        color: Option<&str>,
        description: &str,
    ) -> Result<Outcome, StructureError> {
        let name = validate_subnet_name(name)?;
        let description = validate_description(description)?;
        let color = color.map(validate_color).transpose()?;
        let id = id_text(self.generators.new_id());
        let parent = parent.map(id_text);
        let now = self.generators.now().to_iso();
        let keys = Keys {
            subnets: BTreeSet::from([id.clone()]),
            ..Keys::default()
        };
        let step = UndoStep {
            action: StructureAction::SubnetCreate,
            subject: name.clone(),
        };
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let color = match (color, &parent) {
                    (Some(color), _) => color,
                    (None, Some(parent)) => {
                        rows::get_subnet(connection, parent)
                            .await?
                            .ok_or(StructureError::SubnetNotFound)?
                            .color
                    }
                    (None, None) => {
                        let roots = rows::count_roots(connection).await?;
                        let index = usize::try_from(roots).unwrap_or(0) % CLUSTER_COLORS.len();
                        CLUSTER_COLORS[index].to_owned()
                    }
                };
                if let Some(parent) = &parent
                    && rows::get_subnet(connection, parent).await?.is_none()
                {
                    return Err(StructureError::SubnetNotFound);
                }
                let key = name_key(&name);
                if let Some(existing) =
                    rows::sibling_named(connection, parent.as_deref(), &key).await?
                {
                    return Err(StructureError::SubnetNameTaken { existing });
                }
                rows::insert_subnet(
                    connection,
                    &SubnetRow {
                        id: id.clone(),
                        name,
                        name_key: key,
                        description,
                        color,
                        parent_id: parent,
                        created_at: now.clone(),
                        updated_at: now,
                    },
                )
                .await?;
                Ok(Some(id))
            })
        })
        .await
    }

    /// Changes name, description or colour of a subnet.
    pub async fn update_subnet(
        &self,
        id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        color: Option<&str>,
    ) -> Result<Outcome, StructureError> {
        let name = name.map(validate_subnet_name).transpose()?;
        let description = description.map(validate_description).transpose()?;
        let color = color.map(validate_color).transpose()?;
        let id = id_text(id);
        let mut connection = self.database.reader().acquire().await?;
        let current = rows::get_subnet(&mut connection, &id)
            .await?
            .ok_or(StructureError::SubnetNotFound)?;
        drop(connection);
        let step = UndoStep {
            action: StructureAction::SubnetUpdate,
            subject: name.clone().unwrap_or_else(|| current.name.clone()),
        };
        let keys = Keys {
            subnets: BTreeSet::from([id.clone()]),
            ..Keys::default()
        };
        let now = self.generators.now().to_iso();
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let mut row = rows::get_subnet(connection, &id)
                    .await?
                    .ok_or(StructureError::SubnetNotFound)?;
                let before = row.clone();
                if let Some(name) = name {
                    let key = name_key(&name);
                    if let Some(existing) =
                        rows::sibling_named(connection, row.parent_id.as_deref(), &key).await?
                        && existing != id
                    {
                        return Err(StructureError::SubnetNameTaken { existing });
                    }
                    row.name = name;
                    row.name_key = key;
                }
                if let Some(description) = description {
                    row.description = description;
                }
                if let Some(color) = color {
                    row.color = color;
                }
                if row != before {
                    row.updated_at = now;
                    rows::update_subnet(connection, &row).await?;
                }
                Ok(Some(id))
            })
        })
        .await
    }

    /// Moves a subnet below `parent`, or to the top level.
    pub async fn move_subnet(
        &self,
        id: Uuid,
        parent: Option<Uuid>,
    ) -> Result<Outcome, StructureError> {
        let id = id_text(id);
        let parent = parent.map(id_text);
        let mut connection = self.database.reader().acquire().await?;
        let current = rows::get_subnet(&mut connection, &id)
            .await?
            .ok_or(StructureError::SubnetNotFound)?;
        drop(connection);
        let step = UndoStep {
            action: StructureAction::SubnetMove,
            subject: current.name,
        };
        let keys = Keys {
            subnets: BTreeSet::from([id.clone()]),
            ..Keys::default()
        };
        let now = self.generators.now().to_iso();
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let mut row = rows::get_subnet(connection, &id)
                    .await?
                    .ok_or(StructureError::SubnetNotFound)?;
                if row.parent_id == parent {
                    return Ok(Some(id));
                }
                if let Some(parent) = &parent {
                    if rows::get_subnet(connection, parent).await?.is_none() {
                        return Err(StructureError::SubnetNotFound);
                    }
                    if *parent == id || rows::descendants(connection, &id).await?.contains(parent) {
                        return Err(StructureError::Cycle);
                    }
                }
                if let Some(existing) =
                    rows::sibling_named(connection, parent.as_deref(), &row.name_key).await?
                {
                    return Err(StructureError::SubnetNameTaken { existing });
                }
                row.parent_id = parent;
                row.updated_at = now;
                rows::update_subnet(connection, &row).await?;
                Ok(Some(id))
            })
        })
        .await
    }

    /// Deletes a subnet; elements stay (section 16). Its subnets move one
    /// level up – a name taken there gets " (2)", " (3)" … – or, with
    /// `with_children`, go as well (still without elements).
    pub async fn delete_subnet(
        &self,
        id: Uuid,
        with_children: bool,
    ) -> Result<Outcome, StructureError> {
        let id = id_text(id);
        let mut connection = self.database.reader().acquire().await?;
        let current = rows::get_subnet(&mut connection, &id)
            .await?
            .ok_or(StructureError::SubnetNotFound)?;
        let below = if with_children {
            rows::descendants(&mut connection, &id).await?
        } else {
            rows::children(&mut connection, &id).await?
        };
        let mut deleted = vec![id.clone()];
        if with_children {
            deleted.extend(below.iter().cloned());
        }
        let mut documents = BTreeSet::new();
        for subnet in &deleted {
            documents.extend(rows::members_of(&mut connection, subnet).await?);
        }
        drop(connection);
        let keys = Keys {
            subnets: std::iter::once(id.clone())
                .chain(below.iter().cloned())
                .collect(),
            documents: documents.clone(),
            ..Keys::default()
        };
        let step = UndoStep {
            action: StructureAction::SubnetDelete,
            subject: current.name,
        };
        let now = self.generators.now().to_iso();
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let row = rows::get_subnet(connection, &id)
                    .await?
                    .ok_or(StructureError::SubnetNotFound)?;
                if with_children {
                    // Deepest first: no subnet is deleted while it has children.
                    for subnet in &below {
                        rows::delete_subnet(connection, subnet).await?;
                    }
                } else {
                    // The deleted subnet must not block its own name one level up.
                    rows::detach_subnet(connection, &id).await?;
                    for child in &below {
                        let Some(mut child_row) = rows::get_subnet(connection, child).await? else {
                            continue;
                        };
                        let (name, key) =
                            free_name(connection, row.parent_id.as_deref(), &child_row.name)
                                .await?;
                        child_row.name = name;
                        child_row.name_key = key;
                        child_row.parent_id.clone_from(&row.parent_id);
                        child_row.updated_at.clone_from(&now);
                        rows::update_subnet(connection, &child_row).await?;
                    }
                }
                rows::delete_subnet(connection, &id).await?;
                // Memberships went with the subnets: the primary one moves on.
                for document in &documents {
                    let memberships = rows::memberships_of(connection, document).await?;
                    rows::replace_memberships(connection, document, &with_primary(memberships))
                        .await?;
                }
                Ok(None)
            })
        })
        .await
    }

    // --- Memberships -------------------------------------------------------------

    /// Adds a membership; the first one of a document is primary.
    pub async fn assign(&self, document: Uuid, subnet: Uuid) -> Result<Outcome, StructureError> {
        let subnet = id_text(subnet);
        let now = self.generators.now().to_iso();
        self.document_change(
            document,
            StructureAction::DocumentAssign,
            move |connection, document| {
                Box::pin(async move {
                    ensure_subnet(connection, &subnet).await?;
                    let mut memberships = rows::memberships_of(connection, &document).await?;
                    if !memberships.iter().any(|m| m.subnet_id == subnet) {
                        let first = memberships.is_empty();
                        memberships.push(MembershipRow {
                            subnet_id: subnet.clone(),
                            document_id: document.clone(),
                            is_primary: first,
                            created_at: now,
                        });
                        rows::replace_memberships(connection, &document, &memberships).await?;
                    }
                    Ok(Some(subnet))
                })
            },
        )
        .await
    }

    /// Removes a membership; the oldest remaining one becomes primary.
    pub async fn unassign(&self, document: Uuid, subnet: Uuid) -> Result<Outcome, StructureError> {
        let subnet = id_text(subnet);
        self.document_change(
            document,
            StructureAction::DocumentUnassign,
            move |connection, document| {
                Box::pin(async move {
                    let memberships = rows::memberships_of(connection, &document).await?;
                    if !memberships.iter().any(|m| m.subnet_id == subnet) {
                        return Err(StructureError::NotMember);
                    }
                    let rest: Vec<MembershipRow> = memberships
                        .into_iter()
                        .filter(|m| m.subnet_id != subnet)
                        .collect();
                    rows::replace_memberships(connection, &document, &with_primary(rest)).await?;
                    Ok(Some(subnet))
                })
            },
        )
        .await
    }

    /// "Verschieben" (section 13): the document leaves its primary subnet
    /// and is primary in `target`; other memberships stay. Without a
    /// target it leaves every subnet ("Nicht zugeordnet").
    pub async fn move_document(
        &self,
        document: Uuid,
        target: Option<Uuid>,
    ) -> Result<Outcome, StructureError> {
        let target = target.map(id_text);
        let now = self.generators.now().to_iso();
        self.document_change(
            document,
            StructureAction::DocumentMove,
            move |connection, document| {
                Box::pin(async move {
                    let Some(target) = target else {
                        rows::replace_memberships(connection, &document, &[]).await?;
                        return Ok(None);
                    };
                    ensure_subnet(connection, &target).await?;
                    let memberships = rows::memberships_of(connection, &document).await?;
                    if memberships
                        .iter()
                        .any(|m| m.subnet_id == target && m.is_primary)
                    {
                        return Ok(Some(target));
                    }
                    let mut next: Vec<MembershipRow> = memberships
                        .into_iter()
                        .filter(|m| !m.is_primary || m.subnet_id == target)
                        .collect();
                    if let Some(existing) = next.iter_mut().find(|m| m.subnet_id == target) {
                        existing.is_primary = true;
                    } else {
                        next.push(MembershipRow {
                            subnet_id: target.clone(),
                            document_id: document.clone(),
                            is_primary: true,
                            created_at: now,
                        });
                    }
                    rows::replace_memberships(connection, &document, &next).await?;
                    Ok(Some(target))
                })
            },
        )
        .await
    }

    /// Makes an existing membership the primary one.
    pub async fn set_primary(
        &self,
        document: Uuid,
        subnet: Uuid,
    ) -> Result<Outcome, StructureError> {
        let subnet = id_text(subnet);
        self.document_change(
            document,
            StructureAction::DocumentPrimary,
            move |connection, document| {
                Box::pin(async move {
                    let mut memberships = rows::memberships_of(connection, &document).await?;
                    if !memberships.iter().any(|m| m.subnet_id == subnet) {
                        return Err(StructureError::NotMember);
                    }
                    for membership in &mut memberships {
                        membership.is_primary = membership.subnet_id == subnet;
                    }
                    rows::replace_memberships(connection, &document, &memberships).await?;
                    Ok(Some(subnet))
                })
            },
        )
        .await
    }

    // --- Tags ----------------------------------------------------------------------

    pub async fn create_tag(&self, name: &str) -> Result<Outcome, StructureError> {
        let name = validate_tag_name(name)?;
        let id = id_text(self.generators.new_id());
        let now = self.generators.now().to_iso();
        let step = UndoStep {
            action: StructureAction::TagCreate,
            subject: name.clone(),
        };
        let keys = Keys {
            tags: BTreeSet::from([id.clone()]),
            ..Keys::default()
        };
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let key = name_key(&name);
                if let Some(existing) = rows::tag_by_key(connection, &key).await? {
                    return Err(StructureError::TagNameTaken {
                        existing: existing.id,
                    });
                }
                rows::insert_tag(
                    connection,
                    &TagRow {
                        id: id.clone(),
                        name,
                        name_key: key,
                        created_at: now.clone(),
                        updated_at: now,
                    },
                )
                .await?;
                Ok(Some(id))
            })
        })
        .await
    }

    /// Renames a tag. A name another tag has is refused with that tag's id,
    /// so the interface can offer to merge.
    pub async fn rename_tag(&self, id: Uuid, name: &str) -> Result<Outcome, StructureError> {
        let name = validate_tag_name(name)?;
        let id = id_text(id);
        let now = self.generators.now().to_iso();
        let step = UndoStep {
            action: StructureAction::TagRename,
            subject: name.clone(),
        };
        let keys = Keys {
            tags: BTreeSet::from([id.clone()]),
            ..Keys::default()
        };
        self.change(step, keys, |connection| {
            Box::pin(async move {
                let mut row = rows::get_tag(connection, &id)
                    .await?
                    .ok_or(StructureError::TagNotFound)?;
                let key = name_key(&name);
                if let Some(existing) = rows::tag_by_key(connection, &key).await?
                    && existing.id != id
                {
                    return Err(StructureError::TagNameTaken {
                        existing: existing.id,
                    });
                }
                if row.name != name {
                    row.name = name;
                    row.name_key = key;
                    row.updated_at = now;
                    rows::update_tag(connection, &row).await?;
                }
                Ok(Some(id))
            })
        })
        .await
    }

    /// Merges `source` into `target`: its documents get `target` (once),
    /// then `source` goes. One transaction.
    pub async fn merge_tags(&self, source: Uuid, target: Uuid) -> Result<Outcome, StructureError> {
        if source == target {
            return Err(StructureError::SameTag);
        }
        let source = id_text(source);
        let target = id_text(target);
        let mut connection = self.database.reader().acquire().await?;
        let source_row = rows::get_tag(&mut connection, &source)
            .await?
            .ok_or(StructureError::TagNotFound)?;
        let target_row = rows::get_tag(&mut connection, &target)
            .await?
            .ok_or(StructureError::TagNotFound)?;
        let documents: BTreeSet<String> = rows::tagged_with(&mut connection, &source)
            .await?
            .into_iter()
            .collect();
        drop(connection);
        let step = UndoStep {
            action: StructureAction::TagMerge,
            subject: format!("{} → {}", source_row.name, target_row.name),
        };
        let keys = Keys {
            tags: BTreeSet::from([source.clone(), target.clone()]),
            documents: documents.clone(),
            ..Keys::default()
        };
        self.change(step, keys, |connection| {
            Box::pin(async move {
                for document in &documents {
                    let tags = rows::tags_of(connection, document).await?;
                    let mut next: Vec<DocumentTagRow> = Vec::new();
                    for mut tag in tags {
                        if tag.tag_id == source {
                            tag.tag_id.clone_from(&target);
                        }
                        if !next.iter().any(|t| t.tag_id == tag.tag_id) {
                            next.push(tag);
                        }
                    }
                    rows::replace_document_tags(connection, document, &next).await?;
                }
                rows::delete_tag(connection, &source).await?;
                Ok(Some(target))
            })
        })
        .await
    }

    /// Deletes a tag; documents only lose it.
    pub async fn delete_tag(&self, id: Uuid) -> Result<Outcome, StructureError> {
        let id = id_text(id);
        let mut connection = self.database.reader().acquire().await?;
        let row = rows::get_tag(&mut connection, &id)
            .await?
            .ok_or(StructureError::TagNotFound)?;
        let documents: BTreeSet<String> = rows::tagged_with(&mut connection, &id)
            .await?
            .into_iter()
            .collect();
        drop(connection);
        let step = UndoStep {
            action: StructureAction::TagDelete,
            subject: row.name,
        };
        let keys = Keys {
            tags: BTreeSet::from([id.clone()]),
            documents,
            ..Keys::default()
        };
        self.change(step, keys, |connection| {
            Box::pin(async move {
                rows::delete_tag(connection, &id).await?;
                Ok(None)
            })
        })
        .await
    }

    /// Gives a document the tag named `name`; a new name makes a new tag.
    pub async fn tag_document(
        &self,
        document: Uuid,
        name: &str,
    ) -> Result<Outcome, StructureError> {
        let name = validate_tag_name(name)?;
        let key = name_key(&name);
        let mut connection = self.database.reader().acquire().await?;
        let existing = rows::tag_by_key(&mut connection, &key).await?;
        drop(connection);
        let tag_id = existing
            .as_ref()
            .map_or_else(|| id_text(self.generators.new_id()), |tag| tag.id.clone());
        let now = self.generators.now().to_iso();
        let new_tag = existing.is_none().then(|| TagRow {
            id: tag_id.clone(),
            name,
            name_key: key,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        let tags = BTreeSet::from([tag_id.clone()]);
        self.document_change_with(
            document,
            StructureAction::DocumentTag,
            tags,
            move |connection, document| {
                Box::pin(async move {
                    if let Some(row) = &new_tag {
                        rows::insert_tag(connection, row).await?;
                    }
                    let mut tags = rows::tags_of(connection, &document).await?;
                    if !tags.iter().any(|t| t.tag_id == tag_id) {
                        tags.push(DocumentTagRow {
                            document_id: document.clone(),
                            tag_id: tag_id.clone(),
                            created_at: now,
                        });
                        rows::replace_document_tags(connection, &document, &tags).await?;
                    }
                    Ok(Some(tag_id))
                })
            },
        )
        .await
    }

    /// Takes a tag from a document; the tag itself stays.
    pub async fn untag_document(
        &self,
        document: Uuid,
        tag: Uuid,
    ) -> Result<Outcome, StructureError> {
        let tag = id_text(tag);
        self.document_change(
            document,
            StructureAction::DocumentUntag,
            move |connection, document| {
                Box::pin(async move {
                    let tags = rows::tags_of(connection, &document).await?;
                    let rest: Vec<DocumentTagRow> =
                        tags.into_iter().filter(|t| t.tag_id != tag).collect();
                    rows::replace_document_tags(connection, &document, &rest).await?;
                    Ok(Some(tag))
                })
            },
        )
        .await
    }

    // --- Trash and archive -------------------------------------------------------------

    /// Moves a document to the trash or takes it out (stage 3), as a step
    /// of the journal.
    pub async fn set_deleted(
        &self,
        document: Uuid,
        deleted: bool,
    ) -> Result<Outcome, StructureError> {
        let now = self.generators.now().to_iso();
        let action = if deleted {
            StructureAction::DocumentDelete
        } else {
            StructureAction::DocumentRestore
        };
        self.state_change(document, action, move |state| {
            if deleted {
                if state.deleted_at.is_some() {
                    return Err(StructureError::InTrash);
                }
                state.deleted_at = Some(now);
            } else {
                if state.deleted_at.is_none() {
                    return Err(StructureError::NotInTrash);
                }
                state.deleted_at = None;
            }
            Ok(())
        })
        .await
    }

    /// Archives a document or takes it out of the archive.
    pub async fn set_archived(
        &self,
        document: Uuid,
        archived: bool,
    ) -> Result<Outcome, StructureError> {
        let now = self.generators.now().to_iso();
        let action = if archived {
            StructureAction::DocumentArchive
        } else {
            StructureAction::DocumentUnarchive
        };
        self.state_change(document, action, move |state| {
            if archived {
                if state.archived_at.is_none() {
                    state.archived_at = Some(now);
                }
            } else {
                state.archived_at = None;
            }
            Ok(())
        })
        .await
    }

    // --- Without the journal ----------------------------------------------------------

    /// Gives a copy the subnets (same primary) and tags of the original
    /// (section 13). Not a step of its own: the copy is new.
    pub async fn copy_to(&self, from: Uuid, to: Uuid) -> Result<(), StructureError> {
        let from = id_text(from);
        let to = id_text(to);
        let now = self.generators.now().to_iso();
        let _guard = self.write_lock.lock().await;
        let mut transaction = self.database.writer().begin().await?;
        let memberships: Vec<MembershipRow> = rows::memberships_of(&mut transaction, &from)
            .await?
            .into_iter()
            .map(|m| MembershipRow {
                document_id: to.clone(),
                created_at: now.clone(),
                ..m
            })
            .collect();
        rows::replace_memberships(&mut transaction, &to, &memberships).await?;
        let tags: Vec<DocumentTagRow> = rows::tags_of(&mut transaction, &from)
            .await?
            .into_iter()
            .map(|t| DocumentTagRow {
                document_id: to.clone(),
                created_at: now.clone(),
                ..t
            })
            .collect();
        rows::replace_document_tags(&mut transaction, &to, &tags).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Gives a new document the tags named in its file (import of a
    /// `.pwrite`); invalid names are skipped.
    pub async fn tag_by_names(
        &self,
        document: Uuid,
        names: &[String],
    ) -> Result<(), StructureError> {
        let document = id_text(document);
        let now = self.generators.now().to_iso();
        let _guard = self.write_lock.lock().await;
        let mut transaction = self.database.writer().begin().await?;
        let mut tags = rows::tags_of(&mut transaction, &document).await?;
        for name in names {
            let Ok(name) = validate_tag_name(name) else {
                continue;
            };
            let key = name_key(&name);
            let tag_id = match rows::tag_by_key(&mut transaction, &key).await? {
                Some(tag) => tag.id,
                None => {
                    let id = id_text(self.generators.new_id());
                    rows::insert_tag(
                        &mut transaction,
                        &TagRow {
                            id: id.clone(),
                            name,
                            name_key: key,
                            created_at: now.clone(),
                            updated_at: now.clone(),
                        },
                    )
                    .await?;
                    id
                }
            };
            if !tags.iter().any(|t| t.tag_id == tag_id) {
                tags.push(DocumentTagRow {
                    document_id: document.clone(),
                    tag_id,
                    created_at: now.clone(),
                });
            }
        }
        rows::replace_document_tags(&mut transaction, &document, &tags).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Starts over (another library opened).
    pub fn clear_history(&self) {
        self.journal().clear();
    }

    // --- Undo ----------------------------------------------------------------------------

    /// Undoes the last step; `None` if there is none.
    pub async fn undo(&self) -> Result<Option<Outcome>, StructureError> {
        self.replay(true).await
    }

    /// Redoes the last undone step.
    pub async fn redo(&self) -> Result<Option<Outcome>, StructureError> {
        self.replay(false).await
    }

    async fn replay(&self, undo: bool) -> Result<Option<Outcome>, StructureError> {
        let _guard = self.write_lock.lock().await;
        let change = {
            let mut journal = self.journal();
            let stack = if undo {
                &mut journal.undo
            } else {
                &mut journal.redo
            };
            stack.pop()
        };
        let Some(change) = change else {
            return Ok(None);
        };
        let (expected, target) = if undo {
            (&change.after, &change.before)
        } else {
            (&change.before, &change.after)
        };
        let result = async {
            let mut transaction = self.database.writer().begin().await?;
            let current = capture(&mut transaction, &change.keys).await?;
            if current != *expected {
                return Err(StructureError::Changed);
            }
            apply(&mut transaction, &change.keys, target).await?;
            transaction.commit().await?;
            Ok::<_, StructureError>(())
        }
        .await;
        if let Err(error) = result {
            // A step that no longer fits takes the rest of the journal with it.
            log::warn!(
                "structural {} refused: {error}",
                if undo { "undo" } else { "redo" }
            );
            self.journal().clear();
            return Err(match error {
                StructureError::Database(_) => StructureError::Changed,
                other => other,
            });
        }
        let outcome = Outcome {
            step: Some(change.step.clone()),
            documents: change.keys.documents.iter().cloned().collect(),
            subject_id: None,
        };
        let mut journal = self.journal();
        if undo {
            journal.redo.push(change);
        } else {
            journal.undo.push(change);
        }
        Ok(Some(outcome))
    }

    // --- Running a step ------------------------------------------------------------------

    /// Runs `operation` in a transaction between two snapshots of `keys` and
    /// records the step if anything changed.
    async fn change<F>(
        &self,
        step: UndoStep,
        keys: Keys,
        operation: F,
    ) -> Result<Outcome, StructureError>
    where
        F: for<'c> FnOnce(
            &'c mut SqliteConnection,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Option<String>, StructureError>>
                    + Send
                    + 'c,
            >,
        >,
    {
        let _guard = self.write_lock.lock().await;
        let mut transaction = self.database.writer().begin().await?;
        let before = capture(&mut transaction, &keys).await?;
        let subject_id = operation(&mut transaction).await?;
        let after = capture(&mut transaction, &keys).await?;
        transaction.commit().await?;
        let changed = before != after;
        let documents = keys.documents.iter().cloned().collect();
        if changed {
            self.journal().record(Change {
                step: step.clone(),
                keys,
                before,
                after,
            });
        }
        Ok(Outcome {
            step: changed.then_some(step),
            documents,
            subject_id,
        })
    }

    /// A step on one document that must exist and not be in the trash.
    async fn document_change<F>(
        &self,
        document: Uuid,
        action: StructureAction,
        operation: F,
    ) -> Result<Outcome, StructureError>
    where
        F: for<'c> FnOnce(
            &'c mut SqliteConnection,
            String,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Option<String>, StructureError>>
                    + Send
                    + 'c,
            >,
        >,
    {
        self.document_change_with(document, action, BTreeSet::new(), operation)
            .await
    }

    async fn document_change_with<F>(
        &self,
        document: Uuid,
        action: StructureAction,
        tags: BTreeSet<String>,
        operation: F,
    ) -> Result<Outcome, StructureError>
    where
        F: for<'c> FnOnce(
            &'c mut SqliteConnection,
            String,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Option<String>, StructureError>>
                    + Send
                    + 'c,
            >,
        >,
    {
        let document = id_text(document);
        let title = self.live_document_title(&document).await?;
        let keys = Keys {
            tags,
            documents: BTreeSet::from([document.clone()]),
            ..Keys::default()
        };
        let step = UndoStep {
            action,
            subject: title,
        };
        self.change(step, keys, move |connection| {
            operation(connection, document)
        })
        .await
    }

    async fn state_change<F>(
        &self,
        document: Uuid,
        action: StructureAction,
        update: F,
    ) -> Result<Outcome, StructureError>
    where
        F: FnOnce(&mut DocumentState) -> Result<(), StructureError> + Send + 'static,
    {
        let document = id_text(document);
        let mut connection = self.database.reader().acquire().await?;
        let title = rows::document_title(&mut connection, &document)
            .await?
            .ok_or(StructureError::DocumentNotFound)?;
        drop(connection);
        let keys = Keys {
            documents: BTreeSet::from([document.clone()]),
            ..Keys::default()
        };
        let step = UndoStep {
            action,
            subject: title,
        };
        self.change(step, keys, move |connection| {
            Box::pin(async move {
                let mut state = rows::document_state(connection, &document)
                    .await?
                    .ok_or(StructureError::DocumentNotFound)?;
                update(&mut state)?;
                rows::set_document_state(connection, &document, &state).await?;
                Ok(None)
            })
        })
        .await
    }

    /// Title of a document that exists and is not in the trash.
    async fn live_document_title(&self, document: &str) -> Result<String, StructureError> {
        let mut connection = self.database.reader().acquire().await?;
        let title = rows::document_title(&mut connection, document)
            .await?
            .ok_or(StructureError::DocumentNotFound)?;
        let state = rows::document_state(&mut connection, document)
            .await?
            .ok_or(StructureError::DocumentNotFound)?;
        if state.deleted_at.is_some() {
            return Err(StructureError::InTrash);
        }
        Ok(title)
    }
}

/// The primary subnet first, then the others by age.
fn ordered_subnets(memberships: &[MembershipRow]) -> Vec<String> {
    let mut ids: Vec<String> = memberships
        .iter()
        .filter(|m| m.is_primary)
        .map(|m| m.subnet_id.clone())
        .collect();
    ids.extend(
        memberships
            .iter()
            .filter(|m| !m.is_primary)
            .map(|m| m.subnet_id.clone()),
    );
    ids
}

/// Exactly one primary membership: if none is left, the oldest becomes it
/// (`memberships` oldest first, as [`rows::memberships_of`] returns them).
fn with_primary(mut memberships: Vec<MembershipRow>) -> Vec<MembershipRow> {
    if !memberships.iter().any(|m| m.is_primary)
        && let Some(oldest) = memberships.first_mut()
    {
        oldest.is_primary = true;
    }
    memberships
}

async fn ensure_subnet(connection: &mut SqliteConnection, id: &str) -> Result<(), StructureError> {
    rows::get_subnet(connection, id)
        .await?
        .map(|_| ())
        .ok_or(StructureError::SubnetNotFound)
}

/// `name`, or `name (2)`, `name (3)` … if a subnet below `parent` has it.
async fn free_name(
    connection: &mut SqliteConnection,
    parent: Option<&str>,
    name: &str,
) -> Result<(String, String), StructureError> {
    let key = name_key(name);
    if rows::sibling_named(connection, parent, &key)
        .await?
        .is_none()
    {
        return Ok((name.to_owned(), key));
    }
    for number in 2..10_000 {
        let suffix = format!(" ({number})");
        let room = MAX_SUBNET_NAME_CHARS - suffix.chars().count();
        let base: String = name.chars().take(room).collect();
        let candidate = format!("{}{suffix}", base.trim_end());
        let key = name_key(&candidate);
        if rows::sibling_named(connection, parent, &key)
            .await?
            .is_none()
        {
            return Ok((candidate, key));
        }
    }
    Err(StructureError::InvalidName("no free name"))
}

async fn capture(
    connection: &mut SqliteConnection,
    keys: &Keys,
) -> Result<Snapshot, StructureError> {
    let mut snapshot = Snapshot::default();
    for id in &keys.subnets {
        snapshot
            .subnets
            .insert(id.clone(), rows::get_subnet(connection, id).await?);
    }
    for id in &keys.tags {
        snapshot
            .tags
            .insert(id.clone(), rows::get_tag(connection, id).await?);
    }
    for id in &keys.documents {
        let document = match rows::document_state(connection, id).await? {
            Some(state) => Some(DocumentSnapshot {
                memberships: rows::memberships_of(connection, id).await?,
                tags: rows::tags_of(connection, id).await?,
                state,
            }),
            None => None,
        };
        snapshot.documents.insert(id.clone(), document);
    }
    Ok(snapshot)
}

/// Writes `target` for the rows of `keys` (see the module documentation).
async fn apply(
    connection: &mut SqliteConnection,
    keys: &Keys,
    target: &Snapshot,
) -> Result<(), StructureError> {
    // Documents removed for good cannot come back.
    for id in &keys.documents {
        if target.documents.get(id).is_some_and(Option::is_some)
            && rows::document_state(connection, id).await?.is_none()
        {
            return Err(StructureError::Changed);
        }
    }

    // Subnets: detach, delete what the target lacks, insert what it has
    // and is missing (detached), then write every row as the target has it.
    let mut existing = BTreeSet::new();
    for id in &keys.subnets {
        if rows::get_subnet(connection, id).await?.is_some() {
            rows::detach_subnet(connection, id).await?;
            existing.insert(id.clone());
        }
    }
    for id in &existing {
        if !target.subnets.get(id).is_some_and(Option::is_some) {
            rows::delete_subnet(connection, id).await?;
        }
    }
    for (id, row) in &target.subnets {
        if let Some(row) = row
            && !existing.contains(id)
        {
            let detached = SubnetRow {
                parent_id: None,
                name_key: format!("{}\u{1f}{}", row.name_key, row.id),
                ..row.clone()
            };
            rows::insert_subnet(connection, &detached).await?;
        }
    }
    for row in target.subnets.values().flatten() {
        rows::update_subnet(connection, row).await?;
    }

    // Tags: the same way.
    let mut existing = BTreeSet::new();
    for id in &keys.tags {
        if rows::get_tag(connection, id).await?.is_some() {
            rows::detach_tag(connection, id).await?;
            existing.insert(id.clone());
        }
    }
    for id in &existing {
        if !target.tags.get(id).is_some_and(Option::is_some) {
            rows::delete_tag(connection, id).await?;
        }
    }
    for (id, row) in &target.tags {
        if let Some(row) = row
            && !existing.contains(id)
        {
            let detached = TagRow {
                name_key: format!("{}\u{1f}{}", row.name_key, row.id),
                ..row.clone()
            };
            rows::insert_tag(connection, &detached).await?;
        }
    }
    for row in target.tags.values().flatten() {
        rows::update_tag(connection, row).await?;
    }

    // Documents: memberships, tags and state as they were.
    for (id, document) in &target.documents {
        let Some(document) = document else {
            continue;
        };
        rows::replace_memberships(connection, id, &document.memberships).await?;
        rows::replace_document_tags(connection, id, &document.tags).await?;
        rows::set_document_state(connection, id, &document.state).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked_and_keyed_without_case() {
        assert_eq!(validate_subnet_name("  Roman ").unwrap(), "Roman");
        assert!(validate_subnet_name("").is_err());
        assert!(validate_subnet_name(&"x".repeat(201)).is_err());
        assert!(validate_subnet_name("a\u{7}b").is_err());
        assert_eq!(name_key("Äpfel"), name_key("äPFEL"));
        assert!(validate_tag_name("a, b").is_err());
        assert_eq!(validate_tag_name(" Recherche ").unwrap(), "Recherche");
        assert!(validate_description("Zeile 1\nZeile 2\tEnde").is_ok());
        assert!(validate_description("a\u{0}").is_err());
    }

    #[test]
    fn colours_are_tokens_or_reach_three_to_one_on_every_background() {
        assert_eq!(validate_color("Blue").unwrap(), "blue");
        assert!(matches!(
            validate_color("#fff"),
            Err(StructureError::InvalidColor("format"))
        ));
        // White vanishes on the light design, black on the dark one.
        assert!(matches!(
            validate_color("#FFFFFF"),
            Err(StructureError::InvalidColor("contrast"))
        ));
        assert!(matches!(
            validate_color("#000000"),
            Err(StructureError::InvalidColor("contrast"))
        ));
        // A middle tone works on both.
        let color = validate_color("#D9480F").unwrap();
        assert_eq!(color, "#d9480f");
        for background in BACKGROUNDS {
            assert!(contrast_ratio(&color, background) >= MIN_COLOR_CONTRAST);
        }
    }

    #[test]
    fn the_oldest_membership_becomes_primary() {
        let row = |subnet: &str, primary: bool| MembershipRow {
            subnet_id: subnet.to_owned(),
            document_id: "d".to_owned(),
            is_primary: primary,
            created_at: String::new(),
        };
        let rows = with_primary(vec![row("a", false), row("b", false)]);
        assert!(rows[0].is_primary && !rows[1].is_primary);
        let rows = with_primary(vec![row("a", false), row("b", true)]);
        assert!(!rows[0].is_primary && rows[1].is_primary);
        assert_eq!(ordered_subnets(&rows), ["b", "a"]);
        assert!(with_primary(Vec::new()).is_empty());
    }
}
