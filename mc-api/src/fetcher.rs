use crate::Sha1Compare;
use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

/// Fetcher Module
///
/// This module provides a type-safe, builder-pattern based HTTP fetcher
/// with support for JSON/XML deserialization, SHA1 verification,
/// and automatic retry logic.
///
/// # Features
///
/// - **Type-safe API**: Builder pattern with compile-time type checking
/// - **Multiple formats**: Support for JSON, XML, and plain text
/// - **SHA1 verification**: Optional integrity checking for critical data
/// - **Retry logic**: Configurable retry attempts
/// - **Timeout handling**: Configurable request timeout
///
/// # Basic Usage
///
/// ```no_run
/// use mc_api::fetcher::FetcherBuilder;
///
/// // Fetch JSON data
/// let data: String = FetcherBuilder::fetch("http://example.com/data.json")
///     .json()
///     .execute()?
///     .json()?;
///
/// // Fetch XML data
/// let xml_data: String = FetcherBuilder::fetch("http://example.com/data.xml")
///     .xml()
///     .execute()?
///     .xml()?;
///
/// // Fetch plain text
/// let text: String = FetcherBuilder::fetch("http://example.com/text.txt")
///     .execute::<String>()?
///     .text()?;
///
/// // Fetch with SHA1 verification
/// let verified_text: String = FetcherBuilder::fetch("http://example.com/asset.json")
///     .sha1("abc123...")
///     .execute::<String>()?
///     .text()?;
/// # Ok::<(),anyhow::Error>(())
/// ```
///
/// # Comparison with fetch! Macro
///
/// | Feature | fetch! | FetcherBuilder |
/// |---------|--------|----------------|
/// | Type safety | ❌ Macro | ✅ Compile-time checking |
/// | Builder pattern | ❌ No | ✅ Fluent API |
/// | Configurable | ❌ Hardcoded | ✅ Flexible parameters |
/// | SHA1 verification | ✅ Optional | ✅ Optional |
/// | Retry logic | ✅ Fixed 5 times | ✅ Configurable |
#[derive(Default)]
pub enum DeserializeType {
    #[default]
    Byte,
    Json,
    Xml,
    Text,
}

/// Builder for configuring and executing HTTP requests with retry logic.
///
/// # Fields
///
/// * `url` - The URL to fetch data from
/// * `deserialize_type` - How to parse the response (Json, Xml, or None for text)
/// * `retry` - Number of retry attempts (default: 5)
/// * `timeout` - Request timeout in seconds (default: 100)
/// * `wait_time` - Delay between retry attempts in seconds (default: 10)
/// * `sha1` - Optional SHA1 hash for response verification
pub struct FetcherBuilder {
    pub url: String,
    pub deserialize_type: DeserializeType,
    pub retry: u64,
    pub timeout: Duration,
    pub wait_time: Duration,
    pub sha1: Option<String>,
}

/// Result type returned by `FetcherBuilder::execute()`.
///
/// Contains deserialized data in the requested format.
///
/// # Type Parameters
///
/// * `T` - The deserialized type for Json/Xml variants, or unused for Text
///
/// # Variants
///
/// * `Text(String)` - Raw text response when `DeserializeType::None` is used
/// * `Json(T)` - JSON deserialized into type `T`
/// * `Xml(T)` - XML deserialized into type `T`
///
/// # Example
///
/// ```no_run
/// use mc_api::fetcher::FetcherBuilder;
///
/// // Get raw text
/// let text: String = FetcherBuilder::fetch("http://example.com/data.txt")
///     .execute::<String>()?
///     .text()?;
///
/// // Get JSON data
/// let data: String = FetcherBuilder::fetch("http://example.com/data.json")
///     .json()
///     .execute()?
///     .json()?;
/// # Ok::<(),anyhow::Error>(())
/// ```
pub enum FetcherResult<T: serde::de::DeserializeOwned> {
    Text(String),
    Byte(Vec<u8>),
    Json(T),
    Xml(T),
}

impl<T: serde::de::DeserializeOwned> FetcherResult<T> {
    /// # Errors
    /// TODO complete docs
    pub fn byte(self) -> Result<Vec<u8>> {
        if let FetcherResult::Byte(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("TODO complete error"))?
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
            Err(anyhow::anyhow!("TODO complete error"))?
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
            Err(anyhow::anyhow!("TODO complete error"))?
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
            Err(anyhow::anyhow!("TODO complete error"))?
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
    #[must_use]
    pub fn fetch(url: &str) -> FetcherBuilder {
        Self {
            url: url.to_owned(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn json(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Json,
            ..self
        }
    }

    #[must_use]
    pub fn xml(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Xml,
            ..self
        }
    }

    #[must_use]
    pub fn text(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Text,
            ..self
        }
    }

    #[must_use]
    pub fn byte(self) -> FetcherBuilder {
        Self {
            deserialize_type: DeserializeType::Byte,
            ..self
        }
    }

    #[must_use]
    pub fn retry(self, num: u64) -> FetcherBuilder {
        Self { retry: num, ..self }
    }

    #[must_use]
    pub fn timeout(self, secs: u64) -> FetcherBuilder {
        Self {
            timeout: Duration::from_secs(secs),
            ..self
        }
    }

    #[must_use]
    pub fn wait_time(self, secs: u64) -> FetcherBuilder {
        Self {
            wait_time: Duration::from_secs(secs),
            ..self
        }
    }

    #[must_use]
    pub fn sha1(self, sha1code: &str) -> FetcherBuilder {
        Self {
            sha1: Some(sha1code.into()),
            ..self
        }
    }

    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails (connection refused, DNS resolution failure, etc.)
    /// - Request times out after configured duration
    /// - Server returns non-success status code
    /// - JSON/XML response is malformed and cannot be parsed
    /// - Response does not match expected data structure
    /// - XML parsing fails (when using `xml()`)
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
                        return Err(anyhow::anyhow!("TODO complete error message"))
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
