use std::io::Read;
use std::path::PathBuf;

/// Catalog serials read from disc bytes. A hit against a user DAT beats the filename.
pub fn read_catalog_serial(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);

    if let Some(serial) = capture_psx(&text) {
        return Some(serial);
    }
    if let Some(serial) = capture_hardware_product(bytes, b"SEGA SEGAKATANA", 0x40, 10) {
        return Some(serial);
    }
    if let Some(serial) = capture_hardware_product(bytes, b"SEGA SEGASATURN", 0x20, 10) {
        return Some(serial);
    }
    if text.contains("SEGADISCSYSTEM") {
        if let Some(serial) = first_match(
            &text,
            r"GM\s+([A-Z][A-Z0-9]{0,3}-[0-9]{3,5}[A-Z0-9-]*)",
        ) {
            return Some(serial);
        }
    }
    first_match(&text, r"(?i)SERIAL\s*[:=]\s*([A-Z0-9_\-\.]+)")
}

/// Dreamcast product number is 10 bytes at IP.BIN offset 0x40 (`HDR-0176`, `MK-51000`, `T-9714N`).
/// Saturn product number is 10 bytes at offset 0x20 (`MK-81076`, `T-12345`).
fn capture_hardware_product(bytes: &[u8], marker: &[u8], field_offset: usize, field_len: usize) -> Option<String> {
    let start = find_bytes(bytes, marker)?;
    let field = read_field(bytes, start + field_offset, field_len);
    if let Some(product) = field.filter(|value| product_code(value)) {
        return Some(product);
    }
    let window = &bytes[start..];
    let text = String::from_utf8_lossy(window);
    first_match(
        &text,
        r"\b([A-Z][A-Z0-9]{0,3}-[0-9]{3,5}[A-Z]?)\b",
    )
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn read_field(bytes: &[u8], offset: usize, len: usize) -> Option<String> {
    let end = offset.checked_add(len)?;
    let raw = bytes.get(offset..end)?;
    let text = String::from_utf8_lossy(raw);
    let trimmed = text.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn product_code(value: &str) -> bool {
    regex::Regex::new(r"^[A-Z][A-Z0-9]{0,3}-[0-9]{3,5}[A-Z]?$")
        .ok()
        .is_some_and(|re| re.is_match(value))
}

fn capture_psx(text: &str) -> Option<String> {
    let marker = "cdrom:\\";
    let lower = text.to_ascii_lowercase();
    let idx = lower.find(marker)?;
    let rest = &text[idx + marker.len()..];
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect();
    if token.len() >= 4 {
        Some(token.trim_end_matches('.').to_string())
    } else {
        None
    }
}

fn first_match(text: &str, pattern: &str) -> Option<String> {
    let re = regex::Regex::new(pattern).ok()?;
    let caps = re.captures(text)?;
    let value = caps
        .get(1)
        .or_else(|| caps.get(0))
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| !s.is_empty())?;
    Some(value)
}

pub fn read_serial_from_tracks(tracks: &[PathBuf]) -> Option<String> {
    for track in tracks {
        let Ok(file) = std::fs::File::open(track) else {
            continue;
        };
        let mut limited = file.take(2 * 1024 * 1024);
        let mut buf = Vec::new();
        if limited.read_to_end(&mut buf).is_err() {
            continue;
        }
        if let Some(serial) = read_catalog_serial(&buf) {
            return Some(serial);
        }
    }
    None
}
