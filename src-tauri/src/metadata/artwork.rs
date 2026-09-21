use crate::models::Platform;

/// Base URL of the libretro thumbnail service — keyless, deterministic
/// URLs derived from the canonical game title.
pub const LIBRETRO_THUMBNAILS_BASE: &str = "https://thumbnails.libretro.com";

/// Maps a platform to the libretro thumbnail system directory.
pub fn libretro_system_dir(platform: Platform) -> Option<&'static str> {
    match platform {
        Platform::Psx => Some("Sony - PlayStation"),
        Platform::Saturn => Some("Sega - Saturn"),
        Platform::Dreamcast => Some("Sega - Dreamcast"),
        Platform::SegaCd => Some("Sega - Mega CD - Sega CD"),
        Platform::PceCd => Some("NEC - PC Engine CD - TurboGrafx-CD"),
        Platform::Unknown => None,
    }
}

/// Percent-encodes a path segment, keeping only RFC 3986 unreserved
/// characters plus `/` (system dirs contain slashes).
pub fn percent_encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Ordered candidate URLs for a game's artwork.
///
/// libretro thumbnail files are named `<game> (<region>).png` when a region
/// applies (e.g. `Final Fantasy VII (USA).png`), so the region-qualified
/// name is tried first, then the bare title, across box art, title screens,
/// and in-game snapshots. Empty when the platform has no thumbnail directory.
pub fn thumbnail_candidates(
    platform: Platform,
    canonical_title: &str,
    region: &str,
    base_url: &str,
) -> Vec<String> {
    let Some(system) = libretro_system_dir(platform) else {
        return Vec::new();
    };
    let system = percent_encode(system);
    let title = percent_encode(canonical_title.trim());
    if title.is_empty() {
        return Vec::new();
    }

    let region_clean = region.trim();
    let region_known = !region_clean.is_empty()
        && !region_clean.eq_ignore_ascii_case("unknown")
        && !region_clean.contains('/');
    let mut names = Vec::new();
    if region_known {
        names.push(format!("{} ({})", canonical_title.trim(), region_clean));
    }
    names.push(canonical_title.trim().to_string());

    let mut urls = Vec::new();
    for category in ["Named_Boxarts", "Named_Titles", "Named_Snaps"] {
        for name in &names {
            urls.push(format!(
                "{}/{}/{}/{}.png",
                base_url,
                system,
                category,
                percent_encode(name)
            ));
        }
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_dir_mapping() {
        assert_eq!(libretro_system_dir(Platform::Psx), Some("Sony - PlayStation"));
        assert_eq!(libretro_system_dir(Platform::Unknown), None);
    }

    #[test]
    fn test_candidate_urls_and_order() {
        let urls = thumbnail_candidates(
            Platform::Psx,
            "Final Fantasy VII",
            "USA",
            "https://example.test",
        );
        // Region-qualified box art first, then bare title, per category.
        assert_eq!(urls.len(), 6);
        assert_eq!(
            urls[0],
            "https://example.test/Sony%20-%20PlayStation/Named_Boxarts/Final%20Fantasy%20VII%20%28USA%29.png"
        );
        assert_eq!(
            urls[1],
            "https://example.test/Sony%20-%20PlayStation/Named_Boxarts/Final%20Fantasy%20VII.png"
        );
        assert!(urls[2].contains("Named_Titles"));
        assert!(urls[5].contains("Named_Snaps"));

        // Unknown regions skip the qualified variant.
        let plain = thumbnail_candidates(Platform::Psx, "Game", "Unknown", "https://example.test");
        assert_eq!(plain.len(), 3);
        // Path-unsafe region strings are never used.
        let bad = thumbnail_candidates(Platform::Psx, "Game", "../etc", "https://example.test");
        assert_eq!(bad.len(), 3);
    }

    #[test]
    fn test_percent_encoding_specials() {
        assert_eq!(percent_encode("A B&C/D"), "A%20B%26C/D");
        assert_eq!(percent_encode("Crash Bandicoot"), "Crash%20Bandicoot");
    }

    #[test]
    fn test_unknown_platform_has_no_candidates() {
        assert!(thumbnail_candidates(Platform::Unknown, "Game", "USA", "https://x").is_empty());
        assert!(thumbnail_candidates(Platform::Psx, "   ", "USA", "https://x").is_empty());
    }
}
