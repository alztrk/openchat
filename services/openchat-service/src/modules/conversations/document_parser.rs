//! Bounded local document extraction for PDF, DOCX, XLSX, CSV, and HTML files.
//!
//! Enforces strict input size, archive entry count, decompression ratio, memory,
//! and output text limits while preserving page, sheet, cell/row, and section
//! source location markers.

use std::io::{Cursor, Read};

use flate2::read::ZlibDecoder;
use zip::ZipArchive;

/// Maximum raw input size accepted for binary document extraction (10 MiB).
pub(crate) const MAXIMUM_DOCUMENT_INPUT_BYTES: usize = 10 * 1024 * 1024;
/// Maximum extracted UTF-8 text output size (1 MiB, matching `MAXIMUM_TEXT_BYTES`).
pub(crate) const MAXIMUM_DOCUMENT_OUTPUT_BYTES: usize = 1024 * 1024;
/// Maximum number of entries allowed inside a ZIP-based Office container (DOCX/XLSX).
const MAXIMUM_ZIP_ENTRIES: usize = 512;
/// Maximum uncompressed bytes read across all entries in one Office container (16 MiB).
const MAXIMUM_ZIP_UNCOMPRESSED_BYTES: usize = 16 * 1024 * 1024;
/// Maximum uncompressed bytes for a single XML entry inside an Office container (8 MiB).
const MAXIMUM_SINGLE_XML_BYTES: usize = 8 * 1024 * 1024;
/// Maximum decompressed bytes per PDF stream (4 MiB).
const MAXIMUM_PDF_STREAM_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of PDF streams inspected per document.
const MAXIMUM_PDF_STREAMS: usize = 1024;

/// Structured error categories for bounded document extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocumentParseError {
    /// The file extension or format is not a supported document type.
    UnsupportedFormat,
    /// The input file is empty or exceeds `MAXIMUM_DOCUMENT_INPUT_BYTES`.
    FileTooLarge,
    /// The container is a ZIP bomb or exceeds entry/decompression bounds.
    ArchiveLimitExceeded,
    /// The PDF is password-protected or encrypted.
    EncryptedDocument,
    /// The document contains no extractable text (for example, an image-only PDF without OCR).
    NoExtractableText,
    /// The document structure is malformed or corrupted.
    MalformedDocument,
}

impl DocumentParseError {
    /// Machine-readable error code returned over the RPC boundary.
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "document_unsupported_format",
            Self::FileTooLarge => "document_file_too_large",
            Self::ArchiveLimitExceeded => "document_archive_limit_exceeded",
            Self::EncryptedDocument => "document_encrypted",
            Self::NoExtractableText => "document_no_extractable_text",
            Self::MalformedDocument => "document_malformed",
        }
    }

    /// Safe human-readable explanation of why extraction failed.
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::UnsupportedFormat => "This document format is not supported.",
            Self::FileTooLarge => "The document exceeds the maximum supported file size (10 MB).",
            Self::ArchiveLimitExceeded => {
                "The document container exceeded safe decompression limits."
            }
            Self::EncryptedDocument => {
                "Password-protected or encrypted documents cannot be extracted."
            }
            Self::NoExtractableText => {
                "No extractable text was found in this document. Image-only scans without a text layer are not supported."
            }
            Self::MalformedDocument => "The document is malformed or could not be parsed.",
        }
    }
}

/// Result of extracting structured text from a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractedDocument {
    /// Extracted UTF-8 text annotated with source location markers.
    pub text: String,
    /// Whether the extracted output was truncated to stay within `MAXIMUM_DOCUMENT_OUTPUT_BYTES`.
    pub truncated: bool,
    /// Number of logical sections (pages, sheets, or rows) extracted.
    pub section_count: usize,
}

/// Extracts bounded UTF-8 text with source locations from a supported document.
pub(crate) fn extract_document(
    file_name: &str,
    bytes: &[u8],
) -> Result<ExtractedDocument, DocumentParseError> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_DOCUMENT_INPUT_BYTES {
        return Err(DocumentParseError::FileTooLarge);
    }
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .ok_or(DocumentParseError::UnsupportedFormat)?;

    match extension.as_str() {
        "pdf" => extract_pdf(bytes),
        "docx" => extract_docx(bytes),
        "xlsx" => extract_xlsx(bytes),
        "csv" => extract_csv(bytes),
        "html" | "htm" => extract_html(bytes),
        _ => Err(DocumentParseError::UnsupportedFormat),
    }
}

fn extract_docx(bytes: &[u8]) -> Result<ExtractedDocument, DocumentParseError> {
    let mut archive = open_bounded_zip(bytes)?;
    let xml = read_zip_xml_entry(&mut archive, "word/document.xml")?;

    let mut output = String::new();
    let mut section_count = 0usize;
    let mut paragraph_index = 0usize;
    let mut table_index = 0usize;
    let mut truncated = false;

    let mut cursor = xml.as_str();
    while !cursor.is_empty() {
        if let Some(table_pos) = cursor.find("<w:tbl")
            && cursor[..table_pos].find("<w:p").is_none()
        {
            let after_start = &cursor[table_pos..];
            let Some(table_end) = after_start.find("</w:tbl>") else {
                break;
            };
            let table_xml = &after_start[..table_end + "</w:tbl>".len()];
            table_index += 1;
            let mut row_index = 0usize;
            for row_chunk in table_xml.split("<w:tr").skip(1) {
                let Some(row_end) = row_chunk.find("</w:tr>") else {
                    continue;
                };
                let row_xml = &row_chunk[..row_end];
                let mut cells = Vec::new();
                for cell_chunk in row_xml.split("<w:tc").skip(1) {
                    if let Some(cell_end) = cell_chunk.find("</w:tc>") {
                        let cell_text = extract_word_text_runs(&cell_chunk[..cell_end]);
                        if !cell_text.trim().is_empty() {
                            cells.push(cell_text.trim().to_owned());
                        }
                    }
                }
                if !cells.is_empty() {
                    row_index += 1;
                    section_count += 1;
                    let line = format!(
                        "[Table {table_index} | Row {row_index}] {}\n",
                        cells.join(" | ")
                    );
                    if !append_bounded(&mut output, &line) {
                        truncated = true;
                        break;
                    }
                }
            }
            if truncated {
                break;
            }
            cursor = &after_start[table_end + "</w:tbl>".len()..];
            continue;
        }

        let Some(p_start) = cursor.find("<w:p") else {
            break;
        };
        let after_p = &cursor[p_start..];
        let valid_tag = after_p
            .as_bytes()
            .get(4)
            .is_some_and(|b| matches!(b, b'>' | b' ' | b'/'));
        if !valid_tag {
            cursor = &after_p[4..];
            continue;
        }
        let Some(p_end) = after_p.find("</w:p>") else {
            break;
        };
        let p_xml = &after_p[..p_end];
        let text = extract_word_text_runs(p_xml);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            paragraph_index += 1;
            section_count += 1;
            let line = format!("[Paragraph {paragraph_index}] {trimmed}\n");
            if !append_bounded(&mut output, &line) {
                truncated = true;
                break;
            }
        }
        cursor = &after_p[p_end + "</w:p>".len()..];
    }

    if output.trim().is_empty() {
        return Err(DocumentParseError::NoExtractableText);
    }

    Ok(ExtractedDocument {
        text: output.trim_end().to_owned(),
        truncated,
        section_count,
    })
}

fn extract_word_text_runs(xml_fragment: &str) -> String {
    let mut text = String::new();
    let mut rest = xml_fragment;
    while let Some(pos) = rest.find("<w:") {
        let tag_slice = &rest[pos..];
        if tag_slice.starts_with("<w:tab") {
            text.push('\t');
            rest = &tag_slice[6..];
        } else if tag_slice.starts_with("<w:br") || tag_slice.starts_with("<w:cr") {
            text.push('\n');
            rest = &tag_slice[5..];
        } else if tag_slice.starts_with("<w:t>") || tag_slice.starts_with("<w:t ") {
            if let Some(gt) = tag_slice.find('>') {
                let content_start = &tag_slice[gt + 1..];
                if let Some(end) = content_start.find("</w:t>") {
                    text.push_str(&decode_xml_entities(&content_start[..end]));
                    rest = &content_start[end + 6..];
                    continue;
                }
            }
            break;
        } else {
            rest = &tag_slice[3..];
        }
    }
    text
}

fn extract_xlsx(bytes: &[u8]) -> Result<ExtractedDocument, DocumentParseError> {
    let mut archive = open_bounded_zip(bytes)?;
    let shared_strings = read_zip_xml_entry(&mut archive, "xl/sharedStrings.xml")
        .map(|xml| parse_xlsx_shared_strings(&xml))
        .unwrap_or_default();
    let sheet_names = read_zip_xml_entry(&mut archive, "xl/workbook.xml")
        .map(|xml| parse_xlsx_sheet_names(&xml))
        .unwrap_or_default();

    let mut worksheet_paths = Vec::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| DocumentParseError::MalformedDocument)?;
        let name = file
            .name()
            .map_err(|_| DocumentParseError::MalformedDocument)?
            .into_owned();
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            worksheet_paths.push(name);
        }
    }
    worksheet_paths.sort();
    if worksheet_paths.is_empty() {
        return Err(DocumentParseError::MalformedDocument);
    }

    let mut output = String::new();
    let mut section_count = 0usize;
    let mut truncated = false;

    for (sheet_idx, path) in worksheet_paths.iter().enumerate() {
        let sheet_xml = read_zip_xml_entry(&mut archive, path)?;
        let default_name = format!("Sheet{}", sheet_idx + 1);
        let sheet_name = sheet_names
            .get(sheet_idx)
            .map(String::as_str)
            .unwrap_or(&default_name);

        let mut sheet_has_rows = false;
        for row_chunk in sheet_xml.split("<row").skip(1) {
            let Some(row_end) = row_chunk.find("</row>") else {
                continue;
            };
            let row_xml = &row_chunk[..row_end];
            let row_number = extract_xml_attribute(row_xml, "r")
                .unwrap_or_else(|| "?".to_owned());

            let mut cells = Vec::new();
            for cell_chunk in row_xml.split("<c").skip(1) {
                let valid_c = cell_chunk
                    .as_bytes()
                    .first()
                    .is_some_and(|b| matches!(b, b' ' | b'>'));
                if !valid_c {
                    continue;
                }
                let cell_end = cell_chunk.find("</c>").unwrap_or(cell_chunk.len());
                let cell_xml = &cell_chunk[..cell_end];
                let cell_ref = extract_xml_attribute(cell_xml, "r")
                    .unwrap_or_else(|| "Cell".to_owned());
                let cell_type = extract_xml_attribute(cell_xml, "t").unwrap_or_default();

                let value = if cell_type == "inlineStr" {
                    extract_tag_text(cell_xml, "t")
                } else {
                    let raw = extract_tag_text(cell_xml, "v");
                    if cell_type == "s" {
                        raw.trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|idx| shared_strings.get(idx).cloned())
                            .unwrap_or(raw)
                    } else {
                        raw
                    }
                };
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    cells.push(format!("{cell_ref}: {trimmed}"));
                }
            }
            if !cells.is_empty() {
                sheet_has_rows = true;
                let line = format!(
                    "[Sheet: {sheet_name} | Row {row_number}] {}\n",
                    cells.join(" | ")
                );
                if !append_bounded(&mut output, &line) {
                    truncated = true;
                    break;
                }
            }
        }
        if sheet_has_rows {
            section_count += 1;
        }
        if truncated {
            break;
        }
    }

    if output.trim().is_empty() {
        return Err(DocumentParseError::NoExtractableText);
    }

    Ok(ExtractedDocument {
        text: output.trim_end().to_owned(),
        truncated,
        section_count,
    })
}

fn parse_xlsx_shared_strings(xml: &str) -> Vec<String> {
    let mut strings = Vec::new();
    for item in xml.split("<si").skip(1) {
        let Some(end) = item.find("</si>") else {
            continue;
        };
        let si_xml = &item[..end];
        let mut combined = String::new();
        let mut rest = si_xml;
        while let Some(t_pos) = rest.find("<t") {
            let after_t = &rest[t_pos..];
            let valid = after_t
                .as_bytes()
                .get(2)
                .is_some_and(|b| matches!(b, b'>' | b' '));
            if !valid {
                rest = &after_t[2..];
                continue;
            }
            let Some(gt) = after_t.find('>') else {
                break;
            };
            let content = &after_t[gt + 1..];
            let Some(close) = content.find("</t>") else {
                break;
            };
            combined.push_str(&decode_xml_entities(&content[..close]));
            rest = &content[close + 4..];
        }
        strings.push(combined);
    }
    strings
}

fn parse_xlsx_sheet_names(xml: &str) -> Vec<String> {
    let mut names = Vec::new();
    for chunk in xml.split("<sheet ").skip(1) {
        if let Some(end) = chunk.find('>')
            && let Some(name) = extract_xml_attribute(&chunk[..end], "name")
        {
            names.push(decode_xml_entities(&name));
        }
    }
    names
}

fn extract_pdf(bytes: &[u8]) -> Result<ExtractedDocument, DocumentParseError> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(DocumentParseError::MalformedDocument);
    }
    if find_subslice(bytes, b"/Encrypt").is_some() {
        return Err(DocumentParseError::EncryptedDocument);
    }

    let mut output = String::new();
    let mut page_number = 0usize;
    let mut stream_count = 0usize;
    let mut truncated = false;
    let mut offset = 0usize;

    while offset < bytes.len() && stream_count < MAXIMUM_PDF_STREAMS {
        let Some(stream_rel) = find_subslice(&bytes[offset..], b"stream") else {
            break;
        };
        let stream_pos = offset + stream_rel;
        let dict_start = offset.max(stream_pos.saturating_sub(512));
        let dict_slice = &bytes[dict_start..stream_pos];

        let mut data_start = stream_pos + 6;
        if bytes.get(data_start) == Some(&b'\r') && bytes.get(data_start + 1) == Some(&b'\n') {
            data_start += 2;
        } else if bytes.get(data_start) == Some(&b'\n') {
            data_start += 1;
        } else {
            offset = stream_pos + 6;
            continue;
        }

        let Some(end_rel) = find_subslice(&bytes[data_start..], b"endstream") else {
            break;
        };
        let mut data_end = data_start + end_rel;
        while data_end > data_start && matches!(bytes[data_end - 1], b'\r' | b'\n' | b' ') {
            data_end -= 1;
        }
        let raw_stream = &bytes[data_start..data_end];
        stream_count += 1;

        let decoded_stream = if find_subslice(dict_slice, b"/FlateDecode").is_some() {
            decompress_pdf_flate(raw_stream)
        } else {
            Some(raw_stream.to_vec())
        };

        if let Some(stream_bytes) = decoded_stream {
            let page_text = extract_pdf_text_operators(&stream_bytes);
            let trimmed = page_text.trim();
            if !trimmed.is_empty() {
                page_number += 1;
                let line = format!("[Page {page_number}]\n{trimmed}\n\n");
                if !append_bounded(&mut output, &line) {
                    truncated = true;
                    break;
                }
            }
        }

        offset = data_start + end_rel + 9;
    }

    if output.trim().is_empty() {
        return Err(DocumentParseError::NoExtractableText);
    }

    Ok(ExtractedDocument {
        text: output.trim_end().to_owned(),
        truncated,
        section_count: page_number,
    })
}

fn decompress_pdf_flate(raw: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(raw).take((MAXIMUM_PDF_STREAM_BYTES + 1) as u64);
    let mut out = Vec::new();
    if decoder.read_to_end(&mut out).is_ok() && out.len() <= MAXIMUM_PDF_STREAM_BYTES {
        return Some(out);
    }
    None
}

fn extract_pdf_text_operators(stream: &[u8]) -> String {
    let text = String::from_utf8_lossy(stream);
    let mut extracted = String::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'(' {
            let (literal, next_i) = parse_pdf_string_literal(bytes, i + 1);
            i = next_i;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if bytes[i..].starts_with(b"Tj")
                || bytes[i..].starts_with(b"'")
                || bytes[i..].starts_with(b"\"")
            {
                if !literal.trim().is_empty() {
                    if !extracted.is_empty() && !extracted.ends_with([' ', '\n']) {
                        extracted.push(' ');
                    }
                    extracted.push_str(&literal);
                }
                if bytes[i] == b'\'' || bytes[i] == b'"' {
                    extracted.push('\n');
                }
            }
        } else if bytes[i] == b'[' {
            let mut j = i + 1;
            let mut array_text = String::new();
            while j < bytes.len() && bytes[j] != b']' {
                if bytes[j] == b'(' {
                    let (lit, next_j) = parse_pdf_string_literal(bytes, j + 1);
                    array_text.push_str(&lit);
                    j = next_j;
                } else {
                    j += 1;
                }
            }
            if j < bytes.len() && bytes[j] == b']' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if bytes[j..].starts_with(b"TJ") && !array_text.trim().is_empty() {
                    if !extracted.is_empty() && !extracted.ends_with([' ', '\n']) {
                        extracted.push(' ');
                    }
                    extracted.push_str(&array_text);
                }
            }
            i = j;
        } else if bytes[i..].starts_with(b"T*") || bytes[i..].starts_with(b"Td") || bytes[i..].starts_with(b"TD") {
            if !extracted.ends_with('\n') && !extracted.is_empty() {
                extracted.push('\n');
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    extracted
}

fn parse_pdf_string_literal(bytes: &[u8], mut index: usize) -> (String, usize) {
    let mut out = String::new();
    let mut depth = 1usize;
    while index < bytes.len() && depth > 0 {
        let ch = bytes[index];
        if ch == b'\\' && index + 1 < bytes.len() {
            index += 1;
            match bytes[index] {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'(' => out.push('('),
                b')' => out.push(')'),
                b'\\' => out.push('\\'),
                other => out.push(char::from(other)),
            }
        } else if ch == b'(' {
            depth += 1;
            out.push('(');
        } else if ch == b')' {
            depth -= 1;
            if depth > 0 {
                out.push(')');
            }
        } else if ch >= 0x20 || ch == b'\n' || ch == b'\t' {
            out.push(char::from(ch));
        }
        index += 1;
    }
    (out, index)
}

fn extract_csv(bytes: &[u8]) -> Result<ExtractedDocument, DocumentParseError> {
    let content = std::str::from_utf8(bytes).map_err(|_| DocumentParseError::MalformedDocument)?;
    let mut output = String::new();
    let mut row_count = 0usize;
    let mut truncated = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        row_count += 1;
        let formatted = format!("[Row {row_count}] {trimmed}\n");
        if !append_bounded(&mut output, &formatted) {
            truncated = true;
            break;
        }
    }

    if output.is_empty() {
        return Err(DocumentParseError::NoExtractableText);
    }

    Ok(ExtractedDocument {
        text: output.trim_end().to_owned(),
        truncated,
        section_count: row_count,
    })
}

fn extract_html(bytes: &[u8]) -> Result<ExtractedDocument, DocumentParseError> {
    let content = std::str::from_utf8(bytes).map_err(|_| DocumentParseError::MalformedDocument)?;
    let converted = htmd::convert(content).map_err(|_| DocumentParseError::MalformedDocument)?;
    let trimmed = converted.trim();
    if trimmed.is_empty() {
        return Err(DocumentParseError::NoExtractableText);
    }
    let mut output = String::new();
    let truncated = !append_bounded(&mut output, trimmed);
    let section_count = output.lines().filter(|line| line.starts_with('#')).count().max(1);
    Ok(ExtractedDocument {
        text: output,
        truncated,
        section_count,
    })
}

fn open_bounded_zip(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, DocumentParseError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| DocumentParseError::MalformedDocument)?;
    if archive.len() == 0 || archive.len() > MAXIMUM_ZIP_ENTRIES {
        return Err(DocumentParseError::ArchiveLimitExceeded);
    }
    let mut total_uncompressed = 0u64;
    for index in 0..archive.len() {
        let raw = archive
            .by_index_raw(index)
            .map_err(|_| DocumentParseError::MalformedDocument)?;
        let raw_name = raw
            .name()
            .map_err(|_| DocumentParseError::MalformedDocument)?;
        if raw_name.contains("..") {
            return Err(DocumentParseError::MalformedDocument);
        }
        total_uncompressed = total_uncompressed.saturating_add(raw.size());
        if total_uncompressed > MAXIMUM_ZIP_UNCOMPRESSED_BYTES as u64 {
            return Err(DocumentParseError::ArchiveLimitExceeded);
        }
    }
    Ok(archive)
}

fn read_zip_xml_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    entry_name: &str,
) -> Result<String, DocumentParseError> {
    let file = archive
        .by_name(entry_name)
        .map_err(|_| DocumentParseError::MalformedDocument)?;
    if file.size() > MAXIMUM_SINGLE_XML_BYTES as u64 {
        return Err(DocumentParseError::ArchiveLimitExceeded);
    }
    let mut buffer = Vec::with_capacity(file.size().min(64 * 1024) as usize);
    file.take((MAXIMUM_SINGLE_XML_BYTES + 1) as u64)
        .read_to_end(&mut buffer)
        .map_err(|_| DocumentParseError::MalformedDocument)?;
    if buffer.len() > MAXIMUM_SINGLE_XML_BYTES {
        return Err(DocumentParseError::ArchiveLimitExceeded);
    }
    String::from_utf8(buffer).map_err(|_| DocumentParseError::MalformedDocument)
}

fn extract_xml_attribute(tag_fragment: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{attr_name}=\"");
    let start = tag_fragment.find(&pattern)? + pattern.len();
    let rest = &tag_fragment[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn extract_tag_text(xml_fragment: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let Some(start) = xml_fragment.find(&open) else {
        return String::new();
    };
    let after = &xml_fragment[start + open.len()..];
    let Some(gt) = after.find('>') else {
        return String::new();
    };
    let content = &after[gt + 1..];
    let Some(end) = content.find(&close) else {
        return String::new();
    };
    decode_xml_entities(&content[..end])
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn append_bounded(target: &mut String, addition: &str) -> bool {
    if target.len().saturating_add(addition.len()) <= MAXIMUM_DOCUMENT_OUTPUT_BYTES {
        target.push_str(addition);
        return true;
    }
    let remaining = MAXIMUM_DOCUMENT_OUTPUT_BYTES.saturating_sub(target.len());
    if remaining > 0 {
        let mut boundary = remaining.min(addition.len());
        while boundary > 0 && !addition.is_char_boundary(boundary) {
            boundary -= 1;
        }
        target.push_str(&addition[..boundary]);
    }
    false
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    use super::{DocumentParseError, extract_document};

    fn create_test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, content) in entries {
                zip.start_file(*name, options).expect("start zip entry");
                zip.write_all(content).expect("write zip entry");
            }
            zip.finish().expect("finish zip");
        }
        cursor.into_inner()
    }

    #[test]
    fn extracts_docx_paragraphs_and_tables_with_source_locations() {
        let document_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                    <w:p><w:r><w:t>Quarterly Architecture Review</w:t></w:r></w:p>
                    <w:tbl>
                        <w:tr>
                            <w:tc><w:p><w:r><w:t>Module</w:t></w:r></w:p></w:tc>
                            <w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc>
                        </w:tr>
                    </w:tbl>
                </w:body>
            </w:document>"#;
        let bytes = create_test_zip(&[("word/document.xml", document_xml)]);
        let extracted = extract_document("report.docx", &bytes).expect("docx should parse");
        assert!(extracted.text.contains("[Paragraph 1] Quarterly Architecture Review"));
        assert!(extracted.text.contains("[Table 1 | Row 1] Module | Status"));
        assert_eq!(extracted.section_count, 2);
    }

    #[test]
    fn extracts_xlsx_sheets_and_shared_strings_with_cell_references() {
        let workbook_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
            <workbook><sheets><sheet name="Budget" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let shared_strings_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
            <sst><si><t>Compute</t></si></sst>"#;
        let sheet_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
            <worksheet><sheetData>
                <row r="1">
                    <c r="A1" t="s"><v>0</v></c>
                    <c r="B1"><v>420</v></c>
                </row>
            </sheetData></worksheet>"#;
        let bytes = create_test_zip(&[
            ("xl/workbook.xml", workbook_xml),
            ("xl/sharedStrings.xml", shared_strings_xml),
            ("xl/worksheets/sheet1.xml", sheet_xml),
        ]);
        let extracted = extract_document("metrics.xlsx", &bytes).expect("xlsx should parse");
        assert!(extracted.text.contains("[Sheet: Budget | Row 1] A1: Compute | B1: 420"));
        assert_eq!(extracted.section_count, 1);
    }

    #[test]
    fn extracts_pdf_page_streams_and_rejects_encrypted_pdfs() {
        let valid_pdf = b"%PDF-1.7\n1 0 obj\n<< /Length 44 >>\nstream\nBT /F1 12 Tf 72 720 Td (Hello OpenChat PDF) Tj ET\nendstream\nendobj\n%%EOF";
        let extracted = extract_document("guide.pdf", valid_pdf).expect("pdf should parse");
        assert!(extracted.text.contains("[Page 1]"));
        assert!(extracted.text.contains("Hello OpenChat PDF"));

        let encrypted_pdf = b"%PDF-1.7\n1 0 obj\n<< /Encrypt 2 0 R >>\nendobj\n%%EOF";
        assert_eq!(
            extract_document("locked.pdf", encrypted_pdf),
            Err(DocumentParseError::EncryptedDocument)
        );
    }
}
