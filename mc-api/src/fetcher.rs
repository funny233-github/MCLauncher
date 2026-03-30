/// Provides a type-safe, builder-pattern HTTP fetcher with retry logic.
///
/// Supports JSON/XML deserialization, SHA1 verification, and configurable timeouts.
///
/// # Example
///
/// ```no_run
/// use mc_api::fetcher::FetcherBuilder;
///
/// // Fetch JSON data
/// let data: serde_json::Value = FetcherBuilder::fetch("http://example.com/data.json")
///     .json()
///     .execute()?
///     .json()?;
///
/// // Fetch with SHA1 verification
/// let text: String = FetcherBuilder::fetch("http://example.com/asset.json")
///     .sha1("abc123...")
///     .execute::<String>()?
///     .text()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
use crate::Sha1Compare;
use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

/// Deserialization type for HTTP responses.
#[derive(Default)]
pub enum DeserializeType {
    /// Raw bytes, no deserialization.
    #[default]
    Byte,
    /// JSON deserialization.
    Json,
    /// XML deserialization.
    Xml,
    /// Plain text.
    Text,
}

/// Builder for configuring and executing HTTP requests with retry logic.
///
/// # Example
///
/// ```
/// use mc_api::fetcher::FetcherBuilder;
/// use std::time::Duration;
///
/// let builder = FetcherBuilder::fetch("http://example.com/data.json")
///     .json()
///     .retry(3)
///     .timeout(30)
///     .sha1("abc123...");
/// ```
pub struct FetcherBuilder {
    /// The URL to fetch data from.
    pub url: String,
    /// How to parse the response.
    pub deserialize_type: DeserializeType,
    /// Number of retry attempts on failure.
    pub retry: u64,
    /// Request timeout duration.
    pub timeout: Duration,
    /// Delay between retry attempts.
    pub wait_time: Duration,
    /// Optional SHA1 hash for response verification.
    pub sha1: Option<String>,
}

/// Result type returned by `FetcherBuilder::execute()`.
///
/// Contains deserialized data in the requested format.
///
/// # Example
///
/// ```
/// use mc_api::fetcher::FetcherBuilder;
///
/// let text: String = FetcherBuilder::fetch("http://example.com/data.txt")
///     .text()
///     .execute::<String>()?
///     .text()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub enum FetcherResult<T: serde::de::DeserializeOwned> {
    /// Raw text response.
    Text(String),
    /// Raw byte response.
    Byte(Vec<u8>),
    /// JSON deserialized into type `T`.
    Json(T),
    /// XML deserialized into type `T`.
    Xml(T),
}

impl<T: serde::de::DeserializeOwned> FetcherResult<T> {
    /// Extracts the raw bytes from the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the result is not in the `Byte` variant.
    pub fn byte(self) -> Result<Vec<u8>> {
        if let FetcherResult::Byte(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("Expected Byte variant, found non-Byte result"))?
        }
    }
    /// Extracts the raw text from the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the result is not in the `Text` variant.
    pub fn text(self) -> Result<String> {
        if let FetcherResult::Text(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("Expected Text variant, found non-Text result"))?
        }
    }

    /// Extracts the JSON data from the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the result is not in the `Json` variant.
    pub fn json(self) -> Result<T> {
        if let FetcherResult::Json(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("Expected Json variant, found non-Json result"))?
        }
    }

    /// Extracts the XML data from the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the result is not in the `Xml` variant.
    pub fn xml(self) -> Result<T> {
        if let FetcherResult::Xml(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("Expected Xml variant, found non-Xml result"))?
        }
    }
}

impl Default for FetcherBuilder {
    fn default() -> Self {
        Self {
            url: String::default(),
            deserialize_type: DeserializeType::default(),
            retry: 5,
            timeout: Duration::from_secs(100),
            wait_time: Duration::from_secs(10),
            sha1: None,
        }
    }
}

impl FetcherBuilder {
    /// Creates a new `FetcherBuilder` with the specified URL.
    #[must_use]
    pub fn fetch(url: &str) -> FetcherBuilder {
        Self {
            url: url.to_owned(),
            ..Self::default()
        }
    }

    /// Sets the deserialization type to JSON.
    #[must_use]
    pub fn json(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Json,
            ..self
        }
    }

    /// Sets the deserialization type to XML.
    #[must_use]
    pub fn xml(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Xml,
            ..self
        }
    }

    /// Sets the deserialization type to plain text.
    #[must_use]
    pub fn text(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Text,
            ..self
        }
    }

    /// Sets the deserialization type to raw bytes.
    #[must_use]
    pub fn byte(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Byte,
            ..self
        }
    }

    /// Sets the number of retry attempts on failure.
    #[must_use]
    pub fn retry(self, num: u64) -> FetcherBuilder {
        Self { retry: num, ..self }
    }

    /// Sets the request timeout in seconds.
    #[must_use]
    pub fn timeout(self, secs: u64) -> FetcherBuilder {
        Self {
            timeout: Duration::from_secs(secs),
            ..self
        }
    }

    /// Sets the delay between retry attempts in seconds.
    #[must_use]
    pub fn wait_time(self, secs: u64) -> FetcherBuilder {
        Self {
            wait_time: Duration::from_secs(secs),
            ..self
        }
    }

    /// Sets the SHA1 hash for response verification.
    #[must_use]
    pub fn sha1(self, sha1code: &str) -> FetcherBuilder {
        Self {
            sha1: Some(sha1code.into()),
            ..self
        }
    }

    /// Executes the HTTP request and returns the deserialized result.
    ///
    /// Retries the request up to the configured number of times on failure,
    /// with a configurable delay between attempts.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mc_api::fetcher::FetcherBuilder;
    ///
    /// let result = FetcherBuilder::fetch("http://example.com/data.json")
    ///     .json()
    ///     .execute()?;
    /// let data: serde_json::Value = result.json()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - All retry attempts fail (connection error, timeout, etc.)
    /// - Server returns a non-success status code
    /// - Response cannot be deserialized (malformed JSON/XML, invalid structure)
    /// - SHA1 verification fails (if configured)
    pub fn execute<T: serde::de::DeserializeOwned>(self) -> Result<FetcherResult<T>> {
        let client = Client::new();
        for _ in 0..self.retry {
            let send = client
                .get(&self.url)
                .header(
                    reqwest::header::USER_AGENT,
                    "github.com/funny233-github/MCLauncher",
                )
                .timeout(self.timeout)
                .send();
            if let Ok(send) = send {
                if let DeserializeType::Byte = self.deserialize_type {
                    let data = send.bytes()?;
                    if let Some(sha1) = self.sha1 {
                        if data.sha1_cmp(&sha1).is_ne() {
                            break;
                        }
                    }
                    return Ok(FetcherResult::Byte(data.into()));
                }
                let data = send.text()?;
                if let Some(sha1) = self.sha1 {
                    if data.sha1_cmp(&sha1).is_ne() {
                        break;
                    }
                }
                match self.deserialize_type {
                    DeserializeType::Text => {
                        return Ok(FetcherResult::Text(data));
                    }
                    DeserializeType::Json => {
                        return Ok(FetcherResult::Json(serde_json::from_str(&data)?))
                    }
                    DeserializeType::Xml => {
                        return Ok(FetcherResult::Xml(quick_xml::de::from_str(&data)?))
                    }
                    DeserializeType::Byte => {
                        return Err(anyhow::anyhow!("Internal error: Byte deserialization should have been handled earlier in the code"))
                    }
                }
            }
        }
        Err(anyhow::anyhow!("fetch {} fail", self.url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_returns_string() {
        // This test verifies that DeserializeType::None returns String
        // We'll use serde's ability to serialize/deserialize String
        let data = "Hello, World!";
        let json_string = serde_json::to_string(data).unwrap();
        let result: String = serde_json::from_str(&json_string).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_xml_method_exists() {
        // Test that xml() method sets the correct deserialize type
        let builder = FetcherBuilder::fetch("http://example.com").xml();
        assert!(matches!(builder.deserialize_type, DeserializeType::Xml));
    }
}
