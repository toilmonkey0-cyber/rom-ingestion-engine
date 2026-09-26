use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use regex::Regex;
use tokio::io::{AsyncReadExt, BufReader};

static PROGRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Compressing,\s+([0-9.]+)%\s+complete").expect("valid chdman progress regex")
});

pub const CHD_HEADER_MAGIC: &[u8; 8] = b"MComprHD";

#[derive(Debug, thiserror::Error)]
pub enum ChdmanError {
    #[error("chdman binary not found")]
    BinaryNotFound,

    #[error("chdman process failed with code {code:?}: {stderr}")]
    ProcessFailed {
        code: Option<i32>,
        stderr: String,
    },

    #[error("CHD header verification failed: file does not start with MComprHD magic bytes")]
    HeaderVerificationFailed,

    #[error("{0}")]
    UnsupportedInput(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Parses a line of chdman output to extract the compression percentage.
/// Matches patterns like `Compressing, 42.8% complete...`
pub fn parse_chdman_progress_line(line: &str) -> Option<f32> {
    let caps = PROGRESS_REGEX.captures(line)?;
    caps.get(1)?.as_str().parse::<f32>().ok()
}

/// Verifies whether the first 8 bytes of the file match the CHD magic bytes (`b"MComprHD"`).
pub fn verify_chd_header<P: AsRef<Path>>(path: P) -> Result<bool, std::io::Error> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 8];
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == CHD_HEADER_MAGIC),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e),
    }
}

/// Guard ensuring that partial `.part` files are removed on error, panic, or future cancellation.
struct PartCleanupGuard {
    path: PathBuf,
    completed: bool,
}

impl Drop for PartCleanupGuard {
    fn drop(&mut self) {
        if !self.completed && self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChdmanRunner {
    binary_path: PathBuf,
}

impl Default for ChdmanRunner {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ChdmanRunner {
    pub fn new(binary_path: Option<PathBuf>) -> Self {
        Self {
            binary_path: binary_path.unwrap_or_else(|| PathBuf::from("chdman")),
        }
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub async fn version_string(&self) -> String {
        let Ok(output) = tokio::process::Command::new(&self.binary_path).output().await else {
            return "unknown".to_string();
        };
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        crate::chdman::downloader::parse_version_string(&text).unwrap_or_else(|| "unknown".to_string())
    }

    /// Converts a disc image descriptor (e.g. .cue, .gdi, .iso) to a compressed CHD file.
    ///
    /// Writes progress updates as a float percentage [0.0 - 100.0] to `on_progress`.
    /// Operates non-destructively by writing to `<output>.part` first, validating the
    /// `MComprHD` header upon zero exit status, and atomically renaming to `<output>`.
    /// Automatically cleans up `.part` files on failure or cancellation.
    pub async fn convert<F>(
        &self,
        input_descriptor: &Path,
        output_chd: &Path,
        on_progress: F,
    ) -> Result<(), ChdmanError>
    where
        F: Fn(f32) + Send + Sync + 'static,
    {
        let command = crate::paths::chdman_command_for_input(input_descriptor)
            .map_err(ChdmanError::UnsupportedInput)?;

        if output_chd.is_file() && matches!(verify_chd_header(output_chd), Ok(true)) {
            on_progress(100.0);
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = output_chd.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Build .part path: e.g. /path/to/game.chd -> /path/to/game.chd.part
        let mut part_os = output_chd.as_os_str().to_os_string();
        part_os.push(".part");
        let part_path = PathBuf::from(part_os);

        // Remove any stale leftover .part file
        let _ = tokio::fs::remove_file(&part_path).await;

        // Setup RAII cleanup guard
        let mut guard = PartCleanupGuard {
            path: part_path.clone(),
            completed: false,
        };

        let on_progress = Arc::new(on_progress);

        // Prepare command: createcd or createdvd -i <input> -o <part_path> -f
        let mut cmd = tokio::process::Command::new(&self.binary_path);
        cmd.arg(command)
            .arg("-i")
            .arg(input_descriptor)
            .arg("-o")
            .arg(&part_path)
            .arg("-f")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ChdmanError::BinaryNotFound);
            }
            Err(e) => return Err(ChdmanError::Io(e)),
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let progress_err = on_progress.clone();
        let stderr_task = tokio::spawn(async move {
            let mut err_str = String::new();
            if let Some(err_reader) = stderr {
                let mut reader = BufReader::new(err_reader);
                let mut buf = [0u8; 1024];
                let mut pending = String::new();
                loop {
                    let n = match reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    err_str.push_str(&chunk);
                    pending.push_str(&chunk);
                    while let Some(pos) = pending.find(['\r', '\n']) {
                        let line = pending[..pos].to_string();
                        if let Some(pct) = parse_chdman_progress_line(&line) {
                            (*progress_err)(pct);
                        }
                        pending = pending[pos + 1..].to_string();
                    }
                }
                if let Some(pct) = parse_chdman_progress_line(pending.trim()) {
                    (*progress_err)(pct);
                }
            }
            err_str
        });

        if let Some(stdout_pipe) = stdout {
            let mut reader = BufReader::new(stdout_pipe);
            let mut buf = [0u8; 1024];
            let mut pending = String::new();

            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                let s = String::from_utf8_lossy(&buf[..n]);
                pending.push_str(&s);

                while let Some(pos) = pending.find(['\r', '\n']) {
                    let line = &pending[..pos];
                    if let Some(pct) = parse_chdman_progress_line(line) {
                        (*on_progress)(pct);
                    }
                    pending = pending[pos + 1..].to_string();
                }
            }

            let rem = pending.trim();
            if !rem.is_empty() {
                if let Some(pct) = parse_chdman_progress_line(rem) {
                    (*on_progress)(pct);
                }
            }
        }

        let status = child.wait().await?;
        let stderr_output = stderr_task.await.unwrap_or_default();

        if !status.success() {
            return Err(ChdmanError::ProcessFailed {
                code: status.code(),
                stderr: stderr_output,
            });
        }

        // Verify CHD header
        let is_valid = verify_chd_header(&part_path)?;
        if !is_valid {
            return Err(ChdmanError::HeaderVerificationFailed);
        }

        // Atomically rename .part to output_chd
        tokio::fs::rename(&part_path, output_chd).await?;

        // Succeeded: disarm cleanup guard
        guard.completed = true;

        Ok(())
    }

    pub async fn verify_output(&self, chd: &Path) -> Result<(), ChdmanError> {
    if self.binary_path.as_os_str().is_empty() || !self.binary_path.exists() {
        return Err(ChdmanError::BinaryNotFound);
    }
    let output = tokio::process::Command::new(&self.binary_path)
        .arg("verify")
        .arg("-i")
        .arg(chd)
        .output()
        .await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ChdmanError::ProcessFailed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
    }
}

/// Convenience function executing CHD conversion using default runner on PATH.
pub async fn execute_chd_conversion<F>(
    input: &Path,
    output: &Path,
    on_progress: F,
) -> Result<(), ChdmanError>
where
    F: Fn(f32) + Send + Sync + 'static,
{
    ChdmanRunner::default().convert(input, output, on_progress).await
}
