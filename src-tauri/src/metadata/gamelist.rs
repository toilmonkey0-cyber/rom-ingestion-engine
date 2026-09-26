use std::fs;
use std::io;
use std::path::Path;

/// One `<game>` entry in an EmulationStation-style `gamelist.xml`.
#[derive(Debug, Clone, PartialEq)]
pub struct GamelistEntry {
    /// ROM path relative to the gamelist location (e.g. `./Game (USA).m3u`).
    pub path: String,
    /// Display name shown in the frontend.
    pub name: String,
    /// Optional relative box-art path (e.g. `./media/images/Game (USA).png`).
    pub image: Option<String>,
    /// Optional description text shown in the frontend detail view.
    pub desc: Option<String>,
}

/// Escapes a string for XML text content / double-quoted attribute values.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Renders the full `gamelist.xml` document for a set of entries.
pub fn generate_gamelist_xml(entries: &[GamelistEntry]) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\n<gameList>\n");
    for e in entries {
        out.push_str("  <game>\n");
        out.push_str(&format!("    <path>{}</path>\n", xml_escape(&e.path)));
        out.push_str(&format!("    <name>{}</name>\n", xml_escape(&e.name)));
        if let Some(ref image) = e.image {
            out.push_str(&format!("    <image>{}</image>\n", xml_escape(image)));
        }
        if let Some(ref desc) = e.desc {
            if !desc.trim().is_empty() {
                out.push_str(&format!("    <desc>{}</desc>\n", xml_escape(desc)));
            }
        }
        out.push_str("  </game>\n");
    }
    out.push_str("</gameList>\n");
    out
}

/// Writes `gamelist.xml` to `path`, creating parent directories as needed.
pub fn write_gamelist<P: AsRef<Path>>(path: P, entries: &[GamelistEntry]) -> Result<(), io::Error> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, generate_gamelist_xml(entries).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_escape() {
        let entries = vec![
            GamelistEntry {
                path: "./R-Type & Ryu (USA).m3u".into(),
                name: "R-Type <Final> \"Deluxe\"".into(),
                image: Some("./media/images/R-Type & Ryu (USA).png".into()),
                desc: None,
            },
            GamelistEntry {
                path: "./Game.chd".into(),
                name: "Game".into(),
                image: None,
                desc: Some("A classic <adventure> & more".into()),
            },
        ];
        let xml = generate_gamelist_xml(&entries);
        assert!(xml.starts_with("<?xml version=\"1.0\"?>"));
        assert!(xml.contains("<path>./R-Type &amp; Ryu (USA).m3u</path>"));
        assert!(xml.contains("<name>R-Type &lt;Final&gt; &quot;Deluxe&quot;</name>"));
        assert!(xml.contains("<image>./media/images/R-Type &amp; Ryu (USA).png</image>"));
        // No image element when art is absent
        assert!(xml.contains("<name>Game</name>"));
        let game_section = xml.split("<name>Game</name>").nth(1).unwrap();
        assert!(!game_section.contains("<image>"));
        assert!(xml.contains("<desc>A classic &lt;adventure&gt; &amp; more</desc>"));
    }

    #[test]
    fn test_write_gamelist_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("gamelist.xml");
        write_gamelist(
            &path,
            &[GamelistEntry { path: "./a.chd".into(), name: "A".into(), image: None, desc: None }],
        )
        .unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("./a.chd"));
    }
}
