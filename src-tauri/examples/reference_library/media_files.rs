//! Media files of the reference library (Phase 8): pictures in all five
//! formats (PNG, JPEG, GIF, WebP, SVG), PDFs with text on every page and
//! DOCX attachments. Everything comes from
//! the seeded generator, so the same seed gives the same bytes; the ZIP
//! entries of a DOCX carry a fixed time (1980-01-01).

use std::io::{Cursor, Write};

use image::{ImageFormat, Rgb, RgbImage};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::Rng;

/// A picture of `width` × `height` from seeded colours: a vertical
/// gradient between two of them, a lighter band at a seeded height and a
/// block of a third colour at a seeded place. Pictures differ in content
/// but stay small, as ordinary graphics do.
fn picture(rng: &mut Rng, width: u32, height: u32) -> RgbImage {
    let mut channel = || (rng.next() % 256) as u8;
    let top = [channel(), channel(), channel()];
    let bottom = [channel(), channel(), channel()];
    let block = [channel(), channel(), channel()];
    let band = (rng.next() % u64::from(height.max(1))) as u32;
    let (left, upper) = (
        (rng.next() % u64::from(width.max(1))) as u32,
        (rng.next() % u64::from(height.max(1))) as u32,
    );
    let (block_width, block_height) = (width / 4 + 1, height / 4 + 1);
    let span = height.max(2) - 1;
    RgbImage::from_fn(width, height, |x, y| {
        if x.wrapping_sub(left) < block_width && y.wrapping_sub(upper) < block_height {
            return Rgb(block);
        }
        let mix = |a: u8, b: u8| ((u32::from(a) * (span - y) + u32::from(b) * y) / span) as u8;
        let lift = if y.abs_diff(band) < height / 10 {
            30
        } else {
            0
        };
        Rgb([0, 1, 2].map(|i| mix(top[i], bottom[i]).saturating_add(lift)))
    })
}

fn encoded(image: &RgbImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, format)
        .expect("encoding an image in memory");
    bytes.into_inner()
}

pub fn png(rng: &mut Rng, width: u32, height: u32) -> Vec<u8> {
    encoded(&picture(rng, width, height), ImageFormat::Png)
}

pub fn jpeg(rng: &mut Rng, width: u32, height: u32) -> Vec<u8> {
    encoded(&picture(rng, width, height), ImageFormat::Jpeg)
}

pub fn gif(rng: &mut Rng, width: u32, height: u32) -> Vec<u8> {
    encoded(&picture(rng, width, height), ImageFormat::Gif)
}

/// Lossless WebP (the encoder of `image`).
pub fn webp(rng: &mut Rng, width: u32, height: u32) -> Vec<u8> {
    let picture = image::DynamicImage::ImageRgb8(picture(rng, width, height)).to_rgba8();
    let mut bytes = Cursor::new(Vec::new());
    picture
        .write_to(&mut bytes, ImageFormat::WebP)
        .expect("encoding an image in memory");
    bytes.into_inner()
}

/// A plain SVG: a background, three shapes and a caption (no script).
pub fn svg(rng: &mut Rng, width: u32, height: u32, caption: &str) -> Vec<u8> {
    let mut color = || format!("#{:06x}", rng.next() & 0xFF_FFFF);
    let (background, first, second, third) = (color(), color(), color(), color());
    let x = |rng: &mut Rng| rng.next() % u64::from(width.max(1));
    let y = |rng: &mut Rng| rng.next() % u64::from(height.max(1));
    let (x1, y1, x2, y2, x3, y3) = (x(rng), y(rng), x(rng), y(rng), x(rng), y(rng));
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n\
         <rect width=\"{width}\" height=\"{height}\" fill=\"{background}\"/>\n\
         <circle cx=\"{x1}\" cy=\"{y1}\" r=\"{}\" fill=\"{first}\"/>\n\
         <rect x=\"{x2}\" y=\"{y2}\" width=\"{}\" height=\"{}\" fill=\"{second}\"/>\n\
         <line x1=\"0\" y1=\"{y3}\" x2=\"{x3}\" y2=\"0\" stroke=\"{third}\" stroke-width=\"4\"/>\n\
         <text x=\"8\" y=\"{}\" font-family=\"sans-serif\" font-size=\"14\">{}</text>\n\
         </svg>\n",
        width.min(height) / 5 + 1,
        width / 4 + 1,
        height / 4 + 1,
        height.saturating_sub(8),
        xml_text(caption)
    )
    .into_bytes()
}

/// Text for a PDF content stream: printable ASCII, `(`, `)` and `\` escaped.
fn pdf_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            'Ä' => 'A',
            'Ö' => 'O',
            'Ü' => 'U',
            'ß' => 's',
            c if c.is_ascii() && !c.is_ascii_control() => c,
            _ => ' ',
        })
        .fold(String::new(), |mut out, c| {
            if matches!(c, '(' | ')' | '\\') {
                out.push('\\');
            }
            out.push(c);
            out
        })
}

/// A PDF of `pages` pages (Helvetica), each with the title, its number and
/// a few lines of `lines`; correct cross-reference table.
pub fn pdf(title: &str, pages: usize, lines: &[String]) -> Vec<u8> {
    let mut objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            (0..pages)
                .map(|page| format!("{} 0 R", 4 + page * 2))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    for page in 0..pages {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>",
            5 + page * 2
        ));
        let mut content = format!(
            "BT /F1 18 Tf 60 780 Td ({}) Tj ET BT /F1 11 Tf 60 750 Td (Seite {}) Tj ET",
            pdf_text(title),
            page + 1
        );
        for (line, text) in lines
            .iter()
            .skip(page % lines.len().max(1))
            .take(6)
            .enumerate()
        {
            content.push_str(&format!(
                " BT /F1 11 Tf 60 {} Td ({}) Tj ET",
                720 - line * 16,
                pdf_text(text)
            ));
        }
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let start = out.len();
    let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        xref.push_str(&format!("{offset:010} 00000 n \n"));
    }
    xref.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
        objects.len() + 1
    ));
    out.extend_from_slice(xref.as_bytes());
    out
}

fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A DOCX with one paragraph per line (main part and content types only).
pub fn docx(lines: &[String]) -> Vec<u8> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default());
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>";
    let body: String = lines
        .iter()
        .map(|line| format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>", xml_text(line)))
        .collect();
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
    );
    for (name, content) in [
        ("[Content_Types].xml", types.to_owned()),
        ("word/document.xml", document),
    ] {
        zip.start_file(name, options).expect("zip entry in memory");
        zip.write_all(content.as_bytes())
            .expect("zip data in memory");
    }
    zip.finish().expect("zip in memory").into_inner()
}
