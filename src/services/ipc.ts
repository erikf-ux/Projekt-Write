/**
 * The only module that talks to Tauri (enforced by ESLint).
 *
 * * Commands are typed: request and response types are generated from Rust
 *   (`src/types/generated`, ts-rs). Nothing is maintained twice.
 * * Every failure arrives as a `CommandError` (`{ code, messageKey, details? }`).
 *   Rejections that do not come from a command – for example a call the
 *   capability does not allow – are normalised to `IPC_REJECTED`; their raw
 *   text is discarded because it may contain internals.
 */
import { Channel, invoke } from '@tauri-apps/api/core';
import type { ChooseLocationRequest } from '@/types/generated/ChooseLocationRequest';
import type { CloseResponseRequest } from '@/types/generated/CloseResponseRequest';
import type { CommandError } from '@/types/generated/CommandError';
import type { ArchiveDocumentRequest } from '@/types/generated/ArchiveDocumentRequest';
import type { ConvertDocumentRequest } from '@/types/generated/ConvertDocumentRequest';
import type { CopyDocumentRequest } from '@/types/generated/CopyDocumentRequest';
import type { CreateDocumentRequest } from '@/types/generated/CreateDocumentRequest';
import type { CreateLibraryRequest } from '@/types/generated/CreateLibraryRequest';
import type { DocumentRequest } from '@/types/generated/DocumentRequest';
import type { DocumentSummary } from '@/types/generated/DocumentSummary';
import type { DocumentText } from '@/types/generated/DocumentText';
import type { EmptyRequest } from '@/types/generated/EmptyRequest';
import type { ErrorCode } from '@/types/generated/ErrorCode';
import type { FocusModeRequest } from '@/types/generated/FocusModeRequest';
import type { ImportDocumentRequest } from '@/types/generated/ImportDocumentRequest';
import type { ImportResult } from '@/types/generated/ImportResult';
import type { ImportThemeRequest } from '@/types/generated/ImportThemeRequest';
import type { LibraryStatus } from '@/types/generated/LibraryStatus';
import type { ImportCommitRequest } from '@/types/generated/ImportCommitRequest';
import type { ImportCancelRequest } from '@/types/generated/ImportCancelRequest';
import type { ImportOutcome } from '@/types/generated/ImportOutcome';
import type { ImportPreview } from '@/types/generated/ImportPreview';
import type { ImportProgress } from '@/types/generated/ImportProgress';
import type { ImportStartRequest } from '@/types/generated/ImportStartRequest';
import type { MediaFlagRequest } from '@/types/generated/MediaFlagRequest';
import type { MediaIdRequest } from '@/types/generated/MediaIdRequest';
import type { MediaSummary } from '@/types/generated/MediaSummary';
import type { MediaUse } from '@/types/generated/MediaUse';
import type { PageCountRequest } from '@/types/generated/PageCountRequest';
import type { PurgeReport } from '@/types/generated/PurgeReport';
import type { StoreThumbnailRequest } from '@/types/generated/StoreThumbnailRequest';
import type { UpdateMediaRequest } from '@/types/generated/UpdateMediaRequest';
import type { LibraryPreferences } from '@/types/generated/LibraryPreferences';
import type { LocationChoice } from '@/types/generated/LocationChoice';
import type { MenuStateRequest } from '@/types/generated/MenuStateRequest';
import type { OpenLinkRequest } from '@/types/generated/OpenLinkRequest';
import type { SetFavoriteRequest } from '@/types/generated/SetFavoriteRequest';
import type { MoveLibraryRequest } from '@/types/generated/MoveLibraryRequest';
import type { MoveProgress } from '@/types/generated/MoveProgress';
import type { OpenLibraryRequest } from '@/types/generated/OpenLibraryRequest';
import type { RemoveThemeRequest } from '@/types/generated/RemoveThemeRequest';
import type { RenameDocumentRequest } from '@/types/generated/RenameDocumentRequest';
import type { SaveDocumentRequest } from '@/types/generated/SaveDocumentRequest';
import type { ExternalFileInfo } from '@/types/generated/ExternalFileInfo';
import type { ExternalFileRequest } from '@/types/generated/ExternalFileRequest';
import type { ExternalText } from '@/types/generated/ExternalText';
import type { FileCheck } from '@/types/generated/FileCheck';
import type { ImportExternalRequest } from '@/types/generated/ImportExternalRequest';
import type { OpenExternalRequest } from '@/types/generated/OpenExternalRequest';
import type { OpenTarget } from '@/types/generated/OpenTarget';
import type { RecentFileEntry } from '@/types/generated/RecentFileEntry';
import type { SaveExternalAsRequest } from '@/types/generated/SaveExternalAsRequest';
import type { SaveExternalRequest } from '@/types/generated/SaveExternalRequest';
import type { SaveOutcome } from '@/types/generated/SaveOutcome';
import type { SaveVersionRequest } from '@/types/generated/SaveVersionRequest';
import type { DiscardDraftRequest } from '@/types/generated/DiscardDraftRequest';
import type { DraftEntry } from '@/types/generated/DraftEntry';
import type { DraftRequest } from '@/types/generated/DraftRequest';
import type { RestoredDraft } from '@/types/generated/RestoredDraft';
import type { RestoredSession } from '@/types/generated/RestoredSession';
import type { SaveSessionRequest } from '@/types/generated/SaveSessionRequest';
import type { SystemCheckRequest } from '@/types/generated/SystemCheckRequest';
import type { WriteDraftRequest } from '@/types/generated/WriteDraftRequest';
import type { CreateSubnetRequest } from '@/types/generated/CreateSubnetRequest';
import type { DeleteSubnetRequest } from '@/types/generated/DeleteSubnetRequest';
import type { DocumentStructure } from '@/types/generated/DocumentStructure';
import type { DocumentTagRequest } from '@/types/generated/DocumentTagRequest';
import type { DocumentUntagRequest } from '@/types/generated/DocumentUntagRequest';
import type { MembershipRequest } from '@/types/generated/MembershipRequest';
import type { MergeTagsRequest } from '@/types/generated/MergeTagsRequest';
import type { MoveDocumentRequest } from '@/types/generated/MoveDocumentRequest';
import type { MoveSubnetRequest } from '@/types/generated/MoveSubnetRequest';
import type { RenameTagRequest } from '@/types/generated/RenameTagRequest';
import type { StructureDocumentRequest } from '@/types/generated/StructureDocumentRequest';
import type { StructureState } from '@/types/generated/StructureState';
import type { StructureUpdate } from '@/types/generated/StructureUpdate';
import type { TagNameRequest } from '@/types/generated/TagNameRequest';
import type { TagRequest } from '@/types/generated/TagRequest';
import type { UndoHistory } from '@/types/generated/UndoHistory';
import type { RebuildReport } from '@/types/generated/RebuildReport';
import type { SearchHit } from '@/types/generated/SearchHit';
import type { BackupOptionsRequest } from '@/types/generated/BackupOptionsRequest';
import type { BackupOverview } from '@/types/generated/BackupOverview';
import type { BackupProgress } from '@/types/generated/BackupProgress';
import type { BackupSummary } from '@/types/generated/BackupSummary';
import type { DialogTitleRequest } from '@/types/generated/DialogTitleRequest';
import type { RestoreRequest } from '@/types/generated/RestoreRequest';
import type { ExportDocumentRequest } from '@/types/generated/ExportDocumentRequest';
import type { ExportOutcome } from '@/types/generated/ExportOutcome';
import type { ExportLibraryRequest } from '@/types/generated/ExportLibraryRequest';
import type { ExportPlan } from '@/types/generated/ExportPlan';
import type { ExportLibraryAddRequest } from '@/types/generated/ExportLibraryAddRequest';
import type { ExportIdRequest } from '@/types/generated/ExportIdRequest';
import type { ExportPackRequest } from '@/types/generated/ExportPackRequest';
import type { PackImport } from '@/types/generated/PackImport';
import type { SearchRequest } from '@/types/generated/SearchRequest';
import type { UpdateSubnetRequest } from '@/types/generated/UpdateSubnetRequest';
import type { VersionRequest } from '@/types/generated/VersionRequest';
import type { VersionSummary } from '@/types/generated/VersionSummary';
import type { VersionText } from '@/types/generated/VersionText';
import type { SystemCheckResponse } from '@/types/generated/SystemCheckResponse';
import type { AnalysisProgress } from '@/types/generated/AnalysisProgress';
import type { AnalysisReport } from '@/types/generated/AnalysisReport';
import type { MeshConnection } from '@/types/generated/MeshConnection';
import type { MeshDecisionRequest } from '@/types/generated/MeshDecisionRequest';
import type { MeshSuggestionList } from '@/types/generated/MeshSuggestionList';
import type { MeshSuggestionsRequest } from '@/types/generated/MeshSuggestionsRequest';
import type { MeshThresholdRequest } from '@/types/generated/MeshThresholdRequest';
import type { MeshConnectionRequest } from '@/types/generated/MeshConnectionRequest';
import type { MeshConnectRequest } from '@/types/generated/MeshConnectRequest';
import type { MeshGraph } from '@/types/generated/MeshGraph';
import type { MeshGraphRequest } from '@/types/generated/MeshGraphRequest';
import type { MeshPositionsRequest } from '@/types/generated/MeshPositionsRequest';
import type { MeshRestoreRequest } from '@/types/generated/MeshRestoreRequest';
import type { UiEvent } from '@/types/generated/UiEvent';
import type { UiState } from '@/types/generated/UiState';
import type { UpdateUiSettingsRequest } from '@/types/generated/UpdateUiSettingsRequest';

/** Every app command with its request and response type. */
export interface Commands {
  system_check: { request: SystemCheckRequest; response: SystemCheckResponse };
  library_status: { request: EmptyRequest; response: LibraryStatus };
  library_default_location: { request: EmptyRequest; response: LocationChoice | null };
  library_choose_location: { request: ChooseLocationRequest; response: LocationChoice | null };
  library_create: { request: CreateLibraryRequest; response: LibraryStatus };
  library_open: { request: OpenLibraryRequest; response: LibraryStatus };
  library_reopen: { request: EmptyRequest; response: LibraryStatus };
  library_move: { request: MoveLibraryRequest; response: LibraryStatus };
  library_cancel_move: { request: EmptyRequest; response: boolean };
  library_accept_location: { request: EmptyRequest; response: LibraryStatus };
  ui_state: { request: EmptyRequest; response: UiState };
  ui_update_settings: { request: UpdateUiSettingsRequest; response: UiState };
  ui_subscribe: { request: EmptyRequest; response: null };
  ui_menu_state: { request: MenuStateRequest; response: null };
  ui_set_focus_mode: { request: FocusModeRequest; response: null };
  ui_close_response: { request: CloseResponseRequest; response: null };
  documents_list: { request: EmptyRequest; response: DocumentSummary[] };
  document_create: { request: CreateDocumentRequest; response: DocumentSummary };
  document_read: { request: DocumentRequest; response: DocumentText };
  document_save: { request: SaveDocumentRequest; response: DocumentSummary };
  document_rename: { request: RenameDocumentRequest; response: DocumentSummary };
  document_copy: { request: CopyDocumentRequest; response: DocumentSummary };
  document_set_favorite: { request: SetFavoriteRequest; response: DocumentSummary };
  document_convert: { request: ConvertDocumentRequest; response: DocumentSummary };
  document_backlinks: { request: DocumentRequest; response: DocumentSummary[] };
  document_delete: { request: DocumentRequest; response: DocumentSummary };
  document_restore: { request: DocumentRequest; response: DocumentSummary };
  document_archive: { request: ArchiveDocumentRequest; response: DocumentSummary };
  documents_trash: { request: EmptyRequest; response: DocumentSummary[] };
  document_purge: { request: DocumentRequest; response: null };
  trash_empty: { request: EmptyRequest; response: number };
  document_versions: { request: DocumentRequest; response: VersionSummary[] };
  document_version_text: { request: VersionRequest; response: VersionText };
  document_version_save: { request: SaveVersionRequest; response: VersionSummary };
  document_version_close: { request: DocumentRequest; response: VersionSummary };
  document_version_restore: { request: VersionRequest; response: DocumentSummary };
  recovery_write: { request: WriteDraftRequest; response: null };
  recovery_discard: { request: DiscardDraftRequest; response: null };
  recovery_list: { request: EmptyRequest; response: DraftEntry[] };
  recovery_restore: { request: DraftRequest; response: RestoredDraft };
  recovery_drop: { request: DraftRequest; response: null };
  session_save: { request: SaveSessionRequest; response: null };
  session_restore: { request: EmptyRequest; response: RestoredSession };
  structure_state: { request: EmptyRequest; response: StructureState };
  structure_document: { request: StructureDocumentRequest; response: DocumentStructure };
  structure_history: { request: EmptyRequest; response: UndoHistory };
  search_query: { request: SearchRequest; response: SearchHit[] };
  search_rebuild: { request: EmptyRequest; response: RebuildReport };
  backup_overview: { request: EmptyRequest; response: BackupOverview };
  backup_choose_target: { request: DialogTitleRequest; response: BackupOverview | null };
  backup_reset_target: { request: EmptyRequest; response: BackupOverview };
  backup_set_options: { request: BackupOptionsRequest; response: BackupOverview };
  backup_create: { request: EmptyRequest; response: BackupSummary };
  backup_cancel: { request: EmptyRequest; response: boolean };
  backup_check: { request: DialogTitleRequest; response: BackupSummary | null };
  backup_restore: { request: RestoreRequest; response: LibraryStatus | null };
  export_document: { request: ExportDocumentRequest; response: ExportOutcome | null };
  export_library_begin: { request: ExportLibraryRequest; response: ExportPlan | null };
  export_library_add: { request: ExportLibraryAddRequest; response: null };
  export_library_finish: { request: ExportIdRequest; response: ExportOutcome };
  export_library_cancel: { request: ExportIdRequest; response: boolean };
  export_pack: { request: ExportPackRequest; response: number | null };
  import_pack: { request: ExportLibraryRequest; response: PackImport | null };
  mesh_graph: { request: MeshGraphRequest; response: MeshGraph };
  mesh_connect: { request: MeshConnectRequest; response: MeshConnection };
  mesh_disconnect: { request: MeshConnectionRequest; response: MeshConnection };
  mesh_restore_connection: { request: MeshRestoreRequest; response: MeshConnection };
  mesh_save_positions: { request: MeshPositionsRequest; response: null };
  mesh_suggestions: { request: MeshSuggestionsRequest; response: MeshSuggestionList };
  mesh_analyze: { request: EmptyRequest; response: AnalysisReport };
  mesh_analysis_cancel: { request: EmptyRequest; response: boolean };
  mesh_suggestion_decide: { request: MeshDecisionRequest; response: MeshConnection | null };
  mesh_set_threshold: { request: MeshThresholdRequest; response: number };
  structure_undo: { request: EmptyRequest; response: StructureUpdate | null };
  structure_redo: { request: EmptyRequest; response: StructureUpdate | null };
  subnet_create: { request: CreateSubnetRequest; response: StructureUpdate };
  subnet_update: { request: UpdateSubnetRequest; response: StructureUpdate };
  subnet_move: { request: MoveSubnetRequest; response: StructureUpdate };
  subnet_delete: { request: DeleteSubnetRequest; response: StructureUpdate };
  document_assign: { request: MembershipRequest; response: StructureUpdate };
  document_unassign: { request: MembershipRequest; response: StructureUpdate };
  document_set_primary: { request: MembershipRequest; response: StructureUpdate };
  document_move: { request: MoveDocumentRequest; response: StructureUpdate };
  tag_create: { request: TagNameRequest; response: StructureUpdate };
  tag_rename: { request: RenameTagRequest; response: StructureUpdate };
  tag_merge: { request: MergeTagsRequest; response: StructureUpdate };
  tag_delete: { request: TagRequest; response: StructureUpdate };
  document_tag: { request: DocumentTagRequest; response: StructureUpdate };
  document_untag: { request: DocumentUntagRequest; response: StructureUpdate };
  media_list: { request: EmptyRequest; response: MediaSummary[] };
  media_trash_list: { request: EmptyRequest; response: MediaSummary[] };
  media_get: { request: MediaIdRequest; response: MediaSummary };
  media_update: { request: UpdateMediaRequest; response: MediaSummary };
  media_set_favorite: { request: MediaFlagRequest; response: MediaSummary };
  media_mark_opened: { request: MediaIdRequest; response: MediaSummary };
  media_delete: { request: MediaIdRequest; response: MediaSummary };
  media_restore: { request: MediaIdRequest; response: MediaSummary };
  media_archive: { request: MediaFlagRequest; response: MediaSummary };
  media_purge: { request: MediaIdRequest; response: null };
  media_empty_trash: { request: EmptyRequest; response: PurgeReport };
  media_uses: { request: MediaIdRequest; response: MediaUse[] };
  media_set_page_count: { request: PageCountRequest; response: null };
  media_store_thumbnail: { request: StoreThumbnailRequest; response: null };
  media_open_external: { request: MediaIdRequest; response: null };
  media_import_files: { request: ImportStartRequest; response: ImportPreview | null };
  media_import_folder: { request: ImportStartRequest; response: ImportPreview | null };
  media_import_commit: { request: ImportCommitRequest; response: ImportOutcome };
  media_import_cancel: { request: ImportCancelRequest; response: null };
  library_preferences: { request: EmptyRequest; response: LibraryPreferences };
  library_update_preferences: { request: LibraryPreferences; response: LibraryPreferences };
  link_open: { request: OpenLinkRequest; response: null };
  document_import: { request: ImportDocumentRequest; response: ImportResult };
  theme_import: { request: ImportThemeRequest; response: UiState };
  theme_remove: { request: RemoveThemeRequest; response: UiState };
  external_open: { request: OpenExternalRequest; response: ExternalFileInfo | null };
  external_read: { request: ExternalFileRequest; response: ExternalText };
  external_check: { request: ExternalFileRequest; response: FileCheck };
  external_keep_mine: { request: ExternalFileRequest; response: null };
  external_save: { request: SaveExternalRequest; response: SaveOutcome };
  external_save_as: { request: SaveExternalAsRequest; response: ExternalFileInfo | null };
  external_close: { request: ExternalFileRequest; response: null };
  external_import: { request: ImportExternalRequest; response: ImportResult };
  external_recent: { request: EmptyRequest; response: RecentFileEntry[] };
  external_open_recent: { request: ExternalFileRequest; response: ExternalFileInfo };
  external_forget_recent: { request: ExternalFileRequest; response: RecentFileEntry[] };
  open_requests_take: { request: EmptyRequest; response: OpenTarget[] };
}

/** Commands that report progress through a channel argument `onProgress`. */
export interface ProgressCommands {
  library_move: MoveProgress;
  media_import_files: ImportProgress;
  media_import_folder: ImportProgress;
  backup_create: BackupProgress;
  backup_check: BackupProgress;
  backup_restore: BackupProgress;
  mesh_analyze: AnalysisProgress;
}

export type CommandName = keyof Commands;

export type CommandResult<T> = { ok: true; value: T } | { ok: false; error: CommandError };

const errorCodes: Record<ErrorCode, true> = {
  INVALID_INPUT: true,
  DATABASE: true,
  SCHEMA_TOO_NEW: true,
  INTERNAL: true,
  IPC_REJECTED: true,
  FILE_SYSTEM: true,
  LIBRARY_NOT_OPEN: true,
  LIBRARY_NOT_FOUND: true,
  LIBRARY_INVALID: true,
  LIBRARY_TOO_NEW: true,
  LIBRARY_LOCKED: true,
  LIBRARY_LOCK_UNSUPPORTED: true,
  LIBRARY_MOVED: true,
  LIBRARY_UNSAFE_PATH: true,
  LIBRARY_DATABASE_MISSING: true,
  LIBRARY_DATABASE_DAMAGED: true,
  LIBRARY_BACKUP_FAILED: true,
  LIBRARY_BUSY: true,
  FOLDER_NOT_EMPTY: true,
  FOLDER_IS_LIBRARY: true,
  LOCATION_NOT_ACCEPTED: true,
  LOCATION_EXPIRED: true,
  PATH_NOT_UNICODE: true,
  MOVE_CANCELLED: true,
  MOVE_TARGET_INVALID: true,
  MOVE_VERIFICATION_FAILED: true,
  MOVE_SWITCH_FAILED: true,
  SETTINGS_READ_ONLY: true,
  SETTINGS_NOT_SAVED: true,
  DOCUMENT_NOT_FOUND: true,
  DOCUMENT_TOO_LARGE: true,
  DOCUMENT_NOT_TEXT: true,
  DOCUMENT_FORMAT_UNSUPPORTED: true,
  IMPORT_UNSUPPORTED_TYPE: true,
  THEME_INVALID: true,
  THEME_CONTRAST: true,
  THEME_LIMIT: true,
  THEME_NOT_FOUND: true,
  LINK_NOT_ALLOWED: true,
  LINK_OPEN_FAILED: true,
  PWRITE_INVALID: true,
  DOCUMENT_READ_ONLY: true,
  FILE_UNKNOWN: true,
  FILE_MISSING: true,
  FILE_CHANGED: true,
  FILE_READ_ONLY: true,
  FILE_UNENCODABLE: true,
  VERSION_NOT_FOUND: true,
  VERSION_DAMAGED: true,
  SUBNET_NOT_FOUND: true,
  SUBNET_CYCLE: true,
  SUBNET_NAME_TAKEN: true,
  TAG_NOT_FOUND: true,
  TAG_NAME_TAKEN: true,
  STRUCTURE_CHANGED: true,
  MEDIA_NOT_FOUND: true,
  MEDIA_TYPE_REFUSED: true,
  MEDIA_TOO_LARGE: true,
  MEDIA_IN_USE: true,
  IMPORT_NOT_FOUND: true,
  MEDIA_OPEN_FAILED: true,
  BACKUP_REFUSED: true,
  BACKUP_NEWER: true,
  BACKUP_RUNNING: true,
  BACKUP_TARGET_INSIDE_LIBRARY: true,
  BACKUP_CANCELLED: true,
  MESH_NODE_NOT_FOUND: true,
  MESH_CONNECTION_NOT_FOUND: true,
  MESH_CONNECTION_EXISTS: true,
  MESH_SUGGESTION_NOT_FOUND: true,
  MESH_ANALYSIS_RUNNING: true,
  MESH_ANALYSIS_CANCELLED: true,
};

export const IPC_REJECTED: CommandError = { code: 'IPC_REJECTED', messageKey: 'error.ipcRejected' };

export function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  const { code, messageKey, details } = candidate;
  return (
    typeof code === 'string' &&
    Object.hasOwn(errorCodes, code) &&
    typeof messageKey === 'string' &&
    (details === undefined || (typeof details === 'object' && details !== null))
  );
}

export function normalizeError(raw: unknown): CommandError {
  return isCommandError(raw) ? raw : IPC_REJECTED;
}

/** Calls an app command. Never throws. */
export async function call<C extends CommandName>(
  command: C,
  request: Commands[C]['request'],
): Promise<CommandResult<Commands[C]['response']>> {
  try {
    const value = await invoke<Commands[C]['response']>(command, { request });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}

/**
 * Calls a command that streams progress. Progress messages arrive through a
 * Tauri channel (no event permission needed). Never throws.
 */
export async function callWithProgress<C extends CommandName & keyof ProgressCommands>(
  command: C,
  request: Commands[C]['request'],
  onProgress: (progress: ProgressCommands[C]) => void,
): Promise<CommandResult<Commands[C]['response']>> {
  const channel = new Channel<ProgressCommands[C]>();
  channel.onmessage = onProgress;
  try {
    const value = await invoke<Commands[C]['response']>(command, { request, onProgress: channel });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}

/**
 * Hands a dropped or pasted file to Rust as raw bytes (ADR 0019): the
 * command `media_import_bytes` prepares an import of one. The name travels
 * URI-encoded in a header. Never throws.
 */
export async function importMediaBytes(
  name: string,
  bytes: Uint8Array,
): Promise<CommandResult<ImportPreview>> {
  try {
    const value = await invoke<ImportPreview>('media_import_bytes', bytes, {
      headers: { 'x-pw-file-name': encodeURIComponent(name) },
    });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}

/**
 * Registers the handler for messages from Rust: menu clicks and close
 * requests of the window. A later call replaces the handler. Never throws.
 */
export async function subscribeUiEvents(onEvent: (event: UiEvent) => void): Promise<CommandResult<null>> {
  const channel = new Channel<UiEvent>();
  channel.onmessage = onEvent;
  try {
    const value = await invoke<null>('ui_subscribe', { request: {}, onEvent: channel });
    return { ok: true, value };
  } catch (raw) {
    return { ok: false, error: normalizeError(raw) };
  }
}
