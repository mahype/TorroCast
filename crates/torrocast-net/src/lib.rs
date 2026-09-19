//! HTTP for TorroCast. Everything that goes over the network passes through
//! [`Fetch`], so directories and feeds can be tested against recorded answers
//! and the real client lives in exactly one place.

use std::fmt;
use std::io::Read;
use std::time::Duration;

/// Directories refuse generic agents (Podcast Index answers 403), and hosters
/// count downloads by agent: say who we are.
pub const USER_AGENT: &str =
    concat!("TorroCast/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mahype/TorroCast)");

/// Feeds of long-running shows reach several megabytes; beyond this something is wrong.
pub const MAX_BODY: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The server answered, but not with success.
    Status(u16),
    /// No answer at all: DNS, connection, TLS, timeout.
    Unreachable(String),
    /// The answer is larger than we are willing to hold.
    TooLarge,
    /// A range was asked for and the server sent something else.
    RangeIgnored,
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(code) => write!(formatter, "HTTP {code}"),
            Self::Unreachable(reason) => write!(formatter, "unreachable: {reason}"),
            Self::TooLarge => write!(formatter, "answer too large"),
            Self::RangeIgnored => write!(formatter, "server ignored the byte range"),
        }
    }
}

impl std::error::Error for FetchError {}

pub trait Fetch: Send + Sync {
    /// The whole body of `url`.
    fn get(&self, url: &str) -> Result<Vec<u8>, FetchError>;

    /// Bytes `start..=end` of `url`. Used to read a tag at the head of an
    /// audio file without downloading the episode.
    fn get_range(&self, url: &str, start: u64, end: u64) -> Result<Vec<u8>, FetchError>;
}

pub struct HttpClient {
    agent: ureq::Agent,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    #[must_use]
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(45))
            .redirects(8)
            .build();
        Self { agent }
    }

    fn read(response: ureq::Response, limit: u64) -> Result<Vec<u8>, FetchError> {
        let mut body = Vec::new();
        response
            .into_reader()
            .take(limit + 1)
            .read_to_end(&mut body)
            .map_err(|error| FetchError::Unreachable(error.to_string()))?;
        if body.len() as u64 > limit {
            return Err(FetchError::TooLarge);
        }
        Ok(body)
    }
}

fn translate(error: ureq::Error) -> FetchError {
    match error {
        ureq::Error::Status(code, _) => FetchError::Status(code),
        ureq::Error::Transport(transport) => FetchError::Unreachable(transport.to_string()),
    }
}

impl Fetch for HttpClient {
    fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let response = self.agent.get(url).call().map_err(translate)?;
        Self::read(response, MAX_BODY)
    }

    fn get_range(&self, url: &str, start: u64, end: u64) -> Result<Vec<u8>, FetchError> {
        let response = self.agent.get(url).set("Range", &format!("bytes={start}-{end}")).call().map_err(translate)?;
        // A server that ignores the range sends the whole episode with 200.
        // Reading that would be the download we set out to avoid.
        if response.status() != 206 {
            return Err(FetchError::RangeIgnored);
        }
        Self::read(response, end - start + 1)
    }
}

/// Percent-encodes a query value.
#[must_use]
pub fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => encoded.push(char::from(byte)),
            b' ' => encoded.push('+'),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encodes_umlauts_and_spaces() {
        assert_eq!(encode("lage der nation"), "lage+der+nation");
        assert_eq!(encode("Küche & Co"), "K%C3%BCche+%26+Co");
    }
}
