use std::path::Path;
use std::time::Duration;
use serde::{Deserialize, Serialize};

use crate::models::{ClassificationSource, GameClassification};

/// Default endpoint of a local Needle engine sidecar
/// (`needle.exe --model needle3.cact --tools tools.json --serve`).
/// The sidecar must be spawned with `NEEDLE_TELEMETRY=0` and
/// `DO_NOT_TRACK=1` so a local classifier never phones home.
pub const NEEDLE_DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8080";

/// Tool name the sidecar's `tools.json` must expose for ROM classification.
pub const NEEDLE_CLASSIFY_TOOL: &str = "classify_rom";

/// Regions accepted verbatim from the model's grammar-constrained answers.
const REGION_VOCABULARY: &[&str] = &["USA", "Europe", "Japan", "World"];

/// A single tool call returned by the Needle engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeedleCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Response envelope of the Needle serve-mode HTTP API (`POST /complete`).
///
/// `function_calls` holds grounded calls the engine is confident in;
/// `suppressed_calls` holds calls the confidence gate withheld. Both carry
/// full arguments, so a withheld classification is usable at a discount.
/// `confidence` is `None` for weights built through local LoRA fine-tuning
/// (the calibration head ships only with base/platform-trained weights).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeedleServeResponse {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub function_calls: Vec<NeedleCall>,
    #[serde(default)]
    pub suppressed_calls: Vec<NeedleCall>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// Errors occurring during Needle evaluation.
#[derive(Debug, thiserror::Error)]
pub enum NeedleError {
    #[error("HTTP network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("API error (status {status_code}): {message}")]
    ApiError {
        status_code: u16,
        message: String,
    },

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// The engine returned no call (and no withheld call): the input is
    /// off-topic for the toolset, e.g. firmware or junk masquerading as a
    /// disc image. The caller should use the heuristic tier.
    #[error("Needle produced no grounded classification for this input")]
    NoCall,

    #[error("Failed to parse classification: {0}")]
    ParseError(String),
}

/// Parses a Needle serve-mode response into a `GameClassification`.
///
/// Answers are validated against the same vocabulary the Jev parser uses:
/// the decode grammar makes out-of-vocabulary answers impossible in a
/// correct sidecar, and this parser keeps that true even against a buggy
/// or hostile one. Nothing from the model becomes a filesystem component.
pub fn parse_needle_response(
    response_json: &str,
    original_filename: &str,
) -> Result<GameClassification, NeedleError> {
    let resp: NeedleServeResponse = serde_json::from_str(response_json)?;

    let (call, withheld) = if let Some(c) = resp.function_calls.first() {
        (c, false)
    } else if let Some(c) = resp.suppressed_calls.first() {
        (c, true)
    } else {
        return Err(NeedleError::NoCall);
    };
    if call.name != NEEDLE_CLASSIFY_TOOL {
        return Err(NeedleError::ParseError(format!(
            "unexpected tool call: {}",
            call.name
        )));
    }
    let args = &call.arguments;

    // Base confidence: calibrated score when present; withheld calls are
    // discounted; local-LoRA weights (None) get a neutral default.
    let mut confidence = match resp.confidence {
        Some(c) => {
            if withheld {
                c * 0.5
            } else {
                c
            }
        }
        None => 0.75,
    };

    // 1. Platform
    let platform = args
        .get("platform")
        .and_then(|v| v.as_str())
        .map(crate::classifier::redump::parse_platform)
        .unwrap_or(crate::models::Platform::Unknown);
    if platform == crate::models::Platform::Unknown {
        // Out-of-vocabulary answer: keep the uncertainty visible so
        // downstream `needs_review` triggers (same policy as the Jev parser).
        confidence *= 0.25;
    }

    // 2. Disc number: "single" -> None; "1".."8" -> Some(n); anything else
    // is dropped and the filename tags decide.
    let disc_number_from_model = args
        .get("disc_number")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            let lower = s.trim().to_ascii_lowercase();
            if lower == "single" || lower == "none" || lower == "0" || lower == "n/a" {
                None
            } else if let Ok(num) = s.trim().parse::<u8>() {
                (1..=8).contains(&num).then_some(num)
            } else {
                let (extracted, _) = crate::classifier::redump::extract_disc_info(s);
                extracted
            }
        });

    // 3. Multi-disc: trust the boolean, else infer from disc numbers.
    let is_multidisc_from_model = args.get("is_multidisc").and_then(|v| v.as_bool());

    // 4. Region: exact vocabulary match first, then the normal Redump tag
    // extraction used everywhere else (model answer, filename, Unknown).
    let region = args
        .get("region")
        .and_then(|v| v.as_str())
        .map(|s| {
            if REGION_VOCABULARY.contains(&s) {
                s.to_string()
            } else {
                crate::classifier::redump::extract_region(&format!("({})", s))
                    .or_else(|| {
                        crate::classifier::redump::extract_region(original_filename)
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            }
        })
        .unwrap_or_else(|| {
            crate::classifier::redump::extract_region(original_filename)
                .unwrap_or_else(|| "Unknown".to_string())
        });

    // Filename tags for title, disc fallback, and totals (same as Jev tier).
    let (file_disc_num, total_discs) =
        crate::classifier::redump::extract_disc_info(original_filename);
    let disc_number = disc_number_from_model.or(file_disc_num);
    let is_multidisc = match is_multidisc_from_model {
        Some(m) => m,
        None => disc_number.map(|d| d > 0).unwrap_or(false),
    };

    let file_stem = Path::new(original_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(original_filename);
    let canonical_title = crate::classifier::redump::clean_canonical_title(file_stem);

    Ok(GameClassification {
        canonical_title,
        platform,
        region,
        is_multidisc,
        disc_number,
        total_discs,
        confidence: confidence.clamp(0.0, 1.0) as f32,
        source: ClassificationSource::NeedleAI,
    })
}

/// Request body for `POST /complete`.
#[derive(Debug, Serialize)]
struct CompleteRequest<'a> {
    input: &'a str,
}

/// Client for a local Needle engine sidecar (`--serve` HTTP API).
///
/// The engine owns ONE process-global conversation, so every file is
/// classified as `POST /reset` followed by `POST /complete`; without the
/// reset, answers leak across files ("carries over from history"). The
/// input text is the bare filename, optionally prefixed with the parent
/// folder (`Folder/File.cue`) because folder names are a strong platform
/// signal; instructions live in the tool description, and verbs in the
/// input leak into grounded fields, so none are added here.
#[derive(Debug, Clone)]
pub struct NeedleClient {
    base_url: String,
    client: reqwest::Client,
    initial_backoff: Duration,
    max_retries: usize,
}

impl NeedleClient {
    fn build_http_client() -> reqwest::Client {
        // Local sidecar: fail fast so a dead endpoint cannot stall a scan.
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("reqwest client with timeouts builds successfully")
    }

    /// Creates a `NeedleClient` targeting `http://127.0.0.1:8080`.
    pub fn new() -> Self {
        Self {
            base_url: NEEDLE_DEFAULT_ENDPOINT.to_string(),
            client: Self::build_http_client(),
            initial_backoff: Duration::from_millis(100),
            max_retries: 2,
        }
    }

    /// Creates a `NeedleClient` with a custom sidecar base URL
    /// (e.g. a non-default `--port`).
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            client: Self::build_http_client(),
            initial_backoff: Duration::from_millis(50),
            max_retries: 2,
        }
    }

    /// Sets the initial exponential backoff delay.
    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    /// Sets the maximum retry attempts.
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Returns the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Classifies a ROM filename and folder context via the local sidecar.
    pub async fn evaluate(
        &self,
        filename: &str,
        folder: &str,
    ) -> Result<GameClassification, NeedleError> {
        let input = if folder.trim().is_empty() {
            filename.to_string()
        } else {
            format!("{}/{}", folder.trim(), filename)
        };

        let mut last_error: Option<NeedleError> = None;
        let mut delay = self.initial_backoff;

        for attempt in 0..self.max_retries.max(1) {
            // Fresh conversation per file.
            let reset = self
                .client
                .post(format!("{}/reset", self.base_url))
                .send()
                .await;
            if let Err(err) = reset {
                last_error = Some(NeedleError::NetworkError(err));
                if attempt + 1 < self.max_retries {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                break;
            }

            let res = self
                .client
                .post(format!("{}/complete", self.base_url))
                .json(&CompleteRequest { input: &input })
                .send()
                .await;
            match res {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        let message = response.text().await.unwrap_or_default();
                        return Err(NeedleError::ApiError {
                            status_code: status.as_u16(),
                            message,
                        });
                    }
                    let body = response.text().await?;
                    return parse_needle_response(&body, filename);
                }
                Err(err) => {
                    last_error = Some(NeedleError::NetworkError(err));
                    if attempt + 1 < self.max_retries {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                        continue;
                    }
                    break;
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| NeedleError::ParseError("Maximum retry attempts exceeded".into())))
    }

    /// Alias for `evaluate`.
    pub async fn evaluate_game_filename(
        &self,
        filename: &str,
        folder: &str,
    ) -> Result<GameClassification, NeedleError> {
        self.evaluate(filename, folder).await
    }
}
