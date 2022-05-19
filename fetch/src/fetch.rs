pub use ureq::Error;
pub use url::{
    Url,
    ParseError as UrlParseError,
};


static AGENT: once_cell::sync::Lazy<ureq::Agent> = once_cell::sync::Lazy::new(|| {
    ureq::AgentBuilder::new()
        .build()
});

pub fn get(url: &Url) -> Result<
    std::io::BufReader<impl std::io::Read + Send>,
    Error
> {
    AGENT
        .request_url("GET", url)
        .call()
        .map(|resp| {
            std::io::BufReader::new(resp.into_reader())
        })
}
