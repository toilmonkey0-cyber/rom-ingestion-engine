use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();
    let env_mode = env::var("MOCK_CHDMAN_MODE").unwrap_or_default();

    // `chdman verify -i <path>`: exit 0 when the file exists and carries the
    // CHD magic; exit 1 otherwise. MOCK_CHDMAN_VERIFY_FAIL or a path
    // containing "verify_fail" forces a verification failure.
    if args.len() > 1 && args[1] == "verify" {
        let input = args
            .iter()
            .position(|a| a == "-i")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();
        let force_fail = env::var("MOCK_CHDMAN_VERIFY_FAIL").is_ok() || input.contains("verify_fail");

        if force_fail {
            eprintln!("Error: CHD verification failed (mock)");
            std::process::exit(1);
        }

        let mut magic = [0u8; 8];
        let readable = std::fs::File::open(&input)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic))
            .is_ok();
        if readable && &magic == b"MComprHD" {
            println!("chdman - verify (mock): OK");
            std::process::exit(0);
        }
        eprintln!("Error: raw CHD header read failure or bad magic");
        std::process::exit(1);
    }

    // Arguments expected from ChdmanRunner:
    // createcd -i <input> -o <part_path> -f
    let mut input_path = String::new();
    let mut out_path = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-i" && i + 1 < args.len() {
            input_path = args[i + 1].clone();
            i += 1;
        } else if args[i] == "-o" && i + 1 < args.len() {
            out_path = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        }
        i += 1;
    }

    let mode = if !env_mode.is_empty() {
        env_mode
    } else if input_path.contains("bad_header") {
        "bad_header".to_string()
    } else if input_path.contains("failure") {
        "failure".to_string()
    } else if input_path.contains("stderr_progress") {
        "stderr_progress".to_string()
    } else {
        "success".to_string()
    };

    match mode.as_str() {
        "success" => {
            if input_path.contains("slowdisc") {
                let started = PathBuf::from(&input_path).with_extension("started");
                let _ = File::create(&started);
                std::thread::sleep(std::time::Duration::from_millis(1200));
            }
            println!("chdman - MAME Compressed Hunks of Data (CHD) manager 0.268");
            println!("Compressing, 25.0% complete...");
            println!("Compressing, 50.0% complete...");
            println!("Compressing, 100.0% complete...");
            if let Some(path) = out_path {
                let mut f = File::create(path).unwrap();
                f.write_all(b"MComprHD\x00\x00\x00\x05mockcompresseddata").unwrap();
            }
            std::process::exit(0);
        }
        "bad_header" => {
            println!("Compressing, 50.0% complete...");
            println!("Compressing, 100.0% complete...");
            if let Some(path) = out_path {
                let mut f = File::create(path).unwrap();
                f.write_all(b"BADMAGIC\x00\x00\x00\x05mockcompresseddata").unwrap();
            }
            std::process::exit(0);
        }
        "stderr_progress" => {
            eprint!("Compressing, 40.0% complete...\r");
            eprintln!("Compressing, 100.0% complete...");
            if let Some(path) = out_path {
                let mut f = File::create(path).unwrap();
                f.write_all(b"MComprHD\x00\x00\x00\x05mockcompresseddata").unwrap();
            }
            std::process::exit(0);
        }
        "failure" => {
            eprintln!("Error: input file could not be parsed as CUE or GDI");
            if let Some(path) = out_path {
                let mut f = File::create(path).unwrap();
                f.write_all(b"partial junk").unwrap();
            }
            std::process::exit(1);
        }
        _ => {
            eprintln!("Unknown mock mode: {}", mode);
            std::process::exit(2);
        }
    }
}
