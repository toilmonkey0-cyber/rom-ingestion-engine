use std::fs;
use std::io;
use std::path::Path;

/// Generates the text content for an M3U playlist file from a list of relative disc paths.
///
/// Ensures all entries use forward slashes (`/`), even on Windows, as RetroArch,
/// frontends, and emulators require forward slashes in M3U playlists.
pub fn generate_m3u_content<S: AsRef<str>>(disc_relative_paths: &[S]) -> String {
    let mut content = String::new();
    for entry in disc_relative_paths {
        let normalized = entry.as_ref().trim().replace('\\', "/");
        if !normalized.is_empty() {
            content.push_str(&normalized);
            content.push('\n');
        }
    }
    content
}

/// Writes an M3U playlist file to the specified target path.
///
/// Creates any necessary parent directories automatically before writing.
pub fn write_m3u_file<P: AsRef<Path>, S: AsRef<str>>(
    m3u_file_path: P,
    disc_relative_paths: &[S],
) -> Result<(), io::Error> {
    let path = m3u_file_path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let content = generate_m3u_content(disc_relative_paths);
    fs::write(path, content.as_bytes())
}

/// Parses an M3U playlist file content into a list of relative disc paths.
///
/// Skips empty lines and comments (lines starting with `#`).
pub fn parse_m3u_content(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.replace('\\', "/"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_m3u_content_basic() {
        let entries = vec![
            ".discs/Game (Disc 1).chd",
            ".discs/Game (Disc 2).chd",
        ];
        assert_eq!(
            generate_m3u_content(&entries),
            ".discs/Game (Disc 1).chd\n.discs/Game (Disc 2).chd\n"
        );
    }

    #[test]
    fn test_generate_m3u_content_replaces_backslashes() {
        let entries = vec![
            r".discs\Game (Disc 1).chd",
            r".discs\Game (Disc 2).chd",
        ];
        assert_eq!(
            generate_m3u_content(&entries),
            ".discs/Game (Disc 1).chd\n.discs/Game (Disc 2).chd\n"
        );
    }

    #[test]
    fn test_parse_m3u_content() {
        let m3u = "# EXT-M3U\n.discs/Game (Disc 1).chd\n\n.discs\\Game (Disc 2).chd\n";
        let parsed = parse_m3u_content(m3u);
        assert_eq!(
            parsed,
            vec![
                ".discs/Game (Disc 1).chd",
                ".discs/Game (Disc 2).chd",
            ]
        );
    }
}
