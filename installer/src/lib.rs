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

/// Trait for file installation operations.
///
/// This trait defines the interface for implementing custom file installation tasks.
/// Implementations must support both asynchronous installation and progress bar updates.
///
/// # Required Methods
///
/// * [`install`](Self::install) - Execute the installation operation asynchronously
/// * [`bar_update`](Self::bar_update) - Update the progress bar after installation
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync + Clone + 'static` to support concurrent
/// execution in the task pool.
///
/// # Example Implementation
///
/// ```
/// use installer::FileInstall;
/// use indicatif::ProgressBar;
/// use std::path::PathBuf;
///
/// struct CustomTask {
///     source: PathBuf,
///     destination: PathBuf,
/// }
///
/// impl FileInstall for CustomTask {
///     async fn install(&self) -> anyhow::Result<()> {
///         // Custom installation logic here
///         tokio::fs::copy(&self.source, &self.destination).await?;
///         Ok(())
///     }
///
///     fn bar_update(&self, bar: &ProgressBar) {
///         bar.inc(1);
///         bar.set_message(format!("Installed: {:?}", self.source));
///     }
/// }
/// ```
pub trait FileInstall {
    /// Execute the installation operation asynchronously.
    ///
    /// This method performs the actual installation task, such as downloading files,
    /// copying files, or other custom installation operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the installation operation fails.
    /// The specific error conditions depend on the implementation.
    ///
    /// # Async Context
    ///
    /// This method is called asynchronously and can use async operations.
    /// The returned future must be `Send` to support concurrent execution.
    fn install(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    /// Update the progress bar after installation.
    ///
    /// This method is called after the `install` method completes successfully.
    /// It should update the progress bar to reflect the completion of the task.
    ///
    /// # Parameters
    ///
    /// * `bar` - Reference to the shared progress bar
    ///
    /// # Usage
    ///
    /// Typically, implementations will:
    /// - Increment the progress counter (`bar.inc(1)`)
    /// - Update the progress message (`bar.set_message(...)`)
    fn bar_update(&self, bar: &ProgressBar);
}

/// Trait for SHA1 hash comparison.
///
/// This trait provides a convenient method for computing SHA1 hashes of data
/// and comparing them with expected hash values. It's implemented for any type
/// that can be referenced as a byte slice.
///
/// # Type Parameters
///
/// * `C` - The type of the SHA1 hash string, must implement `AsRef<str> + Into<String>`
///
/// # Example Usage
///
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
pub trait ShaCompare {
    /// Compare the SHA1 hash of self with the expected hash.
    ///
    /// Computes the SHA1 hash of the data and compares it with the expected hash.
    /// Returns `Ordering::Equal` if hashes match, `Ordering::Less` if computed hash
    /// is lexicographically less, or `Ordering::Greater` otherwise.
    ///
    /// # Parameters
    ///
    /// * `sha1code` - The expected SHA1 hash string
    ///
    /// # Returns
    ///
    /// Returns an `Ordering` result indicating the comparison outcome.
    ///
    /// # Performance
    ///
    /// This method computes the SHA1 hash on each call. For repeated comparisons,
    /// consider caching the computed hash.
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
/// This structure contains all the information needed to download and install a single file,
/// including the URL, expected SHA1 hash, target path, and progress message.
///
/// # Fields
///
/// * `url` - The URL of the file to download
/// * `sha1` - Optional SHA1 hash for integrity verification
/// * `save_file` - The destination path where the file should be saved
/// * `message` - Message to display in the progress bar during installation
///
/// # Behavior
///
/// - If `sha1` is `Some`, the downloaded file will be verified against the hash
/// - If the file already exists and the hash matches, the download is skipped
/// - If `sha1` is `None`, the file is always downloaded regardless of existing content
///
/// # Example
///
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
    pub url: String,
    pub sha1: Option<String>,
    pub save_file: PathBuf,
    pub message: String,
}

/// Downloads bytes from a URL with retry logic and optional SHA1 verification.
///
/// This function downloads data from the specified URL with automatic retries,
/// timeout handling, and optional hash verification.
///
/// # Parameters
///
/// * `url` - The URL to download from
/// * `sha1` - Optional SHA1 hash for verification
/// * `sleep_time` - Time to sleep between retry attempts
/// * `retry_num` - Maximum number of retry attempts
///
/// # Behavior
///
/// - Makes up to `retry_num` attempts to download the file
/// - Waits `sleep_time` between retries
/// - Times out after 128 seconds per download attempt
/// - If `sha1` is provided, verifies the downloaded data against the hash
/// - Skips verification if `sha1` is `None`
///
/// # Errors
///
/// Returns an error if:
/// - All retry attempts fail
/// - Network connectivity issues occur
/// - Timeout is exceeded
/// - SHA1 verification fails (if hash is provided)
///
/// # Example
///
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
/// # Panics
///
/// This function does not panic. All error conditions are handled and returned
/// via the `Result` type.
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
            .timeout(Duration::from_secs(128))
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
    /// This method checks if the file needs to be downloaded based on:
    /// - Whether the file exists at the target location
    /// - Whether the existing file's SHA1 hash matches the expected hash
    /// - Whether a SHA1 hash is provided
    ///
    /// # Installation Logic
    ///
    /// 1. If `sha1` is `None`, always download the file
    /// 2. If `sha1` is `Some`:
    ///    - Skip download if file exists and hash matches
    ///    - Download if file doesn't exist or hash doesn't match
    ///
    /// # Download Behavior
    ///
    /// - Downloads with retry logic (5 attempts, 10 second delay between retries)
    /// - Creates parent directories if they don't exist
    /// - Writes downloaded data to the target file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Download fails after all retry attempts
    /// - Filesystem permissions prevent directory creation
    /// - Filesystem permissions prevent file writing
    /// - SHA1 verification fails
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The file path has no parent directory (e.g., a root path or path with no directory component)
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
    /// Increments the progress counter and updates the progress message
    /// with the task's message.
    ///
    /// # Parameters
    ///
    /// * `bar` - Reference to the shared progress bar
    fn bar_update(&self, bar: &ProgressBar) {
        bar.inc(1);
        bar.set_message(self.message.clone());
    }
}

/// A pool of installation tasks that can be executed concurrently.
///
/// This structure manages a collection of installation tasks and provides
/// functionality to execute them concurrently with progress tracking.
///
/// # Type Parameters
///
/// * `T` - The task type, must implement `FileInstall`, `Send`, `Sync`, `Clone`, and `'static`
///
/// # Fields
///
/// * `pool` - A deque of tasks to be executed
/// * `bar` - A progress bar for tracking installation progress
///
/// # Features
///
/// - **Concurrent Execution**: Tasks are executed with configurable concurrency (64 by default)
/// - **Progress Tracking**: Visual progress bar with elapsed time and task messages
/// - **Thread Safety**: All tasks are executed safely in parallel
/// - **Error Handling**: Errors in individual tasks will cause the entire installation to panic
///
/// # Example
///
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
    pub pool: VecDeque<T>,
    bar: ProgressBar,
}

/// Creates a `TaskPool` from a `VecDeque` of tasks.
///
/// This implementation automatically creates and configures a progress bar
/// for tracking the installation progress.
///
/// # Progress Bar Configuration
///
/// The progress bar is configured with:
/// - Length equal to the number of tasks
/// - Template: `[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}`
/// - Progress characters: `##-`
///
/// # Example
///
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
/// ]);
///
/// let pool = TaskPool::from(tasks);
/// // Pool is ready to install
/// ```
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
    /// This method processes all tasks in the pool with the following characteristics:
    ///
    /// # Execution Model
    ///
    /// - Tasks are executed concurrently with a buffer of 64 parallel operations
    /// - Each task runs in its own async context
    /// - Progress is updated in real-time as tasks complete
    ///
    /// # Concurrency
    ///
    /// - Up to 64 tasks execute simultaneously
    /// - Tasks are pulled from the deque as they complete
    /// - This provides optimal throughput for I/O-bound operations
    ///
    /// # Progress Tracking
    ///
    /// - Progress bar updates as each task completes
    /// - Task messages are displayed in the progress bar
    /// - Progress bar shows elapsed time, progress bar, and task count
    ///
    /// # Error Handling
    ///
    /// - Errors in individual tasks cause the entire installation to panic
    /// - This method does not handle errors gracefully; any task failure will abort the process
    /// - For error-tolerant behavior, implement custom error handling in your task types
    ///
    /// # Panics
    ///
    /// This method will panic if:
    /// - Any task's `install` method returns an error
    /// - Progress bar update fails
    ///
    /// # Blocking Behavior
    ///
    /// This method is marked with `#[tokio::main(flavor = "current_thread")]`, which means:
    /// - It creates a new tokio runtime in the current thread
    /// - It blocks until all tasks complete
    /// - It's designed to be called from outside an async context
    ///
    /// # Example
    ///
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
    /// # Performance Considerations
    ///
    /// - The default concurrency of 64 is optimal for most network I/O operations
    /// - For CPU-bound tasks, consider reducing the buffer size
    /// - For very large numbers of tasks, consider batching
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
