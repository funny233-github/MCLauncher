pub mod asyncuntil;
use crate::asyncuntil::AsyncIterator;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use reqwest::header;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub trait FileInstall {
    fn install(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
    fn bar_update(&self, bar: &ProgressBar);
}

trait ShaCompare {
    fn sha1_cmp<C>(&self, sha1code: C) -> Ordering
    where
        C: AsRef<str> + Into<String>;
}

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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InstallTask {
    pub url: String,
    pub sha1: Option<String>,
    pub save_file: PathBuf,
    pub message: String,
}

async fn fetch_bytes(
    url: &String,
    sha1: &Option<String>,
    sleep_time: Duration,
    retry_num: u32,
) -> anyhow::Result<bytes::Bytes> {
    let client = reqwest::Client::new();
    for _ in 0..retry_num {
        let send = client
            .get(url)
            .header(header::USER_AGENT, "github.com/funny233-github/MCLauncher")
            .timeout(Duration::from_secs(10))
            .send()
            .await;
        let data = match send {
            Ok(_send) => _send.bytes().await,
            Err(e) => Err(e),
        };
        if let Ok(_data) = data {
            if sha1.is_none() || _data.sha1_cmp(sha1.as_ref().unwrap()).is_eq() {
                return Ok(_data);
            }
        };
        warn!("install fail, then retry");
        tokio::time::sleep(sleep_time).await;
    }
    Err(anyhow::anyhow!("download {url} fail"))
}

impl FileInstall for InstallTask {
    async fn install(&self) -> anyhow::Result<()> {
        if self.sha1.is_none()
            || !(self.save_file.exists()
                && fs::read(&self.save_file)
                    .unwrap()
                    .sha1_cmp(self.sha1.as_ref().unwrap())
                    .is_eq())
        {
            let data = fetch_bytes(&self.url, &self.sha1, Duration::from_secs(3), 5).await?;
            fs::create_dir_all(self.save_file.parent().unwrap()).unwrap();
            fs::write(&self.save_file, data).unwrap();
        }
        Ok(())
    }
    fn bar_update(&self, bar: &ProgressBar) {
        bar.inc(1);
        bar.set_message(self.message.clone());
    }
}

#[derive(Debug, Clone)]
pub struct TaskPool<T>
where
    T: FileInstall + std::marker::Send + std::marker::Sync + Clone + 'static,
{
    pub pool: VecDeque<T>,
    bar: ProgressBar,
}

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

const MAX_THREAD: usize = 64;

impl<T> TaskPool<T>
where
    T: FileInstall + std::marker::Send + std::marker::Sync + Clone,
{
    //Execute all install task.
    //# Error
    //Return Error when install fail 5 times
    pub fn install(self) -> anyhow::Result<()> {
        self.pool
            .into_iter()
            .map(|x| {
                let share = self.bar.clone();
                async move {
                    x.install().await.unwrap();
                    x.bar_update(&share);
                }
            })
            .async_execute(MAX_THREAD);
        Ok(())
    }
}
