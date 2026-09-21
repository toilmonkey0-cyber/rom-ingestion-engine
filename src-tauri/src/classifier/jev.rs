use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use serde::{Deserialize, Serialize};

use crate::models::{ClassificationSource, GameClassification};

/// Default TypeSafe Jev System One production endpoint.
pub const JEV_DEFAULT_ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

/// TypeSafe Jev System One model identifier.
pub const JEV_MODEL: &str = "jev-latest";

/// Represents a typed evaluation question for TypeSafe Jev System One.
/// Supports `choice` and `noul` question primitives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JevQuestion {
    Choice {
        instructions: String,
        criteria: HashMap<String, String>,
    },
    Noul {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<HashMap<String, String>>,
    },
}

impl JevQuestion {
    /// Creates a choice primitive question with instructions and selectable criteria.
    pub fn choice(instructions: impl Into<String>, criteria: HashMap<String, String>) -> Self {
        Self::Choice {
            instructions: instructions.into(),
            criteria,
        }
    }

    /// Creates a noul primitive question evaluating probability/likelihood.
    pub fn noul(instructions: impl Into<String>) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: None,
        }
    }

    /// Creates a noul primitive question with explicit criteria descriptions.
    pub fn noul_with_criteria(
        instructions: impl Into<String>,
        criteria: HashMap<String, String>,
    ) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: Some(criteria),
        }
    }

    /// Returns the question instructions.
    pub fn instructions(&self) -> &str {
        match self {
            Self::Choice { instructions, .. } => instructions,
            Self::Noul { instructions, .. } => instructions,
        }
    }

    /// Returns criteria map if present.
    pub fn criteria(&self) -> Option<&HashMap<String, String>> {
        match self {
            Self::Choice { criteria, .. } => Some(criteria),
            Self::Noul { criteria, .. } => criteria.as_ref(),
        }
    }

    /// Returns the primitive type name ("choice" or "noul").
    pub fn primitive(&self) -> &'static str {
        match self {
            Self::Choice { .. } => "choice",
            Self::Noul { .. } => "noul",
        }
    }
}

/// Request payload sent to TypeSafe Jev System One API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JevRequest {
    pub state: HashMap<String, serde_json::Value>,
    pub model: String,
    pub questions: HashMap<String, JevQuestion>,
}

/// A choice answer returned by TypeSafe Jev System One.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceAnswer {
    pub choice: String,
    #[serde(default)]
    pub probabilities: HashMap<String, f64>,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    1.0
}

/// A noul (probability 0.0 - 1.0) answer returned by TypeSafe Jev System One.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoulAnswer {
    pub noul: f64,
}

/// Evaluated answer from TypeSafe Jev System One (`choice` or `noul`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JevAnswer {
    Choice(ChoiceAnswer),
    Noul(NoulAnswer),
}

impl JevAnswer {
    /// Returns reference to `ChoiceAnswer` if this answer is of type `choice`.
    pub fn as_choice(&self) -> Option<&ChoiceAnswer> {
        match self {
            JevAnswer::Choice(c) => Some(c),
            _ => None,
        }
    }

    /// Returns reference to `NoulAnswer` if this answer is of type `noul`.
    pub fn as_noul(&self) -> Option<&NoulAnswer> {
        match self {
            JevAnswer::Noul(n) => Some(n),
            _ => None,
        }
    }
}

/// Token usage metadata from TypeSafe Jev System One.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JevUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
}

/// Structured response payload from TypeSafe Jev System One.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JevResponse {
    pub model: String,
    pub answers: HashMap<String, JevAnswer>,
    #[serde(default)]
    pub usage: Option<JevUsage>,
}

/// Errors occurring during TypeSafe Jev evaluation.
#[derive(Debug, thiserror::Error)]
pub enum JevError {
    #[error("HTTP network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("API error (status {status_code}): {message}")]
    ApiError {
        status_code: u16,
        message: String,
    },

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Missing expected answer for question: {0}")]
    MissingAnswer(String),

    #[error("Invalid answer type for '{question}': expected {expected}, got {actual}")]
    InvalidAnswerType {
        question: String,
        expected: String,
        actual: String,
    },

    #[error("Failed to parse classification: {0}")]
    ParseError(String),
}

/// Constructs a `JevRequest` for evaluating messy ROM filenames using `jev-latest`.
pub fn build_jev_request(filename: &str, folder: &str) -> JevRequest {
    let mut state = HashMap::new();
    state.insert(
        "filename".to_string(),
        serde_json::Value::String(filename.to_string()),
    );
    state.insert(
        "folder".to_string(),
        serde_json::Value::String(folder.to_string()),
    );

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let clean_stem = crate::classifier::redump::clean_canonical_title(stem);
    state.insert(
        "clean_filename".to_string(),
        serde_json::Value::String(clean_stem),
    );

    let mut questions = HashMap::new();

    // 1. Platform classification
    let mut platform_criteria = HashMap::new();
    platform_criteria.insert(
        "psx".to_string(),
        "Sony PlayStation / PSX / PS1".to_string(),
    );
    platform_criteria.insert("saturn".to_string(), "Sega Saturn".to_string());
    platform_criteria.insert("dreamcast".to_string(), "Sega Dreamcast".to_string());
    platform_criteria.insert("segacd".to_string(), "Sega CD / Mega CD".to_string());
    platform_criteria.insert(
        "pcecd".to_string(),
        "PC Engine CD / TurboGrafx-CD".to_string(),
    );
    questions.insert(
        "platform".to_string(),
        JevQuestion::choice(
            "Identify the target retro gaming console platform for this disc image based on filename and directory clues.",
            platform_criteria,
        ),
    );

    // 2. Is Multi-disc probability
    questions.insert(
        "is_multidisc".to_string(),
        JevQuestion::noul(
            "Evaluate the probability that this disc image belongs to a multi-disc game release (0.0 = single disc, 1.0 = multi-disc).",
        ),
    );

    // 3. Disc number
    let mut disc_criteria = HashMap::new();
    disc_criteria.insert(
        "single".to_string(),
        "Single-disc game release (not part of a multi-disc set)".to_string(),
    );
    for d in 1..=8 {
        disc_criteria.insert(d.to_string(), format!("Disc {}", d));
    }
    questions.insert(
        "disc_number".to_string(),
        JevQuestion::choice(
            "Identify the disc number for this game disc if it is part of a multi-disc set, or select 'single' if it is a standalone single-disc release.",
            disc_criteria,
        ),
    );

    // 4. Release region
    let mut region_criteria = HashMap::new();
    region_criteria.insert("USA".to_string(), "North America / USA release".to_string());
    region_criteria.insert("Europe".to_string(), "European release".to_string());
    region_criteria.insert("Japan".to_string(), "Japanese release".to_string());
    region_criteria.insert("World".to_string(), "Worldwide / Global release".to_string());
    questions.insert(
        "region".to_string(),
        JevQuestion::choice(
            "Identify the primary release region for this game dump based on filename tags.",
            region_criteria,
        ),
    );

    JevRequest {
        state,
        model: JEV_MODEL.to_string(),
        questions,
    }
}

/// Parses a TypeSafe Jev System One JSON response into a `GameClassification`.
pub fn parse_jev_response(
    response_json: &str,
    original_filename: &str,
) -> Result<GameClassification, JevError> {
    let resp: JevResponse = serde_json::from_str(response_json)?;

    let mut confidences: Vec<f64> = Vec::new();

    // 1. Platform
    let platform_ans = resp
        .answers
        .get("platform")
        .ok_or_else(|| JevError::MissingAnswer("platform".to_string()))?;
    let platform = match platform_ans {
        JevAnswer::Choice(c) => {
            let parsed = crate::classifier::redump::parse_platform(&c.choice);
            if parsed == crate::models::Platform::Unknown {
                // The model answered outside the expected vocabulary: keep the
                // answer's uncertainty visible instead of a confident Unknown,
                // so downstream `needs_review` triggers.
                confidences.push(c.confidence * 0.25);
            } else {
                confidences.push(c.confidence);
            }
            parsed
        }
        JevAnswer::Noul(_) => {
            return Err(JevError::InvalidAnswerType {
                question: "platform".to_string(),
                expected: "choice".to_string(),
                actual: "noul".to_string(),
            });
        }
    };

    // 2. Is Multi-disc
    let is_multidisc_from_jev = if let Some(ans) = resp.answers.get("is_multidisc") {
        match ans {
            JevAnswer::Noul(n) => {
                let is_multi = n.noul >= 0.5;
                let cert = if is_multi { n.noul } else { 1.0 - n.noul };
                confidences.push(cert);
                Some(is_multi)
            }
            JevAnswer::Choice(c) => {
                confidences.push(c.confidence);
                let is_multi = matches!(
                    c.choice.to_ascii_lowercase().as_str(),
                    "true" | "yes" | "1" | "multidisc"
                );
                Some(is_multi)
            }
        }
    } else {
        None
    };

    // 3. Disc number
    let disc_number_from_jev = if let Some(ans) = resp.answers.get("disc_number") {
        match ans {
            JevAnswer::Choice(c) => {
                confidences.push(c.confidence);
                let lower = c.choice.to_ascii_lowercase();
                if lower == "single" || lower == "none" || lower == "0" || lower == "n/a" {
                    None
                } else if let Ok(num) = c.choice.trim().parse::<u8>() {
                    Some(num)
                } else {
                    let (extracted, _) = crate::classifier::redump::extract_disc_info(&c.choice);
                    extracted
                }
            }
            JevAnswer::Noul(n) => {
                confidences.push(n.noul);
                None
            }
        }
    } else {
        None
    };

    // 4. Region — model answers are validated against the known region
    // vocabulary: anything else (typos, hallucinations, path-unsafe strings)
    // falls back to the filename, then "Unknown". The raw answer never
    // becomes a filesystem path component.
    let region = if let Some(ans) = resp.answers.get("region") {
        match ans {
            JevAnswer::Choice(c) => {
                let normalized = crate::classifier::redump::extract_region(&format!("({})", c.choice))
                    .unwrap_or_else(|| {
                        crate::classifier::redump::extract_region(original_filename)
                            .unwrap_or_else(|| "Unknown".to_string())
                    });
                confidences.push(c.confidence);
                normalized
            }
            JevAnswer::Noul(_) => crate::classifier::redump::extract_region(original_filename)
                .unwrap_or_else(|| "Unknown".to_string()),
        }
    } else {
        crate::classifier::redump::extract_region(original_filename)
            .unwrap_or_else(|| "Unknown".to_string())
    };

    // Check filename tags for additional context / fallback
    let (file_disc_num, file_total_discs) =
        crate::classifier::redump::extract_disc_info(original_filename);
    let disc_number = disc_number_from_jev.or(file_disc_num);

    let is_multidisc = if let Some(im) = is_multidisc_from_jev {
        if let Some(d) = disc_number {
            if d > 1 {
                true
            } else {
                im
            }
        } else {
            im
        }
    } else if let Some(d) = disc_number {
        d > 0
    } else {
        false
    };

    let total_discs = file_total_discs;

    // Extract canonical title from original filename stem
    let file_stem = Path::new(original_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(original_filename);
    let canonical_title = crate::classifier::redump::clean_canonical_title(file_stem);

    let confidence = if confidences.is_empty() {
        0.5
    } else {
        (confidences.iter().sum::<f64>() / confidences.len() as f64) as f32
    };

    Ok(GameClassification {
        canonical_title,
        platform,
        region,
        is_multidisc,
        disc_number,
        total_discs,
        confidence,
        source: ClassificationSource::JevAI,
    })
}

/// Client for calling TypeSafe Jev System One (`jev-latest`) API.
#[derive(Debug, Clone)]
pub struct JevClient {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    initial_backoff: Duration,
    max_retries: usize,
}

impl JevClient {
    /// Builds the shared HTTP client: bounded connect/total timeouts so a slow
    /// or hung endpoint cannot stall the scan command indefinitely.
    fn build_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(45))
            .build()
            .expect("reqwest client with timeouts builds successfully")
    }

    /// Creates a new `JevClient` with default production endpoint `https://api.typesafe.ai/v1/systemone`.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: JEV_DEFAULT_ENDPOINT.to_string(),
            client: Self::build_http_client(),
            initial_backoff: Duration::from_millis(200),
            max_retries: 3,
        }
    }

    /// Creates a `JevClient` with a custom base URL (useful for testing against a mock HTTP server).
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: Self::build_http_client(),
            initial_backoff: Duration::from_millis(50),
            max_retries: 3,
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

    /// Evaluates a ROM filename and folder context asynchronously via TypeSafe Jev System One.
    /// Employs exponential backoff on HTTP 429/529 rate limit or capacity codes.
    pub async fn evaluate(
        &self,
        filename: &str,
        folder: &str,
    ) -> Result<GameClassification, JevError> {
        let payload = build_jev_request(filename, folder);
        let mut last_error: Option<JevError> = None;
        let mut delay = self.initial_backoff;

        for attempt in 0..self.max_retries {
            let request_builder = self
                .client
                .post(&self.base_url)
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {}", self.api_key),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(&payload);

            let res = match request_builder.send().await {
                Ok(response) => response,
                Err(err) => {
                    last_error = Some(JevError::NetworkError(err));
                    if attempt + 1 < self.max_retries {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                        continue;
                    } else {
                        return Err(last_error.unwrap());
                    }
                }
            };

            let status = res.status();
            if status.as_u16() == 429 || status.as_u16() == 529 {
                let err_text = res.text().await.unwrap_or_default();
                last_error = Some(JevError::ApiError {
                    status_code: status.as_u16(),
                    message: err_text,
                });
                if attempt + 1 < self.max_retries {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                } else {
                    return Err(last_error.unwrap());
                }
            }

            if !status.is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(JevError::ApiError {
                    status_code: status.as_u16(),
                    message: err_text,
                });
            }

            let body_text = res.text().await?;
            return parse_jev_response(&body_text, filename);
        }

        Err(last_error.unwrap_or_else(|| {
            JevError::ParseError("Maximum retry attempts exceeded".to_string())
        }))
    }

    /// Alias for `evaluate`.
    pub async fn evaluate_game_filename(
        &self,
        filename: &str,
        folder: &str,
    ) -> Result<GameClassification, JevError> {
        self.evaluate(filename, folder).await
    }
}
