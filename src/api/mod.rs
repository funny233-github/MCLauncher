use regex::Regex;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
macro_rules! fetch {
    ($client:ident,$url:ident, $type:ident) => {{
        let mut res = Err(anyhow::anyhow!("fetch fail"));
        for _ in 0..5 {
            let send = $client
                .get(&$url)
                .header(reqwest::header::USER_AGENT, "mc_launcher")
                .send();
            let data = send.and_then(|x| x.$type());
            if let Ok(_data) = data {
                res = Ok(_data);
                break;
            }
            log::warn!("fetch fail, then retry");
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        res
    }};
    ($client:ident,$url:ident,$sha1:ident, $type:ident) => {{
        let mut res = Err(anyhow::anyhow!("fetch fail"));
        for _ in 0..5 {
            let send = $client
                .get(&$url)
                .header(reqwest::header::USER_AGENT, "mc_launcher")
                .send();
            let data = send.and_then(|x| x.$type());
            if let Ok(_data) = data {
                if _data.sha1_cmp(&$sha1).is_eq() {
                    res = Ok(_data);
                    break;
                }
            }
            log::warn!("fetch fail, then retry");
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        res
    }};
}

pub trait Sha1Compare {
    fn sha1_cmp(&self, sha1code: &str) -> Ordering;
}

pub trait DomainReplacer<T> {
    fn replace_domain(&self, domain: &str) -> T;
}

impl DomainReplacer<String> for String {
    fn replace_domain(&self, domain: &str) -> String {
        let regex = Regex::new(r"(?<replace>https://\S+?/)").unwrap();
        let replace = regex.captures(self.as_str()).unwrap();
        self.replace(&replace["replace"], domain)
    }
}

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

pub mod official;
