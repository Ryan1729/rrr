use std::path::PathBuf;
use fetch::Url;
use syndicated::Post;

pub struct Root(PathBuf);

impl Root {
    fn path_to(&self, file_name: impl AsRef<std::path::Path>) -> PathBuf {
        self.0.join(file_name)
    }

    fn display(&self) -> impl core::fmt::Display + '_ {
        self.0.display()
    }
}

pub struct MustBeDirError();

impl TryFrom<PathBuf> for Root {
    type Error = MustBeDirError;

    fn try_from(root: PathBuf) -> Result<Self, Self::Error> {
        if !root.is_dir() {
            Err(MustBeDirError())
        } else {
            Ok(Self(root))
        }
    }
}

type RemoteFeeds = Vec<Url>;
type Posts = Vec<syndicated::Post>;

enum FetchRemoteFeedsError {
    Io(std::io::Error),
    Fetch(fetch::Error),
}

impl core::fmt::Display for FetchRemoteFeedsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Fetch(e) => write!(f, "{e}"),
        }
    }
}

/// `output` will be cleared before being filled with the current posts.
fn fetch_remote_feeds(
    output: &mut Posts,
    remote_feeds: &RemoteFeeds
) -> Result<(), FetchRemoteFeedsError> {
    output.clear();

    for feed in remote_feeds {
        use std::io::Read;
        let mut reader = fetch::get(feed)
            .map_err(FetchRemoteFeedsError::Fetch)?;

        let mut buffer = String::with_capacity(4096);
        reader.read_to_string(&mut buffer)
            .map_err(FetchRemoteFeedsError::Io)?;

        syndicated::parse_items(
            std::io::Cursor::new(&buffer),
            output,
        );
    }

    Ok(())
}

pub struct State {
    root: Root,
    // We plan to re-fetch from the feeds during runtime, so we'll want to avoid
    // re-parsing.
    #[allow(unused)]
    remote_feeds: RemoteFeeds,
    posts: Posts,
}

#[derive(Debug)]
pub enum StateCreationError {
    RootMustBeDir,
    Io(std::io::Error),
    UrlParse(fetch::UrlParseError),
    Fetch(fetch::Error),
}

impl core::fmt::Display for StateCreationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RootMustBeDir => write!(f, "Root dir must be a dir"),
            Self::Io(e) => write!(f, "{e}"),
            Self::UrlParse(e) => write!(f, "{e}"),
            Self::Fetch(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StateCreationError {}

impl From<FetchRemoteFeedsError> for StateCreationError {
    fn from(e: FetchRemoteFeedsError) -> Self {
        use FetchRemoteFeedsError as E;
        match e {
            E::Io(e) => Self::Io(e),
            E::Fetch(e) => Self::Fetch(e),
        }
    }
}

const REMOTE_FEEDS: &str = "remote-feeds";

impl TryFrom<PathBuf> for State {
    type Error = StateCreationError;

    fn try_from(root: PathBuf) -> Result<Self, Self::Error> {
        let root = Root::try_from(root)
            .map_err(|MustBeDirError()| Self::Error::RootMustBeDir)?;

        let mut remote_feeds_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(root.path_to(REMOTE_FEEDS))
            .map_err(Self::Error::Io)?;

        let mut remote_feeds_string = String::with_capacity(1024);

        std::io::Read::read_to_string(
            &mut remote_feeds_file,
            &mut remote_feeds_string
        ).map_err(Self::Error::Io)?;

        // TODO store count in file on disk?
        let mut remote_feeds = Vec::with_capacity(16);

        for line in remote_feeds_string.lines() {
            remote_feeds.push(
                Url::parse(line)
                    .map_err(Self::Error::UrlParse)?
            );
        }

        let mut posts = Vec::with_capacity(1024);

        fetch_remote_feeds(&mut posts, &remote_feeds)?;

        Ok(Self {
            root,
            remote_feeds,
            posts,
        })
    }
}

pub enum Output {
    Html(String),
}

impl core::fmt::Write for Output {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self {
            Output::Html(ref mut output) => {
                output.push_str(s);

                Ok(())
            }
        }
    }
}

impl render::Output for Output {}

#[derive(Debug)]
pub struct Refresh;

#[derive(Debug)]
pub enum Task {
    ShowHomePage(Option<Refresh>),
}

pub enum Method {
    Get,
//    Post,
    Other,
}

impl core::fmt::Display for Method {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Other => write!(f, "???"),
        }
    }
}

pub trait TaskSpec {
    fn method(&self) -> Method;
    fn url_suffix(&self) -> String;
    fn query_param(&self, key: &str) -> Option<String>;
}

#[derive(Debug)]
pub struct TaskError(String);

impl core::fmt::Display for TaskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TaskError {}

pub fn extract_task(spec: &impl TaskSpec) -> Result<Task, TaskError> {
    use Task::*;
    
    use render::keys;

    let mut extra = None;
    if let Some(_) = spec.query_param(keys::REFRESH) {
        extra = Some(Refresh);
    }

    let url = spec.url_suffix();
    match (spec.method(), url.as_ref()) {
        (Method::Get, "/") => {
            Ok(ShowHomePage(extra))
        },
        (method, _) => {
            Err(TaskError(
                format!(
                    "No known task for HTTP {method} method at url {url}"
                )
            ))
        },
    }
}

struct Data<'root, 'posts> {
    root: &'root Root,
    posts: &'posts [Post],
}

impl <'posts> render::Data<'_> for Data<'_, 'posts> {
    type RootDisplay = String;
    type PostHolder = Post;
    type Link = String;

    fn posts(&self) -> &'posts [Post] {
        self.posts
    }

    fn root_display(&self) -> Self::RootDisplay {
        format!("{}", self.root.display())
    }
}

#[derive(Debug)]
pub enum PerformError {
    Io(std::io::Error),
    Fetch(fetch::Error),
    Render(render::Error),
}

impl core::fmt::Display for PerformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Fetch(e) => write!(f, "{e}"),
            Self::Render(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PerformError {}

impl From<FetchRemoteFeedsError> for PerformError {
    fn from(e: FetchRemoteFeedsError) -> Self {
        use FetchRemoteFeedsError as E;
        match e {
            E::Io(e) => Self::Io(e),
            E::Fetch(e) => Self::Fetch(e),
        }
    }
}

impl State {
    pub fn perform(&mut self, task: Task) -> Result<Output, PerformError> {
        use Task::*;

        match task {
            ShowHomePage(None) => {}
            ShowHomePage(Some(Refresh)) => {
                fetch_remote_feeds(&mut self.posts, &self.remote_feeds)?;
            }
        }

        // 64k ought to be enough for anybody!
        let mut output = Output::Html(String::with_capacity(65536));

        render::home_page(
            &mut output,
            &Data { root: &self.root, posts: &self.posts }
        ).map_err(PerformError::Render)?;

        Ok(output)
    }
}