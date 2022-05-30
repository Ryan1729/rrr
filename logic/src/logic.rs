use std::path::{Path, PathBuf};
use fetch::Url;
use timestamp::{Timestamp, UtcOffset};

pub use syndicated::Post;

macro_rules! other {
    ($($tokens: tt)+) => {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!($($tokens)*)
        )
    }
}

#[derive(PartialEq, Eq)]
pub struct Root(PathBuf);

impl Root {
    fn path_to(&self, file_name: impl AsRef<std::path::Path>) -> PathBuf {
        self.0.join(file_name)
    }

    fn display(&self) -> std::path::Display<'_> {
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
/// `paths` will be cleared before being filled with the current paths.
fn load_local_posts(
    output: &mut Posts,
    paths: &mut Vec<LocalFeedPath>,
    local_feeds_dir: &LocalFeedsDir,
    utc_offset: UtcOffset,
) -> std::io::Result<()> {
    output.posts.clear();

    // Set the timestamp first, so that if we add an auto refresh later, errors won't
    // cause a tight retry loop.
    output.fetched_at = Timestamp::now_at_offset(utc_offset);

    load_local_feed_paths(paths, local_feeds_dir)?;

    for path in paths {
        // TODO is it worth switching to reusing a single buffer across iterations?
        let buffer = std::fs::read_to_string(path)?;

        syndicated::parse_items(
            std::io::Cursor::new(&buffer),
            &mut output.posts,
        );
    }

    Ok(())
}

/// `output` will be cleared before being filled with the current paths.
fn load_local_feed_paths(
    output: &mut Vec<LocalFeedPath>,
    local_feeds_dir: &LocalFeedsDir,
) -> std::io::Result<()> {
    output.clear();

    for entry in std::fs::read_dir(local_feeds_dir)? {
        output.push(
            LocalFeedPath::new(entry?.path(), local_feeds_dir)
                .map_err(|BadPrefixError()|
                    other!("Got file that was not in the local_feeds_dir")
                )?
        );
    }

    Ok(())
}

#[repr(transparent)]
struct LocalFeedsDir(PathBuf);

impl AsRef<Path> for LocalFeedsDir {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl LocalFeedsDir {
    fn new(root: &Root) -> std::io::Result<Self> {
        ensure_directory(root.path_to(LOCAL_FEEDS))
            .map(Self)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct LocalFeedPath(PathBuf);

impl AsRef<Path> for LocalFeedPath {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

struct BadPrefixError();

impl LocalFeedPath {
    fn new(
        path: PathBuf,
        local_feeds_dir: &LocalFeedsDir
    ) -> Result<Self, BadPrefixError> {
        if path.starts_with(local_feeds_dir) {
            Ok(Self(path))
        } else {
            Err(BadPrefixError())
        }
    }
}

pub struct State {
    root: Root,
    remote_feeds: RemoteFeeds,
    remote_posts: Posts,
    local_posts: Posts,
    local_feed_paths: Vec<LocalFeedPath>,
    local_feeds_dir: LocalFeedsDir,
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

impl From<std::io::Error> for StateCreationError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}


const REMOTE_FEEDS: &str = "remote-feeds";

impl TryFrom<PathBuf> for State {
    type Error = StateCreationError;

    fn try_from(root: PathBuf) -> Result<Self, Self::Error> {
        let root = ensure_directory(root)?;

        let root = Root::try_from(root)
            .map_err(|MustBeDirError()| Self::Error::RootMustBeDir)?;

        let mut remote_feeds_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(root.path_to(REMOTE_FEEDS))?;

        let mut remote_feeds_string = String::with_capacity(1024);

        std::io::Read::read_to_string(
            &mut remote_feeds_file,
            &mut remote_feeds_string
        )?;

        // TODO store count in file on disk?
        let mut remote_feeds = Vec::with_capacity(16);

        for line in remote_feeds_string.lines() {
            remote_feeds.push(
                Url::parse(line)
                    .map_err(Self::Error::UrlParse)?
            );
        }

        let utc_offset = UtcOffset::current_local_or_utc();

        let mut local_feed_paths = Vec::with_capacity(8);

        let mut local_posts = Posts{
            posts: Vec::with_capacity(1024),
            fetched_at: Timestamp::DEFAULT,
        };

        let local_feeds_dir = LocalFeedsDir::new(
            &root,
        )?;

        load_local_posts(
            &mut local_posts,
            &mut local_feed_paths,
            &local_feeds_dir,
            utc_offset
        )?;

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
            local_feed_paths,
            local_feeds_dir,
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
pub struct LocalAddForm {
    pub path: PathBuf,
    pub post: Post,
}

#[derive(Debug)]
pub enum Task {
    ShowHomePage(Flags),
    ShowLocalAddForm,
    SubmitLocalAddForm(LocalAddForm)
}

pub enum Method {
    Get,
    Post,
    Other,
}

impl core::fmt::Display for Method {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Other => write!(f, "???"),
        }
    }
}

pub trait TaskSpec
where
    Self::LocalAddFormError: std::error::Error,
{
    fn method(&self) -> Method;
    fn url_suffix(&self) -> String;
    fn query_param(&self, key: &str) -> Option<String>;
    type LocalAddFormError;
    fn local_add_form(&self) -> Result<Vec<(String, String)>, Self::LocalAddFormError>;
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

    use render::{page_names, param_keys, form_names};

    let url = spec.url_suffix();
    match (spec.method(), url.as_ref()) {
        (Method::Get, "/") => {
            let mut flags = 0;
            if let Some(_) = spec.query_param(param_keys::REFRESH_LOCAL) {
                flags |= REFRESH_LOCAL;
            }

            if let Some(_) = spec.query_param(param_keys::REFRESH_REMOTE) {
                flags |= REFRESH_REMOTE;
            }

            Ok(ShowHomePage(flags))
        },
        (Method::Get, page_names::LOCAL_ADD) => {
            Ok(ShowLocalAddForm)
        },
        (Method::Post, page_names::LOCAL_ADD) => {
            spec.local_add_form()
                .map_err(|e| TaskError(e.to_string()))
                .and_then(|pairs| {
                    let mut path = PathBuf::default();

                    let mut title = None;
                    let mut summary = None;
                    let mut content = None;
                    // I predict that there will almost always be exactly 1 link.
                    let mut links = Vec::with_capacity(1);

                    for (k, v) in pairs {
                        match k.as_str() {
                            form_names::TARGET => {
                                path = PathBuf::from(v);
                            }
                            form_names::TITLE => {
                                title = Some(v);
                            }
                            form_names::SUMMARY => {
                                summary = Some(v);
                            }
                            form_names::CONTENT => {
                                content = Some(v);
                            }
                            form_names::LINK => {
                                // TODO confirm we can actually get multiple links
                                // here. We might need to encode multiple links into
                                // one string, or just only allow one link.
                                links.push(v);
                            }
                            _ => {
                                return Err(TaskError(format!(
                                    "Unhandled Form pair ({k}, {v})"
                                )))
                            }
                        }
                    }

                    Ok(SubmitLocalAddForm(LocalAddForm {
                        path,
                        post: Post {
                            title,
                            summary,
                            content,
                            links,
                        },
                    }))
                })
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

impl From<std::io::Error> for PerformError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<render::Error> for PerformError {
    fn from(e: render::Error) -> Self {
        Self::Render(e)
    }
}

impl State {
    pub fn perform(&mut self, task: Task) -> Result<Output, PerformError> {
        use Task::*;

        // 64k ought to be enough for anybody!
        let mut output = Output::Html(String::with_capacity(65536));

        match task {
            ShowHomePage(flags) => {
                if flags & REFRESH_LOCAL != 0 {
                    load_local_posts(
                        &mut self.local_posts,
                        &mut self.local_feed_paths,
                        &self.local_feeds_dir,
                        self.utc_offset,
                    )?;
                }

                if flags & REFRESH_REMOTE != 0 {
                    fetch_remote_feeds(
                        &mut self.remote_posts,
                        &self.remote_feeds,
                        self.utc_offset,
                    )?;
                }

                render::home_page(
                    &mut output,
                    &Data {
                        root: &self.root,
                        local_posts: &self.local_posts,
                        remote_posts: &self.remote_posts,
                    },
                )?;
            },
            ShowLocalAddForm => {
                load_local_feed_paths(
                    &mut self.local_feed_paths,
                    &self.local_feeds_dir
                )?;

                render::local_add_form(
                    &mut output,
                    self.local_feed_paths
                        .iter()
                        .map(|path| Target {
                            path: path.as_ref(),
                            root: &self.root,
                        }),
                    &Data {
                        root: &self.root,
                        local_posts: &self.local_posts,
                        remote_posts: &self.remote_posts,
                    },
                    None,
                )?;
            }
            SubmitLocalAddForm(form) => {
                match add_local_feed(form) {
                    Ok(()) => {
                        render::local_add_form_success(&mut output)?
                    }
                    Err(form) => {
                        render::local_add_form(
                            &mut output,
                            self.local_feed_paths
                                .iter()
                                .map(|path| Target {
                                    path: path.as_ref(),
                                    root: &self.root,
                                }),
                            &Data {
                                root: &self.root,
                                local_posts: &self.local_posts,
                                remote_posts: &self.remote_posts,
                            },
                            Some(self.previous_form(form))
                        )?;
                    }
                }
            }
        }

        Ok(output)
    }

    fn previous_form(&self, _form: LocalAddForm) -> render::LocalAddForm<Target<'_, '_>> {
        todo!("previous_form")
    }
}

fn add_local_feed(_form: LocalAddForm) -> Result<(), LocalAddForm> {
    // Will probably need to return an error message as well.
    todo!("add_local_feed")
}

fn ensure_directory(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let path = path.as_ref();
    std::fs::create_dir_all(path)?;
    if !path.is_dir() {
        return Err(other!("Not a directory: {}", path.display()))
    }
    path.canonicalize()
}

struct Data<'root, 'posts> {
    root: &'root Root,
    local_posts: &'posts Posts,
    remote_posts: &'posts Posts,
}

impl <'root> render::RootDisplay for Data<'root, '_> {
    type RootDisplay = std::path::Display<'root>;

    fn root_display(&self) -> Self::RootDisplay {
        self.root.display()
    }
}

impl <'posts> render::Data<'_> for Data<'_, 'posts> {
    type Link = String;
    type PostHolder = &'posts Post;
    type Posts = std::slice::Iter<'posts, Post>;
    type Sections = std::array::IntoIter<
        render::Section<Self::Posts, Self::Timestamp>,
        2
    >;
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
}

#[derive(PartialEq, Eq)]
struct Target<'path, 'root> {
    path: &'path Path,
    root: &'root Root,
}

impl <'path> render::Target for Target<'path, '_> {
    type Label = std::path::Display<'path>;
    type Value = std::path::Display<'path>;

    fn label(&self) -> Self::Label {
        self.path
            .strip_prefix(&self.root.0)
            .unwrap_or(&self.path)
            .display()
    }

    fn value(&self) -> Self::Value {
        self.path.display()
    }
}