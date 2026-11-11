//! Checks the type of a file by its content (section 20: "Prüfe importierte
//! Dateitypen über den Inhalt, nicht nur über die Endung. Es gilt eine
//! Positivliste."; ADR 0019, decision 3).
//!
//! The check runs on the copy in `tmp/`, never on the original. The
//! extension of the original name must belong to the type the content
//! shows; Office files with macros are refused.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Largest SVG accepted (ADR 0019, decision 4).
pub const MAX_SVG_BYTES: u64 = 10 * 1024 * 1024;
/// ZIP limits for the Office check.
const MAX_ZIP_ENTRIES: usize = 10_000;
const MAX_CONTENT_TYPES_BYTES: u64 = 1024 * 1024;
/// Bytes read to recognise a type (`%PDF-` may start within the first 1024).
const HEAD_BYTES: usize = 4096;

/// A type of the positive list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
    Pdf,
    Docx,
    Xlsx,
    Pptx,
}

/// Kind of medium, as stored in `media.kind`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, ts_rs::TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MediaKind {
    Image,
    Pdf,
    Attachment,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Image => "image",
            MediaKind::Pdf => "pdf",
            MediaKind::Attachment => "attachment",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "image" => Some(MediaKind::Image),
            "pdf" => Some(MediaKind::Pdf),
            "attachment" => Some(MediaKind::Attachment),
            _ => None,
        }
    }

    /// Folder below the library root (section 9).
    pub fn directory(self) -> &'static str {
        match self {
            MediaKind::Image => "media/images",
            MediaKind::Pdf => "media/pdf",
            MediaKind::Attachment => "media/attachments",
        }
    }
}

impl MediaType {
    /// The whole positive list.
    pub const ALL: [MediaType; 9] = [
        MediaType::Png,
        MediaType::Jpeg,
        MediaType::Gif,
        MediaType::Webp,
        MediaType::Svg,
        MediaType::Pdf,
        MediaType::Docx,
        MediaType::Xlsx,
        MediaType::Pptx,
    ];

    pub fn kind(self) -> MediaKind {
        match self {
            MediaType::Png
            | MediaType::Jpeg
            | MediaType::Gif
            | MediaType::Webp
            | MediaType::Svg => MediaKind::Image,
            MediaType::Pdf => MediaKind::Pdf,
            MediaType::Docx | MediaType::Xlsx | MediaType::Pptx => MediaKind::Attachment,
        }
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            MediaType::Png => "image/png",
            MediaType::Jpeg => "image/jpeg",
            MediaType::Gif => "image/gif",
            MediaType::Webp => "image/webp",
            MediaType::Svg => "image/svg+xml",
            MediaType::Pdf => "application/pdf",
            MediaType::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            MediaType::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            MediaType::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
        }
    }

    pub fn from_mime_type(mime: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.mime_type() == mime)
    }

    /// Whether `extension` (lower case, without dot) names this type.
    pub fn accepts_extension(self, extension: &str) -> bool {
        match self {
            MediaType::Png => extension == "png",
            MediaType::Jpeg => extension == "jpg" || extension == "jpeg",
            MediaType::Gif => extension == "gif",
            MediaType::Webp => extension == "webp",
            MediaType::Svg => extension == "svg",
            MediaType::Pdf => extension == "pdf",
            MediaType::Docx => extension == "docx",
            MediaType::Xlsx => extension == "xlsx",
            MediaType::Pptx => extension == "pptx",
        }
    }

    /// Raster images decoded for thumbnails and checked for their size.
    pub fn is_raster(self) -> bool {
        matches!(
            self,
            MediaType::Png | MediaType::Jpeg | MediaType::Gif | MediaType::Webp
        )
    }
}

/// Why a file is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The content is no type of the positive list.
    Type,
    /// The extension does not belong to the type of the content.
    Extension,
    /// An Office file with macros.
    Macro,
    /// The content claims a type but is not valid for it (ZIP without an
    /// Office main part, SVG with entities, …).
    Content,
}

impl Refusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Refusal::Type => "type",
            Refusal::Extension => "extension",
            Refusal::Macro => "macro",
            Refusal::Content => "content",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    #[error("refused: {}", .0.as_str())]
    Refused(Refusal),
    #[error("SVG larger than {MAX_SVG_BYTES} bytes")]
    SvgTooLarge,
    #[error("read error: {0}")]
    Io(#[from] io::Error),
}

/// Extensions on the positive list (lower case), to skip other files of a
/// folder without reading them.
pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "pdf", "docx", "xlsx", "pptx",
];
/// Office files with macros (never imported, section 17).
pub const MACRO_EXTENSIONS: &[&str] = &["docm", "xlsm", "pptm", "dotm", "xltm", "potm", "ppsm"];

/// Lower-case extension of a file name, without dot.
pub fn extension_of(name: &str) -> Option<String> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension.to_lowercase())
}

/// Checks `path` (the copy) against the positive list, with the name the
/// file had.
pub fn detect(path: &Path, original_name: &str) -> Result<MediaType, DetectError> {
    let extension = extension_of(original_name).unwrap_or_default();
    if MACRO_EXTENSIONS.contains(&extension.as_str()) {
        return Err(DetectError::Refused(Refusal::Macro));
    }
    let mut file = File::open(path)?;
    let mut head = Vec::with_capacity(HEAD_BYTES);
    (&mut file).take(HEAD_BYTES as u64).read_to_end(&mut head)?;
    let detected = match sniff(&head) {
        Some(Sniffed::Type(found)) => found,
        Some(Sniffed::Zip) => office_type(path)?,
        Some(Sniffed::MaybeSvg) => {
            let size = file.metadata()?.len();
            if size > MAX_SVG_BYTES {
                return Err(DetectError::SvgTooLarge);
            }
            let mut text = Vec::new();
            File::open(path)?.read_to_end(&mut text)?;
            check_svg(&text)?;
            MediaType::Svg
        }
        None => return Err(DetectError::Refused(Refusal::Type)),
    };
    if !detected.accepts_extension(&extension) {
        return Err(DetectError::Refused(Refusal::Extension));
    }
    Ok(detected)
}

enum Sniffed {
    Type(MediaType),
    Zip,
    MaybeSvg,
}

fn sniff(head: &[u8]) -> Option<Sniffed> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(Sniffed::Type(MediaType::Png));
    }
    if head.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(Sniffed::Type(MediaType::Jpeg));
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some(Sniffed::Type(MediaType::Gif));
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(Sniffed::Type(MediaType::Webp));
    }
    if head.starts_with(b"PK\x03\x04") {
        return Some(Sniffed::Zip);
    }
    if head[..head.len().min(1024)]
        .windows(5)
        .any(|window| window == b"%PDF-")
    {
        return Some(Sniffed::Type(MediaType::Pdf));
    }
    let text = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let first = text.iter().find(|byte| !byte.is_ascii_whitespace())?;
    (*first == b'<').then_some(Sniffed::MaybeSvg)
}

/// SVG: UTF-8 text whose root element is `svg` (after an XML declaration,
/// comments and a DOCTYPE). A DOCTYPE with an internal subset or an entity
/// declaration is refused: entities can expand without bound.
fn check_svg(data: &[u8]) -> Result<(), DetectError> {
    let refused = || DetectError::Refused(Refusal::Content);
    let text = std::str::from_utf8(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data))
        .map_err(|_| refused())?;
    if text.contains("<!ENTITY") {
        return Err(refused());
    }
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<?") {
            rest = &after[after.find("?>").ok_or_else(refused)? + 2..];
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = &after[after.find("-->").ok_or_else(refused)? + 3..];
        } else if let Some(after) = rest.strip_prefix("<!DOCTYPE") {
            let end = after.find('>').ok_or_else(refused)?;
            if after[..end].contains('[') {
                return Err(refused());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    let after = rest
        .strip_prefix("<svg")
        .or_else(|| rest.strip_prefix("<svg:svg"))
        .ok_or_else(refused)?;
    match after.chars().next() {
        Some(c) if c.is_whitespace() || c == '>' || c == '/' => Ok(()),
        _ => Err(refused()),
    }
}

/// DOCX, XLSX or PPTX: a ZIP whose `[Content_Types].xml` names the main part
/// of the type. Macros (a `vbaProject.bin` part or a `macroEnabled` type)
/// are refused.
fn office_type(path: &Path) -> Result<MediaType, DetectError> {
    let content = || DetectError::Refused(Refusal::Content);
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| content())?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(content());
    }
    // A name that is no valid text counts against the file as well.
    if archive.file_names().any(|name| {
        name.map_or(true, |name| {
            name.to_ascii_lowercase().ends_with("vbaproject.bin")
        })
    }) {
        return Err(DetectError::Refused(Refusal::Macro));
    }
    let mut types = String::new();
    {
        let entry = archive
            .by_name("[Content_Types].xml")
            .map_err(|_| content())?;
        if entry.size() > MAX_CONTENT_TYPES_BYTES {
            return Err(content());
        }
        entry
            .take(MAX_CONTENT_TYPES_BYTES)
            .read_to_string(&mut types)
            .map_err(|_| content())?;
    }
    if types.contains("macroEnabled") || types.contains("vbaProject") {
        return Err(DetectError::Refused(Refusal::Macro));
    }
    let main = [
        ("wordprocessingml.document.main+xml", MediaType::Docx),
        ("spreadsheetml.sheet.main+xml", MediaType::Xlsx),
        ("presentationml.presentation.main+xml", MediaType::Pptx),
    ];
    main.into_iter()
        .find(|(needle, _)| types.contains(needle))
        .map(|(_, found)| found)
        .ok_or_else(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A file with `content` in a temporary folder inside `target/` (never
    /// outside the project folder, section 2).
    struct TestFile {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    fn file_with(content: &[u8]) -> TestFile {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, content).unwrap();
        TestFile { _dir: dir, path }
    }

    fn zip_with(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            for (name, content) in entries {
                writer
                    .start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(content.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    const DOCX_TYPES: &str = r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

    fn refusal(result: Result<MediaType, DetectError>) -> Refusal {
        match result {
            Err(DetectError::Refused(refusal)) => refusal,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn raster_images_and_pdf_are_known_by_their_first_bytes() {
        let png = file_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
        assert_eq!(detect(&png.path, "a.PNG").unwrap(), MediaType::Png);
        let jpeg = file_with(&[0xff, 0xd8, 0xff, 0xe0, 0, 0x10]);
        assert_eq!(detect(&jpeg.path, "a.jpg").unwrap(), MediaType::Jpeg);
        assert_eq!(detect(&jpeg.path, "a.jpeg").unwrap(), MediaType::Jpeg);
        let gif = file_with(b"GIF89a\x01\0\x01\0");
        assert_eq!(detect(&gif.path, "a.gif").unwrap(), MediaType::Gif);
        let webp = file_with(b"RIFF\x24\0\0\0WEBPVP8 ");
        assert_eq!(detect(&webp.path, "a.webp").unwrap(), MediaType::Webp);
        let pdf = file_with(b"junk before\n%PDF-1.7\n");
        assert_eq!(detect(&pdf.path, "a.pdf").unwrap(), MediaType::Pdf);
    }

    #[test]
    fn the_extension_must_belong_to_the_content() {
        let png = file_with(b"\x89PNG\r\n\x1a\n");
        assert_eq!(refusal(detect(&png.path, "a.jpg")), Refusal::Extension);
        assert_eq!(refusal(detect(&png.path, "png")), Refusal::Extension);
        // An executable renamed to .png is no image.
        let exe = file_with(b"MZ\x90\0\x03\0\0\0");
        assert_eq!(refusal(detect(&exe.path, "bild.png")), Refusal::Type);
        let text = file_with(b"plain text");
        assert_eq!(refusal(detect(&text.path, "notiz.pdf")), Refusal::Type);
    }

    #[test]
    fn svg_needs_an_svg_root_and_no_entities() {
        let svg = file_with(
            b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<!-- c -->\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x\">\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        );
        assert_eq!(detect(&svg.path, "a.svg").unwrap(), MediaType::Svg);
        let html = file_with(b"<html><svg></svg></html>");
        assert_eq!(refusal(detect(&html.path, "a.svg")), Refusal::Content);
        let entities = file_with(
            b"<?xml version=\"1.0\"?><!DOCTYPE svg [<!ENTITY a \"aaaa\">]><svg>&a;</svg>",
        );
        assert_eq!(refusal(detect(&entities.path, "a.svg")), Refusal::Content);
        let latin1 = file_with(b"<svg>\xe4</svg>");
        assert_eq!(refusal(detect(&latin1.path, "a.svg")), Refusal::Content);
    }

    #[test]
    fn office_files_are_known_by_their_main_part_and_macros_are_refused() {
        let docx = file_with(&zip_with(&[
            ("[Content_Types].xml", DOCX_TYPES),
            ("word/document.xml", "<w:document/>"),
        ]));
        assert_eq!(detect(&docx.path, "Bericht.docx").unwrap(), MediaType::Docx);
        assert_eq!(
            refusal(detect(&docx.path, "Bericht.xlsx")),
            Refusal::Extension
        );
        assert_eq!(refusal(detect(&docx.path, "Bericht.docm")), Refusal::Macro);

        let macro_type = file_with(&zip_with(&[(
            "[Content_Types].xml",
            &DOCX_TYPES.replace(
                "officedocument.wordprocessingml.document.main+xml",
                "ms-word.document.macroEnabled.main+xml",
            ),
        )]));
        assert_eq!(refusal(detect(&macro_type.path, "a.docx")), Refusal::Macro);

        let vba = file_with(&zip_with(&[
            ("[Content_Types].xml", DOCX_TYPES),
            ("word/vbaProject.bin", "x"),
        ]));
        assert_eq!(refusal(detect(&vba.path, "a.docx")), Refusal::Macro);

        let plain_zip = file_with(&zip_with(&[("a.txt", "a")]));
        assert_eq!(refusal(detect(&plain_zip.path, "a.docx")), Refusal::Content);
    }

    #[test]
    fn extensions_are_read_without_case() {
        assert_eq!(extension_of("Foto.JPEG").as_deref(), Some("jpeg"));
        assert_eq!(extension_of(".hidden"), None);
        assert_eq!(extension_of("ohne"), None);
        assert_eq!(extension_of("a.tar.gz").as_deref(), Some("gz"));
    }
}
