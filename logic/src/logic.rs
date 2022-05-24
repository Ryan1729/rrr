use std::path::{Path, PathBuf};
use fetch::Url;
use syndicated::Post;
use timestamp::{Timestamp, UtcOffset};

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
struct Posts {
    posts: Vec<syndicated::Post>,
    fetched_at: Timestamp,
}

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
    remote_feeds: &RemoteFeeds,
    utc_offset: UtcOffset,
) -> Result<(), FetchRemoteFeedsError> {
    output.posts.clear();

    // Set the timestamp first, so that if we add an auto refresh later, errors won't
    // cause a tight retry loop.
    output.fetched_at = Timestamp::now_at_offset(utc_offset);

    for feed in remote_feeds {
        use std::io::Read;
        let mut reader = fetch::get(feed)
            .map_err(FetchRemoteFeedsError::Fetch)?;

        let mut buffer = String::with_capacity(4096);
        reader.read_to_string(&mut buffer)
            .map_err(FetchRemoteFeedsError::Io)?;

        syndicated::parse_items(
            std::io::Cursor::new(&buffer),
            &mut output.posts,
        );
    }

    Ok(())
}

const LOCAL_FEEDS: &str = "local-feeds";

/// `output` will be cleared before being filled with the current posts.
fn load_local_posts(
    output: &mut Posts,
    root: &Root,
    utc_offset: UtcOffset,
) -> std::io::Result<()> {
    output.posts.clear();

    let local_feeds_dir = root.path_to(LOCAL_FEEDS);
    let local_feeds_dir = ensure_directory(local_feeds_dir)?;

    todo!("load_local_posts")
}

pub struct State {
    root: Root,
    remote_feeds: RemoteFeeds,
    remote_posts: Posts,
    local_posts: Posts,
    utc_offset: UtcOffset,
}

impl State {
    pub fn root_display(&self) -> impl core::fmt::Display + '_ {
        self.root.display()
    }
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
        let root = ensure_directory(root)
            .map_err(Self::Error::Io)?;

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

        let utc_offset = UtcOffset::current_local_or_utc();

        let mut local_posts = Posts{
            posts: Vec::with_capacity(1024),
            fetched_at: Timestamp::DEFAULT,
        };

        load_local_posts(&mut local_posts, &root, utc_offset)
            .map_err(Self::Error::Io)?;

        let mut remote_posts = Posts{
            posts: Vec::with_capacity(1024),
            fetched_at: Timestamp::DEFAULT,
        };

        fetch_remote_feeds(&mut remote_posts, &remote_feeds, utc_offset)?;

        Ok(Self {
            root,
            remote_feeds,
            remote_posts,
            local_posts,
            utc_offset,
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

type Flags = u8;

const REFRESH_LOCAL: Flags  = 0b0000_0001;
const REFRESH_REMOTE: Flags = 0b0000_0010;

#[derive(Debug)]
pub enum Task {
    ShowHomePage(Flags),
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

    let mut flags = 0;
    if let Some(_) = spec.query_param(keys::REFRESH_LOCAL) {
        flags |= REFRESH_LOCAL;
    }

    if let Some(_) = spec.query_param(keys::REFRESH_REMOTE) {
        flags |= REFRESH_REMOTE;
    }

    let url = spec.url_suffix();
    match (spec.method(), url.as_ref()) {
        (Method::Get, "/") => {
            Ok(ShowHomePage(flags))
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
            ShowHomePage(flags) => {
                if flags & REFRESH_LOCAL != 0 {
                    load_local_posts(
                        &mut self.local_posts,
                        &self.root,
                        self.utc_offset,
                    ).map_err(PerformError::Io)?;
                }

                if flags & REFRESH_REMOTE != 0 {
                    fetch_remote_feeds(
                        &mut self.remote_posts,
                        &self.remote_feeds,
                        self.utc_offset,
                    )?;
                }
            }
        }

        // 64k ought to be enough for anybody!
        let mut output = Output::Html(String::with_capacity(65536));

        render::home_page(
            &mut output,
            &Data { 
                root: &self.root,
                local_posts: &self.local_posts,
                remote_posts: &self.remote_posts,
            }
        ).map_err(PerformError::Render)?;

        Ok(output)
    }
}

fn ensure_directory(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    if !path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Not a directory: {}", path.display())
        ))
    }
    path.canonicalize()
}

struct Data<'root, 'posts> {
    root: &'root Root,
    local_posts: &'posts Posts,
    remote_posts: &'posts Posts,
}

impl <'posts> render::Data<'_> for Data<'_, 'posts> {
    type Link = String;
    type PostHolder = &'posts Post;
    type Posts = std::slice::Iter<'posts, Post>;
    type Sections = std::array::IntoIter<
        render::Section<Self::Posts, Self::Timestamp>,
        2
    >;
    type RootDisplay = String;
    type Timestamp = Timestamp;

    fn post_sections(&self) -> Self::Sections {
        use render::SectionKind::*;
        [
            render::Section {
                kind: Local,
                posts: self.local_posts.posts.iter(),
                timestamp: self.local_posts.fetched_at,
            },
            render::Section {
                kind: Remote,
                posts: self.remote_posts.posts.iter(),
                timestamp: self.remote_posts.fetched_at,
            },
        ].into_iter()
    }

    fn root_display(&self) -> Self::RootDisplay {
        format!("{}", self.root.display())
    }
}