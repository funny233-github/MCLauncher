//! Minecraft API Library
//!
//! This library provides a comprehensive interface for interacting with various Minecraft
//! APIs, including the official Mojang API and the Fabric mod loader API. It supports
//! version management, metadata fetching, and integration with mirror servers.
//!
//! # Features
//!
//! - **Official Minecraft API**: Fetch version manifests, version details, and asset indices
//! - **Fabric Loader API**: Retrieve Fabric versions, loaders, yarn mappings, and profiles
//! - **Mirror Support**: Built-in support for mirror servers with domain replacement
//! - **SHA1 Verification**: Integrity checking for downloaded files
//! - **Retry Logic**: Automatic retries with exponential backoff for network failures
//! - **Version Merging**: Combine official versions with mod loader profiles
//!
//! # Architecture
//!
//! The library is organized into two main modules:
//!
//! - **`official`**: Minecraft official API interactions
//! - **`fabric`**: Fabric mod loader API interactions
//!
//! # Usage Example
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version};
//!
//! // Fetch version manifest
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//!
//! // Get version list
//! let releases = manifest.list(&mc_api::official::VersionType::Release);
//! println!("Latest release: {}", releases.first().unwrap());
//!
//! // Fetch specific version
//! let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//! println!("Main class: {}", version.main_class);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Fabric Integration
//!
//! ```no_run
//! use mc_api::fabric::{Versions, Profile};
//!
//! // Fetch all Fabric metadata
//! let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
//! let versions = Versions::fetch(mirror)?;
//!
//! // Fetch Fabric profile
//! let profile = Profile::fetch(mirror, "1.20.6", "0.15.10")?;
//! println!("Main class: {}", profile.main_class);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Mirror Support
//!
//! The library supports using mirror servers to improve download speed and reliability:
//!
//! - **Official mirrors**: Use any compatible mirror for official API calls
//! - **Fabric mirrors**: Dedicated Fabric metadata mirrors
//! - **Domain replacement**: Automatic URL transformation for mirror servers
//!
//! Common mirrors:
//! - `https://bmclapi2.bangbang93.com/` - Bangbang93 API (China)
//! - `https://launchermeta.mojang.com/` - Official Mojang API
//!
//! # Version Merging
//!
//! Combine official Minecraft versions with mod loader profiles:
//!
//! ```no_run
//! use mc_api::official::{VersionManifest, Version};
//! use mc_api::fabric::Profile;
//!
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let fabric_mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
//!
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//! let mut version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//! let profile = Profile::fetch(fabric_mirror, "1.20.4", "0.15.10")?;
//!
//! // Merge Fabric profile into official version
//! version.merge(&profile);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Error Handling
//!
//! The library uses `anyhow::Result` for comprehensive error handling:
//! - Network failures during API calls
//! - Invalid responses from servers
//! - SHA1 verification failures
//! - Version not found errors
//!
//! # Retry Logic
//!
//! Network requests include automatic retry logic:
//! - Up to 5 retry attempts
//! - 10 second delay between retries
//! - 100 second timeout per attempt
//! - Automatic SHA1 verification when hashes are provided
//!
//! # Platform Support
//!
//! The library supports:
//! - **Windows**: Native library filtering
//! - **Linux**: Native library filtering
//! - **macOS**: Native library filtering
//!
//! Platform-specific libraries are automatically filtered based on the target OS.

use regex::Regex;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;

/// Trait for SHA1 hash comparison.
///
/// This trait provides a convenient method for computing SHA1 hashes of data
/// and comparing them with expected hash values.
///
/// # Type Parameters
///
/// The trait is implemented for any type that can be referenced as a byte slice.
///
/// # Comparison
///
/// Computes the SHA1 hash of the data and compares it with the expected hash.
/// The comparison returns an `Ordering` value indicating the relationship between
/// the computed hash and the expected hash. The specific semantics of the
/// comparison (e.g., lexicographic ordering) are determined by the implementation.
///
/// # Example
///
/// ```
/// use mc_api::Sha1Compare;
///
/// let data = b"Hello, World!";
/// let expected_hash = "0a0a9f2a6772942557ab5355d76af442f8f65e01";
///
/// match data.sha1_cmp(expected_hash) {
///     std::cmp::Ordering::Equal => println!("Hash matches!"),
///     _ => println!("Hash does not match"),
/// }
/// ```
///
/// # Performance
///
/// This method computes the SHA1 hash on each call. For repeated comparisons,
/// consider caching the computed hash.
pub trait Sha1Compare {
    /// Compare the SHA1 hash of self with the expected hash.
    ///
    /// Computes the SHA1 hash of the data and compares it with the expected hash.
    ///
    /// # Parameters
    ///
    /// * `sha1code` - The expected SHA1 hash string
    ///
    /// # Returns
    ///
    /// Returns an `Ordering` result indicating the comparison outcome.
    /// The specific semantics of the comparison are determined by the implementation.
    fn sha1_cmp(&self, sha1code: &str) -> Ordering;
}

/// Trait for replacing domain names in URLs.
///
/// This trait provides functionality to replace the domain portion of URLs,
/// which is useful for switching between official servers and mirror servers.
///
/// # Type Parameters
///
/// * `T` - The return type after domain replacement
///
/// # Domain Matching
///
/// The trait uses a regular expression to identify and replace the domain:
/// - Pattern: `https://\S+?/`
/// - Matches the protocol and domain portion of the URL
/// - Preserves the path and query parameters
///
/// # Example
///
/// ```
/// use mc_api::DomainReplacer;
///
/// let original = "https://launchermeta.mojang.com/mc/game/version_manifest.json".to_string();
/// let mirror = "https://bmclapi2.bangbang93.com/";
///
/// let replaced = original.replace_domain(mirror);
/// assert_eq!(replaced, "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json");
/// ```
///
/// # Use Cases
///
/// - Switching between official and mirror servers
/// - Implementing custom mirror support
/// - URL transformation for localization
pub trait DomainReplacer<T> {
    /// Replace the domain in a URL with a new domain.
    ///
    /// This method extracts the path from the original URL and combines it
    /// with the new domain to create a new URL.
    ///
    /// # Parameters
    ///
    /// * `domain` - The new domain to use (e.g., `https://mirror.example.com/`)
    ///
    /// # Returns
    ///
    /// Returns a new URL with the domain replaced.
    fn replace_domain(&self, domain: &str) -> T;
}

/// Implementation of `DomainReplacer` for `String`.
///
/// This provides domain replacement functionality for URL strings, which is
/// commonly used for switching between official Minecraft servers and mirror servers.
///
/// # Regular Expression Pattern
///
/// The implementation uses the pattern `https://\S+?/` to identify the domain portion:
/// - Matches `https://` protocol
/// - Matches any non-whitespace characters until the first `/`
/// - Preserves the rest of the URL (path, query, fragment)
///
/// # Example
///
/// ```
/// use mc_api::DomainReplacer;
///
/// let official_url = "https://launchermeta.mojang.com/mc/game/version_manifest.json".to_string();
/// let mirror_url = official_url.replace_domain("https://bmclapi2.bangbang93.com/");
///
/// assert_eq!(mirror_url, "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json");
/// ```
///
/// # Performance
///
/// The regex is compiled on each call. For performance-critical code, consider
/// caching the compiled regex or using a different approach for repeated replacements.
impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

/// Implementation of `Sha1Compare` for any type that can be referenced as a byte slice.
///
/// This provides SHA1 comparison functionality for common types like `Vec<u8>`,
/// `&[u8]`, `String`, and `&str`.
///
/// # Examples
///
/// ```
/// use mc_api::Sha1Compare;
///
/// // Compare bytes
/// let data = vec![0u8, 1, 2, 3];
/// let hash = data.sha1_cmp("some_hash");
///
/// // Compare string
/// let text = "Hello, World!";
/// let hash = text.sha1_cmp("another_hash");
/// ```
///
/// # Algorithm
///
/// The implementation uses the SHA-1 cryptographic hash algorithm:
/// 1. Creates a new SHA1 hasher
/// 2. Updates the hasher with the data
/// 3. Finalizes the hash
/// 4. Encodes the result as hexadecimal
/// 5. Performs lexicographic string comparison with the expected hash
///
/// # Comparison Semantics
///
/// This implementation performs a lexicographic comparison between the hexadecimal
/// representation of the computed SHA1 hash and the expected hash string:
/// - `Ordering::Equal` when hashes match exactly
/// - `Ordering::Less` when the computed hash is lexicographically less
/// - `Ordering::Greater` when the computed hash is lexicographically greater
///
/// # Cryptographic Considerations
///
/// SHA-1 is considered cryptographically broken and should not be used for
/// security-critical purposes. This implementation is intended for file
/// integrity verification, which is its primary use in this library.
impl<T> Sha1Compare for T
where
    T: AsRef<[u8]>,
{
    fn sha1_cmp(&self, sha1code: &str) -> Ordering {
        let mut hasher = Sha1::new();
        hasher.update(self);
        let sha1 = hasher.finalize();
        hex::encode(sha1).cmp(&sha1code.into())
    }
}

pub mod fabric;
pub mod fetcher;
pub mod neoforge;
pub mod official;
