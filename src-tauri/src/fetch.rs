//! The one place the app goes onto the internet: fetching web pages when the user asks to
//! save a website. Everything else about saving (what to fetch, what to keep) is in
//! `cindrix_core::crawl`.

use cindrix_core::crawl::{Fetcher, Response, USER_AGENT};
use std::io::Read;
use std::time::Duration;

pub struct WebFetcher {
    agent: ureq::Agent,
}

impl WebFetcher {
    pub fn new() -> WebFetcher {
        WebFetcher {
            agent: ureq::AgentBuilder::new()
                .user_agent(USER_AGENT)
                .timeout_connect(Duration::from_secs(15))
                .timeout_read(Duration::from_secs(30))
                .redirects(8)
                .build(),
        }
    }
}

impl Fetcher for WebFetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Response, String> {
        let resp = match self.agent.get(url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(e.to_string().chars().take(160).collect()),
        };
        let status = resp.status();
        let final_url = resp.get_url().to_string();
        let content_type = resp.header("content-type").unwrap_or("").to_string();
        let mut body = Vec::new();
        if status == 200 {
            resp.into_reader()
                .take(max_bytes as u64 + 1)
                .read_to_end(&mut body)
                .map_err(|e| e.to_string())?;
            if body.len() > max_bytes {
                return Err("file too large".into());
            }
        }
        Ok(Response { status, final_url, content_type, body })
    }
}
