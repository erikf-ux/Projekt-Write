//! Subnets, memberships, tags and the structural undo (Phase 7, ADR 0017).
//!
//! Every changing command answers with a [`StructureUpdate`]: all subnets
//! and tags (a few hundred rows; the Explorer redraws from them), the
//! structure of the documents involved, and the next undo and redo step.

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;
use uuid::Uuid;

use crate::commands::input::{EmptyRequest, Input, Validate};
use crate::database::structure::{SubnetRow, TagRow};
use crate::error::{CommandError, ErrorCode};
use crate::services::structure::{
    self, DocumentStructure, Outcome, Structure, StructureError, UndoHistory, UndoStep,
};
use crate::state::AppState;

/// A subnet (section 16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubnetSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A cluster token (`blue`, `green`, `violet`, `orange`, `grey`) or `#rrggbb`.
    pub color: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SubnetRow> for SubnetSummary {
    fn from(row: SubnetRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            color: row.color,
            parent_id: row.parent_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A tag (section 13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TagSummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TagRow> for TagSummary {
    fn from(row: TagRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Result of `structure_state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StructureState {
    pub subnets: Vec<SubnetSummary>,
    pub tags: Vec<TagSummary>,
    /// Documents with at least one subnet or tag (also in the trash).
    pub documents: Vec<DocumentStructure>,
    pub history: UndoHistory,
}

/// Result of a structural action and of undo and redo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StructureUpdate {
    pub subnets: Vec<SubnetSummary>,
    pub tags: Vec<TagSummary>,
    /// The documents involved, as they are now.
    pub documents: Vec<DocumentStructure>,
    pub history: UndoHistory,
    /// What was done (or undone, redone); `None` if nothing changed.
    pub step: Option<UndoStep>,
    /// The subnet or tag created or worked on.
    pub subject_id: Option<String>,
}

fn parse_uuid(field: &str, id: &str, what: &str) -> Result<Uuid, CommandError> {
    Uuid::try_parse(id)
        .ok()
        .filter(|uuid| uuid.hyphenated().to_string() == id)
        .ok_or_else(|| CommandError::invalid_input(field, format!("not a {what} id")))
}

fn parse_optional(
    field: &str,
    id: Option<&String>,
    what: &str,
) -> Result<Option<Uuid>, CommandError> {
    id.map(|id| parse_uuid(field, id, what)).transpose()
}

fn check<T>(result: Result<T, StructureError>) -> Result<(), CommandError> {
    result.map(|_| ()).map_err(structure_error)
}

pub(crate) fn structure_error(error: StructureError) -> CommandError {
    match error {
        StructureError::SubnetNotFound => CommandError::new(ErrorCode::SubnetNotFound),
        StructureError::TagNotFound => CommandError::new(ErrorCode::TagNotFound),
        StructureError::DocumentNotFound | StructureError::InTrash => {
            CommandError::new(ErrorCode::DocumentNotFound)
        }
        StructureError::NotInTrash => CommandError::invalid_input("id", "not in the trash"),
        StructureError::Cycle => CommandError::new(ErrorCode::SubnetCycle),
        StructureError::SubnetNameTaken { existing } => {
            CommandError::new(ErrorCode::SubnetNameTaken).with_detail("existing", &existing)
        }
        StructureError::TagNameTaken { existing } => {
            CommandError::new(ErrorCode::TagNameTaken).with_detail("existing", &existing)
        }
        StructureError::InvalidName(reason) => CommandError::invalid_input("name", reason),
        StructureError::InvalidDescription(reason) => {
            CommandError::invalid_input("description", reason)
        }
        StructureError::InvalidColor(reason) => CommandError::invalid_input("color", reason),
        StructureError::NotMember => CommandError::invalid_input("subnetId", "not a member"),
        StructureError::SameTag => CommandError::invalid_input("targetId", "same tag"),
        StructureError::Changed => CommandError::new(ErrorCode::StructureChanged),
        StructureError::Database(error) => {
            log::error!("structure database error: {error}");
            CommandError::new(ErrorCode::Database)
        }
    }
}

/// A `.pwrite` taken into the library brings the tags of its envelope
/// (ADR 0017, decision 7). A failure here leaves the document untagged.
pub(crate) async fn take_tags(
    structure: &Structure,
    id: Uuid,
    format: crate::services::documents::DocumentFormat,
    content: &[u8],
) {
    if format != crate::services::documents::DocumentFormat::Pwrite {
        return;
    }
    let Ok(crate::services::pwrite::Parsed::Current(file)) =
        crate::services::pwrite::parse(content)
    else {
        return;
    };
    if file.tags.is_empty() {
        return;
    }
    if let Err(error) = structure.tag_by_names(id, &file.tags).await {
        log::warn!("tags of imported document {id} not taken over: {error}");
    }
}

async fn update_from(
    structure: &Structure,
    outcome: Outcome,
) -> Result<StructureUpdate, CommandError> {
    let mut documents = Vec::new();
    for id in &outcome.documents {
        documents.push(structure.document(id).await.map_err(structure_error)?);
    }
    Ok(StructureUpdate {
        subnets: structure
            .subnets()
            .await
            .map_err(structure_error)?
            .into_iter()
            .map(SubnetSummary::from)
            .collect(),
        tags: structure
            .tags()
            .await
            .map_err(structure_error)?
            .into_iter()
            .map(TagSummary::from)
            .collect(),
        documents,
        history: structure.history(),
        step: outcome.step,
        subject_id: outcome.subject_id,
    })
}

/// Everything the Explorer needs at the start.
#[tauri::command]
pub async fn structure_state(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<StructureState, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_structure(|structure, _| async move {
            Ok(StructureState {
                subnets: structure
                    .subnets()
                    .await
                    .map_err(structure_error)?
                    .into_iter()
                    .map(SubnetSummary::from)
                    .collect(),
                tags: structure
                    .tags()
                    .await
                    .map_err(structure_error)?
                    .into_iter()
                    .map(TagSummary::from)
                    .collect(),
                documents: structure.all_documents().await.map_err(structure_error)?,
                history: structure.history(),
            })
        })
        .await
}

/// Request naming one document.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StructureDocumentRequest {
    pub id: String,
}

impl Validate for StructureDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "document").map(|_| ())
    }
}

/// Subnets and tags of one document (after copying or importing it).
#[tauri::command]
pub async fn structure_document(
    state: State<'_, AppState>,
    request: Input<StructureDocumentRequest>,
) -> Result<DocumentStructure, CommandError> {
    let id = request.into_inner().id;
    state
        .session
        .with_structure(|structure, _| async move {
            structure.document(&id).await.map_err(structure_error)
        })
        .await
}

// --- Subnets ---------------------------------------------------------------------

/// Request of `subnet_create`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateSubnetRequest {
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub parent_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub color: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
}

impl Validate for CreateSubnetRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_optional("parentId", self.parent_id.as_ref(), "subnet")?;
        check(structure::validate_subnet_name(&self.name))?;
        if let Some(color) = &self.color {
            check(structure::validate_color(color))?;
        }
        if let Some(description) = &self.description {
            check(structure::validate_description(description))?;
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn subnet_create(
    state: State<'_, AppState>,
    request: Input<CreateSubnetRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let parent = parse_optional("parentId", request.parent_id.as_ref(), "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .create_subnet(
                    &request.name,
                    parent,
                    request.color.as_deref(),
                    request.description.as_deref().unwrap_or(""),
                )
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// Request of `subnet_update`: the fields given change.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdateSubnetRequest {
    pub id: String,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub color: Option<String>,
}

impl Validate for UpdateSubnetRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "subnet")?;
        if let Some(name) = &self.name {
            check(structure::validate_subnet_name(name))?;
        }
        if let Some(description) = &self.description {
            check(structure::validate_description(description))?;
        }
        if let Some(color) = &self.color {
            check(structure::validate_color(color))?;
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn subnet_update(
    state: State<'_, AppState>,
    request: Input<UpdateSubnetRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id, "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .update_subnet(
                    id,
                    request.name.as_deref(),
                    request.description.as_deref(),
                    request.color.as_deref(),
                )
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// Request of `subnet_move`: no parent means the top level.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MoveSubnetRequest {
    pub id: String,
    pub parent_id: Option<String>,
}

impl Validate for MoveSubnetRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "subnet")?;
        parse_optional("parentId", self.parent_id.as_ref(), "subnet")?;
        Ok(())
    }
}

#[tauri::command]
pub async fn subnet_move(
    state: State<'_, AppState>,
    request: Input<MoveSubnetRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id, "subnet")?;
    let parent = parse_optional("parentId", request.parent_id.as_ref(), "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .move_subnet(id, parent)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// Request of `subnet_delete`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DeleteSubnetRequest {
    pub id: String,
    /// Delete its subnets too (otherwise they move one level up).
    pub with_children: bool,
}

impl Validate for DeleteSubnetRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "subnet").map(|_| ())
    }
}

/// Deletes a subnet; elements are never deleted (section 16).
#[tauri::command]
pub async fn subnet_delete(
    state: State<'_, AppState>,
    request: Input<DeleteSubnetRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id, "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .delete_subnet(id, request.with_children)
                .await
                .map_err(structure_error)?;
            log::info!("subnet {id} deleted");
            update_from(&structure, outcome).await
        })
        .await
}

// --- Memberships -----------------------------------------------------------------

/// A document and a subnet.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MembershipRequest {
    pub id: String,
    pub subnet_id: String,
}

impl Validate for MembershipRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "document")?;
        parse_uuid("subnetId", &self.subnet_id, "subnet")?;
        Ok(())
    }
}

/// Request of `document_move`: no subnet means "Nicht zugeordnet".
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MoveDocumentRequest {
    pub id: String,
    pub subnet_id: Option<String>,
}

impl Validate for MoveDocumentRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "document")?;
        parse_optional("subnetId", self.subnet_id.as_ref(), "subnet")?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum MembershipChange {
    Assign,
    Unassign,
    Primary,
}

async fn change_membership(
    state: &AppState,
    request: MembershipRequest,
    change: MembershipChange,
) -> Result<StructureUpdate, CommandError> {
    let document = parse_uuid("id", &request.id, "document")?;
    let subnet = parse_uuid("subnetId", &request.subnet_id, "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = match change {
                MembershipChange::Assign => structure.assign(document, subnet).await,
                MembershipChange::Unassign => structure.unassign(document, subnet).await,
                MembershipChange::Primary => structure.set_primary(document, subnet).await,
            }
            .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// "Zuordnen zu …": adds a subnet; the first one is primary.
#[tauri::command]
pub async fn document_assign(
    state: State<'_, AppState>,
    request: Input<MembershipRequest>,
) -> Result<StructureUpdate, CommandError> {
    change_membership(&state, request.into_inner(), MembershipChange::Assign).await
}

/// "Aus „X“ entfernen".
#[tauri::command]
pub async fn document_unassign(
    state: State<'_, AppState>,
    request: Input<MembershipRequest>,
) -> Result<StructureUpdate, CommandError> {
    change_membership(&state, request.into_inner(), MembershipChange::Unassign).await
}

/// "Als primär festlegen".
#[tauri::command]
pub async fn document_set_primary(
    state: State<'_, AppState>,
    request: Input<MembershipRequest>,
) -> Result<StructureUpdate, CommandError> {
    change_membership(&state, request.into_inner(), MembershipChange::Primary).await
}

/// "Verschieben nach …": changes the primary subnet (section 13).
#[tauri::command]
pub async fn document_move(
    state: State<'_, AppState>,
    request: Input<MoveDocumentRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let document = parse_uuid("id", &request.id, "document")?;
    let subnet = parse_optional("subnetId", request.subnet_id.as_ref(), "subnet")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .move_document(document, subnet)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

// --- Tags ------------------------------------------------------------------------

/// A tag name.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TagNameRequest {
    pub name: String,
}

impl Validate for TagNameRequest {
    fn validate(&self) -> Result<(), CommandError> {
        check(structure::validate_tag_name(&self.name))
    }
}

/// One tag.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TagRequest {
    pub id: String,
}

impl Validate for TagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "tag").map(|_| ())
    }
}

/// Request of `tag_rename`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RenameTagRequest {
    pub id: String,
    pub name: String,
}

impl Validate for RenameTagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "tag")?;
        check(structure::validate_tag_name(&self.name))
    }
}

/// Request of `tag_merge`: `source` goes into `target`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MergeTagsRequest {
    pub source_id: String,
    pub target_id: String,
}

impl Validate for MergeTagsRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("sourceId", &self.source_id, "tag")?;
        parse_uuid("targetId", &self.target_id, "tag")?;
        if self.source_id == self.target_id {
            return Err(CommandError::invalid_input("targetId", "same tag"));
        }
        Ok(())
    }
}

/// Request of `document_tag`: a tag by name (created if new).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DocumentTagRequest {
    pub id: String,
    pub name: String,
}

impl Validate for DocumentTagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "document")?;
        check(structure::validate_tag_name(&self.name))
    }
}

/// Request of `document_untag`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DocumentUntagRequest {
    pub id: String,
    pub tag_id: String,
}

impl Validate for DocumentUntagRequest {
    fn validate(&self) -> Result<(), CommandError> {
        parse_uuid("id", &self.id, "document")?;
        parse_uuid("tagId", &self.tag_id, "tag")?;
        Ok(())
    }
}

#[tauri::command]
pub async fn tag_create(
    state: State<'_, AppState>,
    request: Input<TagNameRequest>,
) -> Result<StructureUpdate, CommandError> {
    let name = request.into_inner().name;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure.create_tag(&name).await.map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// Renames a tag; a name another tag has fails with `TAG_NAME_TAKEN` and
/// that tag's id, so the interface can offer to merge.
#[tauri::command]
pub async fn tag_rename(
    state: State<'_, AppState>,
    request: Input<RenameTagRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let id = parse_uuid("id", &request.id, "tag")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .rename_tag(id, &request.name)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

#[tauri::command]
pub async fn tag_merge(
    state: State<'_, AppState>,
    request: Input<MergeTagsRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let source = parse_uuid("sourceId", &request.source_id, "tag")?;
    let target = parse_uuid("targetId", &request.target_id, "tag")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .merge_tags(source, target)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

/// Deletes a tag; documents only lose it.
#[tauri::command]
pub async fn tag_delete(
    state: State<'_, AppState>,
    request: Input<TagRequest>,
) -> Result<StructureUpdate, CommandError> {
    let id = parse_uuid("id", &request.into_inner().id, "tag")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure.delete_tag(id).await.map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

#[tauri::command]
pub async fn document_tag(
    state: State<'_, AppState>,
    request: Input<DocumentTagRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let document = parse_uuid("id", &request.id, "document")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .tag_document(document, &request.name)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

#[tauri::command]
pub async fn document_untag(
    state: State<'_, AppState>,
    request: Input<DocumentUntagRequest>,
) -> Result<StructureUpdate, CommandError> {
    let request = request.into_inner();
    let document = parse_uuid("id", &request.id, "document")?;
    let tag = parse_uuid("tagId", &request.tag_id, "tag")?;
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = structure
                .untag_document(document, tag)
                .await
                .map_err(structure_error)?;
            update_from(&structure, outcome).await
        })
        .await
}

// --- Undo ------------------------------------------------------------------------

async fn replay(state: &AppState, undo: bool) -> Result<Option<StructureUpdate>, CommandError> {
    state
        .session
        .with_structure(|structure, _| async move {
            let outcome = if undo {
                structure.undo().await
            } else {
                structure.redo().await
            };
            match outcome {
                Ok(Some(outcome)) => update_from(&structure, outcome).await.map(Some),
                Ok(None) => Ok(None),
                Err(error) => Err(structure_error(error)),
            }
        })
        .await
}

/// The next steps to undo and redo (after an action of another module,
/// such as moving a document to the trash).
#[tauri::command]
pub async fn structure_history(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<UndoHistory, CommandError> {
    let _ = request.into_inner();
    state
        .session
        .with_structure(|structure, _| async move { Ok(structure.history()) })
        .await
}

/// Undoes the last structural step (`null` if there is none).
#[tauri::command]
pub async fn structure_undo(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Option<StructureUpdate>, CommandError> {
    let _ = request.into_inner();
    replay(&state, true).await
}

/// Redoes the last undone structural step (`null` if there is none).
#[tauri::command]
pub async fn structure_redo(
    state: State<'_, AppState>,
    request: Input<EmptyRequest>,
) -> Result<Option<StructureUpdate>, CommandError> {
    let _ = request.into_inner();
    replay(&state, false).await
}
