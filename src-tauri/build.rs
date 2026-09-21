use std::fs;
use std::path::Path;

fn ensure_icon() {
    let icon_path = Path::new("icons/icon.ico");
    if !icon_path.exists() {
        if let Some(parent) = icon_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut bytes = Vec::new();
        // ICO Header: reserved (2), type=1 (2), count=1 (2)
        bytes.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
        // Directory Entry (16 bytes)
        let image_size: u32 = 40 + 1024 + 64;
        let offset: u32 = 22;
        bytes.extend_from_slice(&[
            16, 16, 0, 0, // width, height, colors, reserved
            1, 0, 32, 0, // planes, bpp
        ]);
        bytes.extend_from_slice(&image_size.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        // BITMAPINFOHEADER (40 bytes)
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&16i32.to_le_bytes());
        bytes.extend_from_slice(&32i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1024u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        // XOR mask (1024 bytes)
        bytes.resize(bytes.len() + 1024, 0);
        // AND mask (64 bytes)
        bytes.resize(bytes.len() + 64, 0);

        let _ = fs::write(icon_path, bytes);
    }
}

fn main() {
    ensure_icon();
    tauri_build::build()
}
