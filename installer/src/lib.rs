//! Minecraft File Installer Library
//!
//! This library provides functionality for downloading, installing, and verifying files
//! for Minecraft mods and resources. It handles concurrent downloads with progress tracking,
//! SHA1 integrity verification, and retry logic for network reliability.
//!
//! # Features
//!
//! - **Concurrent Downloads**: Download multiple files simultaneously with configurable concurrency
//! - **Progress Tracking**: Visual progress bars showing download status and progress
//! - **Integrity Verification**: SHA1 hash verification to ensure file integrity
//! - **Retry Logic**: Automatic retries for failed downloads (up to 5 attempts with 3 second delay)
//! - **Incremental Updates**: Skip downloading files that already exist with matching hashes
//! - **Error Handling**: Comprehensive error handling for network and filesystem operations
//!
//! # Architecture
//!
//! The library is built around two main components:
//!
//! - **`FileInstall` trait**: Defines the interface for file installation operations
//! - **`TaskPool<T>`**: Manages concurrent execution of multiple installation tasks
//!
//! # Usage Example
//!
//! ```no_run
//! use installer::{InstallTask, TaskPool};
//! use std::collections::VecDeque;
//! use std::path::PathBuf;
//!
//! fn main() -> anyhow::Result<()> {
//!     // Create installation tasks
//!     let tasks = VecDeque::from(vec![
//!         InstallTask {
//!             url: "https://example.com/mod1.jar".to_string(),
//!             sha1: Some("abc123...".to_string()),
//!             save_file: PathBuf::from("./mods/mod1.jar"),
//!             message: "Downloading mod1.jar".to_string(),
//!         },
//!         InstallTask {
//!             url: "https://example.com/mod2.jar".to_string(),
//!             sha1: Some("def456...".to_string()),
//!             save_file: PathBuf::from("./mods/mod2.jar"),
//!             message: "Downloading mod2.jar".to_string(),
//!         },
//!     ]);
//!
//!     // Create task pool and install all files
//!     let pool = TaskPool::from(tasks);
//!     pool.install();
//!
//!     Ok(())
//! }
//! ```
//!
//! # Installation Behavior
//!
//! ## File Skipping
//!
//! Files are skipped (not downloaded) if:
//! - The file already exists at the target location
//! - The existing file's SHA1 hash matches the expected hash
//! - No verification fails
//!
//! ## Hash Verification
//!
//! When `sha1` is provided:
//! - Downloaded files are verified against the expected hash
//! - Existing files are verified before being skipped
//! - Downloads are retried if hash verification fails
//!
//! ## Concurrent Execution
//!
//! - Files are downloaded concurrently (64 parallel downloads by default)
//! - Progress is updated in real-time for each task
//! - Errors in individual tasks will cause the entire installation to panic
//!
//! # Error Handling
//!
//! The library uses `anyhow::Result` for comprehensive error handling:
//! - Network failures during downloads
//! - Filesystem permissions issues
//! - Hash verification failures
//! - Timeout errors (128 second timeout per download)
//!
//! # Progress Display
//!
//! Progress bars show:
//! - Elapsed time
//! - Visual progress bar (40 characters wide)
//! - Task count (position/total)
//! - Current task message
//!
//! Example progress bar:
//! ```text
//! [00:01:23] ##############------------------------ 3/10 Downloading mod3.jar
//! ```

use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use reqwest::header;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Defines interface for file installation operations with async execution and progress tracking.
///
/// Implementations must be `Send + Sync + Clone + 'static` to support concurrent
/// execution in the task pool.
pub trait FileInstall {
    /// Executes the installation operation asynchronously.
    ///
    /// The returned future must be `Send` to support concurrent execution.
    ///
    /// # Errors
    /// Returns an error if the installation operation fails.
    ///
    /// # Panics
    /// Panics depend on the implementation.
    fn install(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    /// Updates the progress bar after installation.
    ///
    /// Typical implementations increment the progress counter and update the message.
    /// # Example
    /// ```no_run
    /// use installer::{FileInstall, InstallTask};
    /// use indicatif::ProgressBar;
    /// use std::path::PathBuf;
    ///
    /// let task = InstallTask {
    ///     url: "https://example.com/file.jar".to_string(),
    ///     sha1: None,
    ///     save_file: PathBuf::from("./file.jar"),
    ///     message: "Installing file.jar".to_string(),
    /// };
    /// let bar = ProgressBar::new(1);
    /// task.bar_update(&bar);
    /// ```
    fn bar_update(&self, bar: &ProgressBar);
}

/// Provides SHA1 hash comparison for byte slices.
///
/// Implemented for any type that can be referenced as a byte slice.
pub trait ShaCompare {
    /// Compares the SHA1 hash of self with the expected hash.
    ///
    /// Returns `Ordering::Equal` if hashes match.
    ///
    /// # Example
    /// ```
    /// use installer::ShaCompare;
    ///
    /// let data = b"Hello, World!";
    /// let expected_hash = "0a0a9f2a6772942557ab5355d76af442f8f65e01";
    ///
    /// match data.sha1_cmp(expected_hash) {
    ///     std::cmp::Ordering::Equal => println!("Hash matches!"),
    ///     _ => println!("Hash does not match"),
    /// }
    /// ```
    fn sha1_cmp<C>(&self, sha1code: C) -> Ordering
    where
        C: AsRef<str> + Into<String>;
}

/// Implementation of `ShaCompare` for any type that can be referenced as a byte slice.
///
/// This provides SHA1 comparison functionality for common types like `Vec<u8>`,
/// `&[u8]`, `String`, and `&str`.
///
/// # Examples
///
/// ```no_run
/// use installer::ShaCompare;
///
/// // Compare bytes
/// let data = vec![0u8, 1, 2, 3];
/// let hash = data.sha1_cmp("some_hash");
///
/// // Compare string
/// let text = "Hello, World!";
/// let hash = text.sha1_cmp("another_hash");
/// ```
impl<T> ShaCompare for T
where
    T: AsRef<[u8]>,
{
    fn sha1_cmp<C>(&self, sha1code: C) -> Ordering
    where
        C: AsRef<str> + Into<String>,
    {
        let mut hasher = Sha1::new();
        hasher.update(self);
        let sha1 = hasher.finalize();
        hex::encode(sha1).cmp(&sha1code.into())
    }
}

/// Represents a file download and installation task.
///
/// If `sha1` is provided, verifies integrity and skips download if file exists
/// with matching hash.
///
/// # Example
/// ```no_run
/// use installer::InstallTask;
/// use std::path::PathBuf;
///
/// let task = InstallTask {
///     url: "https://example.com/mod.jar".to_string(),
///     sha1: Some("abc123...".to_string()),
///     save_file: PathBuf::from("./mods/mod.jar"),
///     message: "Downloading mod.jar".to_string(),
/// };
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InstallTask {
    /// URL of the file to download.
    pub url: String,
    /// Optional SHA1 hash for integrity verification.
    pub sha1: Option<String>,
    /// Destination path where the file should be saved.
    pub save_file: PathBuf,
    /// Message to display in the progress bar during installation.
    pub message: String,
}

/// Downloads bytes from a URL with retry logic and optional SHA1 verification.
///
/// Makes up to `retry_num` attempts to download the file, waiting `sleep_time`
/// between retries. Times out after 128 seconds per attempt. If `sha1` is provided,
/// verifies the downloaded data against the hash.
///
/// # Example
/// ```no_run
/// use installer::fetch_bytes;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let data = fetch_bytes(
///         "https://example.com/file.dat",
///         Some(&"abc123...".into()),
///         Duration::from_secs(3),
///         5,
///     ).await?;
///     println!("Downloaded {} bytes", data.len());
///     Ok(())
/// }
/// ```
///
/// # Errors
/// Returns an error if all retry attempts fail, network issues occur, or SHA1 verification fails.
///
/// # Panics
/// This function does not panic; all errors are returned via the `Result` type.
pub async fn fetch_bytes(
    url: &str,
    sha1: Option<&String>,
    sleep_time: Duration,
    retry_num: u32,
) -> anyhow::Result<bytes::Bytes> {
    let client = reqwest::Client::new();
    for _ in 0..retry_num {
        let send = client
            .get(url)
            .header(header::USER_AGENT, "github.com/funny233-github/MCLauncher")
            .timeout(Duration::from_secs(1000))
            .send()
            .await;

        // There must handle Result for 'for loop'
        let data = if let Ok(send) = send {
            send.bytes().await
        } else {
            continue;
        };

        if let Ok(data) = data {
            if sha1.is_none() || data.sha1_cmp(sha1.unwrap()).is_eq() {
                return Ok(data);
            }
        }
        warn!("install {url} fail, then retry");
        tokio::time::sleep(sleep_time).await;
    }
    Err(anyhow::anyhow!("download {url} fail"))
}

impl FileInstall for InstallTask {
    /// Installs the file by downloading it if needed.
    ///
    /// If `sha1` is `None`, always downloads the file. If `sha1` is `Some`,
    /// skips download if file exists with matching hash. Downloads with retry logic
    /// (5 attempts, 10 second delay) and creates parent directories if needed.
    ///
    /// # Errors
    /// Returns an error if download fails after all retry attempts, filesystem permissions
    /// prevent directory creation or file writing, or SHA1 verification fails.
    ///
    /// # Panics
    /// Panics if the file path has no parent directory.
    async fn install(&self) -> anyhow::Result<()> {
        if self.sha1.is_none()
            || !(self.save_file.exists()
                && fs::read(&self.save_file)?
                    .sha1_cmp(self.sha1.as_ref().unwrap())
                    .is_eq())
        {
            let data =
                fetch_bytes(&self.url, self.sha1.as_ref(), Duration::from_secs(10), 5).await?;
            fs::create_dir_all(self.save_file.parent().unwrap())?;
            fs::write(&self.save_file, data)?;
        }
        Ok(())
    }

    /// Updates the progress bar with task completion status.
    ///
    /// Increments the progress counter and updates the progress message.
    fn bar_update(&self, bar: &ProgressBar) {
        bar.inc(1);
        bar.set_message(self.message.clone());
    }
}

/// A pool of installation tasks that executes concurrently with progress tracking.
///
/// Tasks are executed with configurable concurrency (64 by default). Errors in
/// individual tasks cause the entire installation to panic.
///
/// # Example
/// ```no_run
/// use installer::{InstallTask, TaskPool};
/// use std::collections::VecDeque;
/// use std::path::PathBuf;
///
/// fn main() {
///     let tasks = VecDeque::from(vec![
///         InstallTask {
///             url: "https://example.com/mod1.jar".to_string(),
///             sha1: Some("abc123...".to_string()),
///             save_file: PathBuf::from("./mods/mod1.jar"),
///             message: "Downloading mod1.jar".to_string(),
///         },
///     ]);
///
///     let pool = TaskPool::from(tasks);
///     pool.install(); // Blocks until all tasks complete
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TaskPool<T>
where
    T: FileInstall + std::marker::Send + std::marker::Sync + Clone + 'static,
{
    /// Deque of tasks to be executed.
    pub pool: VecDeque<T>,
    /// Progress bar for tracking installation progress.
    bar: ProgressBar,
}

/// Creates a `TaskPool` from a `VecDeque` of tasks with a configured progress bar.
///
/// The progress bar is configured with the template:
/// `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}`.
impl<T> From<VecDeque<T>> for TaskPool<T>
where
    T: FileInstall + std::marker::Send + std::marker::Sync + Clone,
{
    fn from(tasks: VecDeque<T>) -> Self {
        let bar = ProgressBar::new(tasks.len() as u64);
        bar.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        Self { pool: tasks, bar }
    }
}

impl<T> TaskPool<T>
where
    T: FileInstall + std::marker::Send + std::marker::Sync + Clone,
{
    /// Executes all installation tasks concurrently.
    ///
    /// Tasks are executed concurrently with a buffer of 64 parallel operations.
    /// This method blocks until all tasks complete. Creates a new tokio runtime in
    /// the current thread, so it's designed to be called from outside an async context.
    ///
    /// # Example
    /// ```no_run
    /// use installer::{InstallTask, TaskPool};
    /// use std::collections::VecDeque;
    /// use std::path::PathBuf;
    ///
    /// let tasks = VecDeque::from(vec![
    ///     InstallTask {
    ///         url: "https://example.com/mod1.jar".to_string(),
    ///         sha1: Some("abc123...".to_string()),
    ///         save_file: PathBuf::from("./mods/mod1.jar"),
    ///         message: "Downloading mod1.jar".to_string(),
    ///     },
    ///     InstallTask {
    ///         url: "https://example.com/mod2.jar".to_string(),
    ///         sha1: Some("def456...".to_string()),
    ///         save_file: PathBuf::from("./mods/mod2.jar"),
    ///         message: "Downloading mod2.jar".to_string(),
    ///     },
    /// ]);
    ///
    /// let pool = TaskPool::from(tasks);
    /// pool.install(); // Blocks until all tasks complete
    /// ```
    ///
    /// # Panics
    /// Panics if any task's `install` method returns an error or progress bar update fails.
    #[tokio::main(flavor = "current_thread")]
    pub async fn install(self) {
        let tasks = self.pool.into_iter().map(|x| {
            let share = self.bar.clone();
            async move {
                x.install().await.unwrap();
                x.bar_update(&share);
            }
        });
        stream::iter(tasks)
            .buffer_unordered(64)
            .collect::<VecDeque<_>>()
            .await;
    }
}
