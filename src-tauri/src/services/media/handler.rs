//! The media handler (section 17, "Medien sicher anzeigen"; ADR 0019,
//! decision 8): `pwmedia://localhost/<id>` and `…/<id>/thumbnail`, under
//! Windows `http://pwmedia.localhost/…`.
//!
//! The asset protocol stays off. The handler resolves an id through the
//! database of the open library to its file; the interface never sees a
//! path. Only images and PDFs are served, with the content type from the
//! check on import, `X-Content-Type-Options: nosniff` and a sandboxing CSP
//! on every answer: an SVG never runs a script, whether it is shown as an
//! image or opened on its own.

use tauri::http::{Response, StatusCode, header};
use uuid::Uuid;

use super::{MAX_FILE_BYTES, MediaError, MediaLibrary};
use crate::filesystem::atomic::read_limited;

/// The scheme registered with the webview.
pub const SCHEME: &str = "pwmedia";
/// CSP of every answer.
pub const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; sandbox";

/// What a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Original,
    Thumbnail,
}

/// `/<id>` or `/<id>/thumbnail`, the id in lower case with hyphens.
pub fn parse_path(path: &str) -> Option<(Uuid, Variant)> {
    let rest = path.strip_prefix('/')?;
    let (id, variant) = match rest.split_once('/') {
        Some((id, "thumbnail")) => (id, Variant::Thumbnail),
        Some(_) => return None,
        None => (rest, Variant::Original),
    };
    let uuid = Uuid::try_parse(id).ok()?;
    (uuid.hyphenated().to_string() == id).then_some((uuid, variant))
}

fn answer(status: StatusCode, content_type: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY)
        .header(header::CACHE_CONTROL, "no-cache")
        // The PDF view loads its file with fetch from the app's origin.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn failure(status: StatusCode) -> Response<Vec<u8>> {
    answer(
        status,
        "text/plain; charset=utf-8",
        status
            .canonical_reason()
            .unwrap_or("error")
            .as_bytes()
            .to_vec(),
    )
}

/// Answers one request. `media` is `None` while no library is open.
pub async fn respond(media: Option<MediaLibrary>, method: &str, path: &str) -> Response<Vec<u8>> {
    let head = match method {
        "GET" => false,
        "HEAD" => true,
        _ => return failure(StatusCode::METHOD_NOT_ALLOWED),
    };
    let Some((id, variant)) = parse_path(path) else {
        return failure(StatusCode::NOT_FOUND);
    };
    let Some(media) = media else {
        return failure(StatusCode::NOT_FOUND);
    };
    let served = match variant {
        Variant::Original => media.served_file(id).await.map(Some),
        Variant::Thumbnail => media.thumbnail(id).await,
    };
    let file = match served {
        Ok(Some(file)) => file,
        Ok(None) | Err(MediaError::NotFound) => return failure(StatusCode::NOT_FOUND),
        Err(error) => {
            log::warn!("medium {id} not served: {error}");
            return failure(StatusCode::NOT_FOUND);
        }
    };
    if head {
        return answer(StatusCode::OK, file.mime_type, Vec::new());
    }
    let path = file.path.clone();
    match tokio::task::spawn_blocking(move || read_limited(&path, MAX_FILE_BYTES)).await {
        Ok(Ok(body)) => answer(StatusCode::OK, file.mime_type, body),
        Ok(Err(error)) => {
            log::warn!("medium {id} not read: {error}");
            failure(StatusCode::NOT_FOUND)
        }
        Err(error) => {
            log::warn!("medium {id} not read: {error}");
            failure(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ids_and_thumbnails_are_paths() {
        let id = "6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f";
        assert_eq!(
            parse_path(&format!("/{id}")),
            Some((Uuid::try_parse(id).unwrap(), Variant::Original))
        );
        assert_eq!(
            parse_path(&format!("/{id}/thumbnail")),
            Some((Uuid::try_parse(id).unwrap(), Variant::Thumbnail))
        );
        assert_eq!(parse_path(&format!("/{}", id.to_uppercase())), None);
        assert_eq!(parse_path(&format!("/{id}/../library.json")), None);
        assert_eq!(parse_path("/../database/projectwrite.db"), None);
        assert_eq!(parse_path(id), None);
        assert_eq!(parse_path("/"), None);
    }
}
