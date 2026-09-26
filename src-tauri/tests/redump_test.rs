use rom_ingest_core::classifier::redump::{
    clean_canonical_title, extract_disc_info, extract_region, parse_platform, RedumpDatabase,
    RedumpEntry,
};
use rom_ingest_core::models::{ClassificationSource, Platform};

#[test]
fn test_redump_lookup_known_hash() {
    let mut db = RedumpDatabase::new_in_memory();
    db.insert(
        "a1b2c3d4e5f67890123456789abcdef012345678",
        "Metal Gear Solid",
        Platform::Psx,
        "USA",
        true,
        Some(1),
        Some(2),
    );

    let result = db.lookup_sha1("a1b2c3d4e5f67890123456789abcdef012345678");
    assert!(result.is_some());
    let info = result.unwrap();
    assert_eq!(info.canonical_title, "Metal Gear Solid");
    assert_eq!(info.platform, Platform::Psx);
    assert_eq!(info.region, "USA");
    assert!(info.is_multidisc);
    assert_eq!(info.disc_number, Some(1));
    assert_eq!(info.total_discs, Some(2));
    assert_eq!(info.confidence, 1.0);
    assert_eq!(info.source, ClassificationSource::RedumpCache);
}

#[test]
fn test_redump_lookup_case_insensitive() {
    let mut db = RedumpDatabase::new_in_memory();
    db.insert(
        "A1B2C3D4E5F67890123456789ABCDEF012345678",
        "Castlevania: Symphony of the Night",
        Platform::Psx,
        "USA",
        false,
        None,
        None,
    );

    // Query with lowercase
    let result = db.lookup_sha1("a1b2c3d4e5f67890123456789abcdef012345678");
    assert!(result.is_some());
    assert_eq!(result.unwrap().canonical_title, "Castlevania: Symphony of the Night");

    // Query with mixed case and leading/trailing whitespace
    let result_ws = db.lookup_sha1("  A1b2C3d4E5f67890123456789aBcDeF012345678 \n");
    assert!(result_ws.is_some());
    assert_eq!(result_ws.unwrap().canonical_title, "Castlevania: Symphony of the Night");
}

#[test]
fn test_redump_lookup_unknown_hash() {
    let db = RedumpDatabase::new_in_memory();
    let result = db.lookup_sha1("0000000000000000000000000000000000000000");
    assert!(result.is_none());
}

#[test]
fn test_redump_builtin_database() {
    let db = RedumpDatabase::with_builtin_data();
    assert!(!db.is_empty());
    assert!(db.len() >= 10);

    // Test PSX built-in
    let mgs_disc1 = db.lookup_sha1("223b7a702bdf2aa1c6aa9d6b2c29c8e82fa08d91");
    assert!(mgs_disc1.is_some(), "MGS Disc 1 should be in built-ins");
    let mgs = mgs_disc1.unwrap();
    assert_eq!(mgs.canonical_title, "Metal Gear Solid");
    assert_eq!(mgs.platform, Platform::Psx);
    assert_eq!(mgs.disc_number, Some(1));
    assert_eq!(mgs.confidence, 1.0);
    assert_eq!(mgs.source, ClassificationSource::RedumpCache);

    // Test Saturn built-in
    let pds = db.lookup_sha1("8899aabbccddeeff00112233445566778899aabb");
    assert!(pds.is_some(), "Panzer Dragoon Saga should be in built-ins");
    assert_eq!(pds.unwrap().platform, Platform::Saturn);

    // Test Dreamcast built-in
    let shenmue = db.lookup_sha1("3344556677889900aabbccddeeff001122334455");
    assert!(shenmue.is_some(), "Shenmue should be in built-ins");
    assert_eq!(shenmue.unwrap().platform, Platform::Dreamcast);
}

#[test]
fn test_redump_load_csv_with_header() {
    let mut db = RedumpDatabase::new_in_memory();
    let csv_data = r#"sha1,title,platform,region,is_multidisc,disc_number,total_discs
1111111111111111111111111111111111111111,Final Fantasy VII,psx,USA,true,1,3
2222222222222222222222222222222222222222,Final Fantasy VII,psx,USA,true,2,3
3333333333333333333333333333333333333333,Final Fantasy VII,psx,USA,true,3,3
"#;

    let loaded = db.load_csv_or_tsv(csv_data.as_bytes()).expect("Failed to load CSV");
    assert_eq!(loaded, 3);
    assert_eq!(db.len(), 3);

    let ff7_disc2 = db.lookup_sha1("2222222222222222222222222222222222222222").unwrap();
    assert_eq!(ff7_disc2.canonical_title, "Final Fantasy VII");
    assert_eq!(ff7_disc2.platform, Platform::Psx);
    assert_eq!(ff7_disc2.disc_number, Some(2));
    assert_eq!(ff7_disc2.total_discs, Some(3));
}

#[test]
fn test_redump_load_tsv() {
    let mut db = RedumpDatabase::new_in_memory();
    let tsv_data = "sha1\ttitle\tplatform\tregion\tis_multidisc\tdisc_number\ttotal_discs\n\
4444444444444444444444444444444444444444\tSonic CD\tsegacd\tUSA\tfalse\t\t\n\
5555555555555555555555555555555555555555\tSnatcher\tsegacd\tUSA\tfalse\t1\t1\n";

    let loaded = db.load_csv_or_tsv(tsv_data.as_bytes()).expect("Failed to load TSV");
    assert_eq!(loaded, 2);

    let sonic = db.lookup_sha1("4444444444444444444444444444444444444444").unwrap();
    assert_eq!(sonic.canonical_title, "Sonic CD");
    assert_eq!(sonic.platform, Platform::SegaCd);
    assert!(!sonic.is_multidisc);
    assert_eq!(sonic.disc_number, None);

    let snatcher = db.lookup_sha1("5555555555555555555555555555555555555555").unwrap();
    assert_eq!(snatcher.canonical_title, "Snatcher");
    assert_eq!(snatcher.platform, Platform::SegaCd);
}

#[test]
fn test_redump_load_csv_positional_no_header() {
    let mut db = RedumpDatabase::new_in_memory();
    let csv_data = "6666666666666666666666666666666666666666,Chrono Cross,psx,USA,true,1,2\n";

    let loaded = db.load_csv_or_tsv(csv_data.as_bytes()).expect("Failed to load CSV");
    assert_eq!(loaded, 1);

    let chrono = db.lookup_sha1("6666666666666666666666666666666666666666").unwrap();
    assert_eq!(chrono.canonical_title, "Chrono Cross");
    assert_eq!(chrono.platform, Platform::Psx);
    assert_eq!(chrono.disc_number, Some(1));
    assert_eq!(chrono.total_discs, Some(2));
}

#[test]
fn test_redump_load_infer_multidisc_from_title() {
    let mut db = RedumpDatabase::new_in_memory();
    // Disc information not explicitly provided in columns, but in title
    let csv_data = "7777777777777777777777777777777777777777,Resident Evil 2 (USA) (Disc 2),psx,USA\n";

    let loaded = db.load_csv_or_tsv(csv_data.as_bytes()).expect("Failed to load CSV");
    assert_eq!(loaded, 1);

    let re2 = db.lookup_sha1("7777777777777777777777777777777777777777").unwrap();
    assert_eq!(re2.canonical_title, "Resident Evil 2");
    assert!(re2.is_multidisc);
    assert_eq!(re2.disc_number, Some(2));
}

#[test]
fn test_redump_entry_direct_methods() {
    let mut db = RedumpDatabase::new();
    assert!(db.is_empty());
    assert_eq!(db.len(), 0);

    let entry = RedumpEntry {
        canonical_title: "Nights into Dreams...".to_string(),
        platform: Platform::Saturn,
        region: "USA".to_string(),
        is_multidisc: false,
        disc_number: None,
        total_discs: None,
        serial: None,
    };

    db.insert_entry("aaaabbbbccccddddeeeeffff0000111122223333", entry.clone());
    assert_eq!(db.len(), 1);
    assert!(!db.is_empty());
    assert!(db.contains_sha1("AAAABBBBCCCCDDDDEEEEFFFF0000111122223333"));
    assert!(!db.contains_sha1("1234567890123456789012345678901234567890"));

    let retrieved = db.get_entry("aaaabbbbccccddddeeeeffff0000111122223333");
    assert_eq!(retrieved, Some(&entry));
}

#[test]
fn test_redump_load_semicolon_delimited() {
    let mut db = RedumpDatabase::new();
    let data = "sha1;title;platform;region;is_multidisc;disc_number;total_discs\n\
8888888888888888888888888888888888888888;Lords of Thunder;pcecd;USA;false;;\n";

    let loaded = db.load_csv_or_tsv(data.as_bytes()).unwrap();
    assert_eq!(loaded, 1);

    let lot = db.lookup_sha1("8888888888888888888888888888888888888888").unwrap();
    assert_eq!(lot.canonical_title, "Lords of Thunder");
    assert_eq!(lot.platform, Platform::PceCd);
}

#[test]
fn test_redump_load_with_comments_and_malformed_lines() {
    let mut db = RedumpDatabase::new();
    let data = r#"# This is a comment
// Another comment

9999999999999999999999999999999999999999,Valid Game,psx,USA,false,,
# Ignored line
,Missing Sha1,psx,USA,false,,
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,,psx,USA,false,,
"#;

    let loaded = db.load_csv(data.as_bytes()).unwrap();
    assert_eq!(loaded, 1);
    assert!(db.contains_sha1("9999999999999999999999999999999999999999"));
}

#[test]
fn test_platform_parsing_variations() {
    assert_eq!(parse_platform("PSX"), Platform::Psx);
    assert_eq!(parse_platform("ps1"), Platform::Psx);
    assert_eq!(parse_platform("Sony PlayStation"), Platform::Psx);
    assert_eq!(parse_platform("Saturn"), Platform::Saturn);
    assert_eq!(parse_platform("Sega Saturn"), Platform::Saturn);
    assert_eq!(parse_platform("Dreamcast"), Platform::Dreamcast);
    assert_eq!(parse_platform("DC"), Platform::Dreamcast);
    assert_eq!(parse_platform("Sega CD"), Platform::SegaCd);
    assert_eq!(parse_platform("megacd"), Platform::SegaCd);
    assert_eq!(parse_platform("pcecd"), Platform::PceCd);
    assert_eq!(parse_platform("PC Engine CD"), Platform::PceCd);
    assert_eq!(parse_platform("Turbografx-16 CD"), Platform::PceCd);
    assert_eq!(parse_platform("unknown_sys"), Platform::Unknown);
}

#[test]
fn test_clean_canonical_title_and_tags() {
    assert_eq!(
        clean_canonical_title("Metal Gear Solid (USA) (Disc 1) (v1.1)"),
        "Metal Gear Solid"
    );
    assert_eq!(
        clean_canonical_title("Castlevania - Symphony of the Night [!]"),
        "Castlevania - Symphony of the Night"
    );
    assert_eq!(
        clean_canonical_title("Shenmue (USA) (Disc 2 of 4)"),
        "Shenmue"
    );

    let (d, t) = extract_disc_info("Game (Disc 2 of 4)");
    assert_eq!(d, Some(2));
    assert_eq!(t, Some(4));

    let (d2, t2) = extract_disc_info("Game (CD 1/2)");
    assert_eq!(d2, Some(1));
    assert_eq!(t2, Some(2));

    assert_eq!(extract_region("Game (USA)"), Some("USA".to_string()));
    assert_eq!(extract_region("Game (Japan, En)"), Some("Japan".to_string()));
    assert_eq!(extract_region("Game"), None);
}

#[test]
fn test_redump_load_dat_xml() {
    // sha1("track one data") = 0f48a8b40ad0afa2fc2c9d8f659e3f5b1b0e2d3c (fixture value)
    let dat = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Redump.org - Sony - Playstation</name>
    <description>Redump.org - Sony - Playstation</description>
  </header>
  <game name="Klonoa (USA)">
    <category>Games</category>
    <rom name="Klonoa (USA) (Track 1).bin" size="100" crc="00000000" md5="00000000000000000000000000000000" sha1="0f48a8b40ad0afa2fc2c9d8f659e3f5b1b0e2d3c"/>
    <rom name="Klonoa (USA) (Track 2).bin" size="50" crc="00000000" md5="00000000000000000000000000000000" sha1="1111111111111111111111111111111111111111"/>
  </game>
  <game name="Panzer Dragoon Saga (USA) (Disc 1)">
    <rom name="track01.bin" size="10" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/>
    <rom name="track02.bin" size="10" sha1="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"/>
  </game>
  <game name="Broken Entry (Europe)">
    <rom name="track01.bin" size="10" sha1="not-a-valid-hash"/>
  </game>
</datafile>
"#;

    let mut db = RedumpDatabase::new();
    let count = db.load_dat_xml(dat.as_bytes()).expect("parse DAT");
    // Two valid entries (first rom of each game); the invalid hash is skipped.
    assert_eq!(count, 2);
    assert_eq!(db.len(), 2);

    // Platform comes from the DAT header via loose keyword inference.
    let klonoa = db.lookup_sha1("0F48A8B40AD0AFA2FC2C9D8F659E3F5B1B0E2D3C").expect("case-insensitive lookup");
    assert_eq!(klonoa.platform, Platform::Psx);
    assert_eq!(klonoa.canonical_title, "Klonoa");
    assert_eq!(klonoa.region, "USA");
    assert!(!klonoa.is_multidisc);
    assert_eq!(klonoa.source, ClassificationSource::RedumpCache);

    // Only the FIRST rom of a game is indexed (track-1 semantics).
    assert!(!db.contains_sha1("1111111111111111111111111111111111111111"));
    assert!(!db.contains_sha1("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));

    // Disc metadata parsed from the game name; second game platform inherits header.
    let pds = db.get_entry("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("second entry");
    assert_eq!(pds.platform, Platform::Psx);
    assert_eq!(pds.disc_number, Some(1));
    assert!(pds.is_multidisc);
}

#[test]
fn test_parse_platform_loose_from_dat_headers() {
    use rom_ingest_core::classifier::redump::parse_platform_loose;
    assert_eq!(parse_platform_loose("Redump.org - Sony - Playstation"), Some(Platform::Psx));
    assert_eq!(parse_platform_loose("Sega - Saturn"), Some(Platform::Saturn));
    assert_eq!(parse_platform_loose("Sega - Mega CD - Sega CD"), Some(Platform::SegaCd));
    assert_eq!(parse_platform_loose("NEC - PC Engine CD - TurboGrafx-CD"), Some(Platform::PceCd));
    assert_eq!(parse_platform_loose("Sega Dreamcast"), Some(Platform::Dreamcast));
    assert_eq!(parse_platform_loose("Nintendo - Game Boy"), None);
}

/// Live end-to-end DAT verification: downloads the real Redump PSX DAT from
/// redump.org and parses it. Run with:
/// `cargo test --test redump_test -- --ignored`
#[tokio::test]
#[ignore = "performs a real network download"]
async fn live_download_and_parse_real_redump_psx_dat() {
    let client = reqwest::Client::builder()
        .user_agent("rom-ingest-dat-verify/0.1.0")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .unwrap();
    let bytes = client
        .get("http://redump.org/datfile/psx/")
        .send()
        .await
        .expect("request redump")
        .error_for_status()
        .expect("redump status")
        .bytes()
        .await
        .expect("body");

    let cursor = std::io::Cursor::new(&bytes[..]);
    let mut archive = zip::ZipArchive::new(cursor).expect("zip");
    let mut dat_bytes: Vec<u8> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        if entry.name().to_ascii_lowercase().ends_with(".dat") {
            std::io::Read::read_to_end(&mut entry, &mut dat_bytes).unwrap();
            break;
        }
    }
    assert!(!dat_bytes.is_empty(), "zip contained a .dat");

    let mut db = RedumpDatabase::new();
    let count = db
        .load_dat_xml(std::io::Cursor::new(&dat_bytes[..]))
        .expect("parse real DAT");
    // The PSX datfile currently lists ~10,900 games.
    assert!(count > 9_000, "unexpectedly few entries parsed: {}", count);

    // The very first sha1 in the DAT text must be present and classified.
    let text = String::from_utf8_lossy(&dat_bytes).to_string();
    let start = text.find("sha1=\"").expect("dat has sha1 attrs") + 6;
    let hash: String = text[start..].chars().take(40).collect();
    let sample = db.lookup_sha1(&hash).expect("first DAT hash resolves");
    assert_eq!(sample.platform, Platform::Psx);
    assert!(!sample.canonical_title.is_empty());
    assert_eq!(sample.source, ClassificationSource::RedumpCache);
}
