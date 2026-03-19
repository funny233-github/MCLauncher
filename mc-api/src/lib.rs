//! Minecraft API library for version management and metadata fetching.
//!
//! This library provides interfaces for interacting with the official Mojang API and
//! mod loader APIs (Fabric, `NeoForge`). It supports version management, metadata fetching,
//! mirror server integration, and SHA1 verification.
//!
//! # Example
//! ```no_run
//! use mc_api::official::{VersionManifest, Version};
//!
//! let manifest_mirror = "https://bmclapi2.bangbang93.com/";
//! let manifest = VersionManifest::fetch(manifest_mirror)?;
//! let releases = manifest.list(&mc_api::official::VersionType::Release);
//! let version = Version::fetch(&manifest, "1.20.4", manifest_mirror)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Example (Fabric)
//! ```no_run
//! use mc_api::fabric::{Versions, Profile};
//!
//! let mirror = "https://bmclapi2.bangbang93.com/fabric-meta/";
//! let versions = Versions::fetch(mirror)?;
//! let profile = Profile::fetch(mirror, "1.20.6", "0.15.10")?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use regex::Regex;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;

/// Compares SHA1 hashes of byte data with expected hash strings.
///
/// Computes the SHA1 hash of the data and performs lexicographic string comparison
/// with the expected hash. Returns `Ordering::Equal` when hashes match exactly,
/// `Ordering::Less` when computed hash is lexicographically less, and
/// `Ordering::Greater` when computed hash is lexicographically greater.
/// The hash is computed on each call; consider caching for repeated comparisons.
///
/// # Example
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
pub trait Sha1Compare {
    /// Compares the SHA1 hash of self with the expected hash.
    ///
    /// Computes the SHA1 hash and performs lexicographic comparison.
    fn sha1_cmp(&self, sha1code: &str) -> Ordering;
}

/// Replaces domain names in URLs with new domains.
///
/// Uses a regular expression pattern `https://\S+?/` to identify and replace
/// the protocol and domain portion of URLs while preserving the path and
/// query parameters. This is useful for switching between official servers
/// and mirror servers.
///
/// # Example
/// ```
/// use mc_api::DomainReplacer;
///
/// let original = "https://launchermeta.mojang.com/mc/game/version_manifest.json".to_string();
/// let mirror = "https://bmclapi2.bangbang93.com/";
///
/// let replaced = original.replace_domain(mirror);
/// assert_eq!(replaced, "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json");
/// ```
pub trait DomainReplacer<T> {
    /// Replaces the domain in a URL with a new domain.
    fn replace_domain(&self, domain: &str) -> T;
}

/// Implementation for URL strings, using regex pattern `https://\S+?/` to match
/// and replace the protocol and domain while preserving the path and query.
///
/// The regex is compiled on each call; consider caching for performance-critical code.
impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

/// Implementation for types that can be referenced as byte slices (`Vec<u8>`, `&[u8]`, `String`, `&str`).
///
/// Uses SHA-1 algorithm to compute hash and performs lexicographic comparison.
/// SHA-1 is cryptographically broken and should not be used for security-critical
/// purposes; this implementation is intended for file integrity verification.
///
/// # Example
/// ```
/// use mc_api::Sha1Compare;
///
/// let data = vec![0u8, 1, 2, 3];
/// let _ = data.sha1_cmp("some_hash");
/// ```
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
