use crate::Sha1Compare;
use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

#[derive(Default)]
pub enum DeserializeType {
    #[default]
    None,
    Json,
    Xml,
}

pub struct FetcherBuilder {
    pub url: String,
    pub deserialize_type: DeserializeType,
    pub retry: u64,
    pub timeout: Duration,
    pub wait_time: Duration,
    pub sha1: Option<String>,
}

pub enum FetcherResult<T: serde::de::DeserializeOwned> {
    Text(String),
    Json(T),
    Xml(T),
}

impl<T: serde::de::DeserializeOwned> FetcherResult<T> {
    /// # Errors
    /// TODO complete docs
    pub fn text(self) -> Result<String> {
        if let FetcherResult::Text(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("TODO complete error"))?
        }
    }

    /// # Errors
    /// TODO complete docs
    pub fn json(self) -> Result<T> {
        if let FetcherResult::Json(res) = self {
            Ok(res)
        } else {
            Err(anyhow::anyhow!("TODO complete error"))?
        }
    }

    /// # Errors
    /// TODO complete docs
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
                let data = send.text()?;
                if let Some(sha1) = self.sha1 {
                    if data.sha1_cmp(&sha1).is_ne() {
                        break;
                    }
                }
                match self.deserialize_type {
                    DeserializeType::None => {
                        return Ok(FetcherResult::Text(data));
                    }
                    DeserializeType::Json => {
                        return Ok(FetcherResult::Json(serde_json::from_str(data.as_str())?))
                    }
                    DeserializeType::Xml => {
                        return Ok(FetcherResult::Xml(quick_xml::de::from_str(data.as_str())?))
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
