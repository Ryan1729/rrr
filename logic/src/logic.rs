use std::collections::BTreeMap;
use std::fs::File;
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

struct RemoteFeeds {
    feeds: Vec<Url>,
    fetched_at: Timestamp,
}

impl RemoteFeeds {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            feeds: Vec::with_capacity(capacity),
            fetched_at: Timestamp::DEFAULT,
        }
    }
}

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

    for feed in &remote_feeds.feeds {
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

/// `output` will be cleared before being filled with the current paths and posts.
fn load_local_posts(
    output: &mut LocalPosts,
    local_feeds_dir: &LocalFeedsDir,
    utc_offset: UtcOffset,
) -> std::io::Result<()> {
    load_local_feed_paths(output, local_feeds_dir)?;

    for (path, posts) in output.iter_mut() {
        // TODO is it worth switching to reusing a single buffer across iterations?
        let buffer = std::fs::read_to_string(path)?;

        load_local_post_from_buffer(
            posts,
            std::io::Cursor::new(&buffer),
            utc_offset,
        );
    }

    Ok(())
}

/// `output` will be cleared before being filled with the current posts.
fn load_local_post_from_buffer(
    output: &mut Posts,
    feed_buffer: impl std::io::BufRead + std::io::Seek,
    utc_offset: UtcOffset,
) {
    output.posts.clear();
    output.fetched_at = Timestamp::now_at_offset(utc_offset);

    syndicated::parse_items(
        feed_buffer,
        &mut output.posts,
    );
}

/// Any existing lists of posts will be left alone.
fn load_local_feed_paths(
    output: &mut LocalPosts,
    local_feeds_dir: &LocalFeedsDir,
) -> std::io::Result<()> {
    output.clear();

    for entry in std::fs::read_dir(local_feeds_dir)? {
        let path = LocalFeedPath::new(entry?.path(), local_feeds_dir)
            .map_err(|BadPrefixError()|
                other!("Got file that was not in the local_feeds_dir")
            )?;

        output.entry(path)
            .or_insert_with(|| Posts{
                posts: Vec::with_capacity(1024),
                fetched_at: Timestamp::DEFAULT,
            });
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

mod local_feed_path {
    use super::*;

    /// We keep the PathBuf field private so that one of these cannot be constructed
    /// without confirming the path is inside a `LocalFeedsDir`.
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(transparent)]
    pub struct LocalFeedPath(PathBuf);

    impl AsRef<Path> for LocalFeedPath {
        fn as_ref(&self) -> &Path {
            self.0.as_ref()
        }
    }

    pub struct BadPrefixError();

    impl core::fmt::Display for BadPrefixError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Bad local path prefix")
        }
    }

    impl LocalFeedPath {
        pub(crate) fn new(
            path: PathBuf,
            local_feeds_dir: &LocalFeedsDir,
        ) -> Result<Self, BadPrefixError> {
            if path.starts_with(local_feeds_dir) {
                Ok(Self(path))
            } else {
                Err(BadPrefixError())
            }
        }
    }
}
use local_feed_path::{BadPrefixError, LocalFeedPath};

type LocalPosts = BTreeMap<LocalFeedPath, Posts>;

pub struct State {
    root: Root,
    remote_feeds: RemoteFeeds,
    remote_posts: Posts,
    local_posts: LocalPosts,
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

impl From<RemoteFeedUrlsError> for StateCreationError {
    fn from(e: RemoteFeedUrlsError) -> Self {
        use RemoteFeedUrlsError as E;
        match e {
            E::Io(e) => Self::Io(e),
            E::UrlParse(e) => Self::UrlParse(e),
        }
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

        // TODO store count in file on disk, so we can giva an accurate capacity?
        let mut remote_feeds = RemoteFeeds::with_capacity(16);

        let utc_offset = UtcOffset::current_local_or_utc();

        load_remote_feed_urls(
            &mut remote_feeds_file,
            &mut remote_feeds,
            utc_offset,
        )?;

        let mut local_posts = LocalPosts::new();

        let local_feeds_dir = LocalFeedsDir::new(
            &root,
        )?;

        load_local_posts(
            &mut local_posts,
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

const REFRESH_LOCAL: Flags       = 0b0000_0001;
const REFRESH_REMOTE: Flags      = 0b0000_0010;
const REFRESH_REMOTE_URLS: Flags = 0b0000_0100;

#[derive(Debug)]
pub struct LocalAddForm {
    pub path: LocalFeedPath,
    pub post: Post,
}

#[derive(Debug)]
pub struct RemoteFeedAddForm {
    pub url: String,
}

#[derive(Debug)]
pub enum Task {
    ShowHomePage(Flags),
    ShowLocalAddForm,
    SubmitLocalAddForm(LocalAddForm),
    ShowRemoteFeedAddForm,
    SubmitRemoteFeedAddForm(RemoteFeedAddForm),
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
    Self::RemoteFeedAddFormError: std::error::Error,
{
    fn method(&self) -> Method;
    fn url_suffix(&self) -> String;
    fn query_param(&self, key: &str) -> Option<String>;
    type LocalAddFormError;
    fn local_add_form(&self) -> Result<Vec<(String, String)>, Self::LocalAddFormError>;
    type RemoteFeedAddFormError;
    fn remote_feed_add_form(&self) -> Result<Vec<(String, String)>, Self::RemoteFeedAddFormError>;
}

#[derive(Debug)]
pub struct TaskError(String);

impl core::fmt::Display for TaskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TaskError {}

pub fn extract_task(
    spec: &impl TaskSpec,
    state: &State
) -> Result<Task, TaskError> {
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

            if let Some(_) = spec.query_param(param_keys::REFRESH_REMOTE_URLS) {
                flags |= REFRESH_REMOTE_URLS;
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
                        // TODO `NonEmptyString` type so we don't need to do this
                        // as manually/carefully. Or, consider removing some Option
                        // wrappings.
                        if v.is_empty() {
                            continue;
                        }

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

                    let path = LocalFeedPath::new(
                        path,
                        &state.local_feeds_dir
                    ).map_err(|e|
                        TaskError(format!("{e}"))
                    )?;

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
        (Method::Get, page_names::REMOTE_ADD) => {
            Ok(ShowRemoteFeedAddForm)
        },
        (Method::Post, page_names::REMOTE_ADD) => {
            spec.remote_feed_add_form()
                .map_err(|e| TaskError(e.to_string()))
                .and_then(|pairs| {
                    let mut url = String::new();

                    for (k, v) in pairs {
                        if v.is_empty() {
                            continue;
                        }

                        match k.as_str() {
                            form_names::FEED_URL => {
                                url = v;
                            }
                            _ => {
                                return Err(TaskError(format!(
                                    "Unhandled Form pair ({k}, {v})"
                                )))
                            }
                        }
                    }

                    Ok(SubmitRemoteFeedAddForm(RemoteFeedAddForm {
                        url,
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
    MissingLocalFile,
    UrlParse(fetch::UrlParseError),
}

impl core::fmt::Display for PerformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Fetch(e) => write!(f, "{e}"),
            Self::Render(e) => write!(f, "{e}"),
            Self::MissingLocalFile => write!(f, "Local file did not exist"),
            Self::UrlParse(e) => write!(f, "{e}"),
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

impl From<RemoteFeedUrlsError> for PerformError {
    fn from(e: RemoteFeedUrlsError) -> Self {
        use RemoteFeedUrlsError as E;
        match e {
            E::Io(e) => Self::Io(e),
            E::UrlParse(e) => Self::UrlParse(e),
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

        macro_rules! data {
            () => {
                &Data {
                    root: &self.root,
                    local_posts: &self.local_posts,
                    remote_posts: &self.remote_posts,
                    remote_feeds: &self.remote_feeds,
                }
            }
        }

        match task {
            ShowHomePage(flags) => {
                if flags & REFRESH_LOCAL != 0 {
                    load_local_posts(
                        &mut self.local_posts,
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

                if flags & REFRESH_REMOTE_URLS != 0 {
                    let mut remote_feeds_file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(self.root.path_to(REMOTE_FEEDS))?;

                    load_remote_feed_urls(
                        &mut remote_feeds_file,
                        &mut self.remote_feeds,
                        self.utc_offset,
                    )?;
                }

                render::home_page(
                    &mut output,
                    data!(),
                )?;
            },
            ShowLocalAddForm => {
                load_local_feed_paths(
                    &mut self.local_posts,
                    &self.local_feeds_dir
                )?;

                render::local_add_form(
                    &mut output,
                    self.local_posts
                        .keys()
                        .map(|path| Target {
                            path: path.as_ref(),
                            root: &self.root,
                        }),
                    data!(),
                    Option::<(
                        render::LocalAddForm<'_, '_, '_, '_, Target<'_, '_>, String>,
                        &str
                    )>::None,
                )?;
            }
            SubmitLocalAddForm(form) => {
                match add_local_post(
                    self.local_posts
                        .get_mut(&form.path)
                        .ok_or(PerformError::MissingLocalFile)?,
                    form,
                    self.utc_offset,
                ) {
                    Ok(()) => {
                        render::local_add_form_success(&mut output)?
                    }
                    Err((form, e)) => {
                        render::local_add_form(
                            &mut output,
                            self.local_posts
                                .keys()
                                .map(|path| Target {
                                    path: path.as_ref(),
                                    root: &self.root,
                                }),
                            data!(),
                            Some((
                                render::LocalAddForm {
                                    target: Target{
                                        path: form.path.as_ref(),
                                        root: &self.root,
                                    },
                                    title: form.post.title.as_ref()
                                        .map(|s| s.as_str())
                                        .unwrap_or_default(),
                                    summary: form.post.summary.as_ref()
                                        .map(|s| s.as_str())
                                        .unwrap_or_default(),
                                    content: form.post.content.as_ref()
                                        .map(|s| s.as_str())
                                        .unwrap_or_default(),
                                    links: &form.post.links,
                                },
                                &e.to_string()
                            ))
                        )?;
                    }
                }
            }
            ShowRemoteFeedAddForm => {
                render::remote_feed_add_form(
                    &mut output,
                    data!(),
                    Option::<(render::RemoteFeedAddForm<'_>, &str)>::None,
                )?;
            }
            SubmitRemoteFeedAddForm(form) => {
                match add_remote_feed(
                    &mut self.remote_feeds,
                    form,
                    &self.root,
                    self.utc_offset,
                ) {
                    Ok(()) => render::remote_feed_add_form_success(&mut output)?,
                    Err((form, e)) => {
                        render::remote_feed_add_form(
                            &mut output,
                            data!(),
                            Some((
                                render::RemoteFeedAddForm {
                                    url: form.url.as_ref()
                                },
                                &e.to_string()
                            ))
                        )?;
                    }
                }
            }
        }

        Ok(output)
    }
}

fn add_local_post(
    posts: &mut Posts,
    form: LocalAddForm,
    utc_offset: UtcOffset,
) -> Result<(), (LocalAddForm, Box<dyn std::error::Error>)> {
    // `q` is short for "question mark" since this is like `?`.
    macro_rules! q {
        ($expr: expr) => {
            match $expr {
                Ok(thing) => thing,
                Err(e) => return Err((form, Box::from(e))),
            }
        }
    }

    {
        let buffer = q!(std::fs::read_to_string(&form.path));

        q!(write_atomically::write_atomically(
            &form.path,
            |file| syndicated::add_post(
                file,
                std::io::Cursor::new(&buffer),
                form.post.clone(),
            )
        ));
    }

    let buffer = q!(std::fs::read_to_string(&form.path));

    load_local_post_from_buffer(
        posts,
        std::io::Cursor::new(&buffer),
        utc_offset,
    );

    Ok(())
}

fn add_remote_feed(
    remote_feeds: &mut RemoteFeeds,
    form: RemoteFeedAddForm,
    root: &Root,
    utc_offset: UtcOffset,
) -> Result<(), (RemoteFeedAddForm, Box<dyn std::error::Error>)> {
    use std::io::{Read, Seek, Write};

    // `q` is short for "question mark" since this is like `?`.
    macro_rules! q {
        ($expr: expr) => {
            match $expr {
                Ok(thing) => thing,
                Err(e) => return Err((form, Box::from(e))),
            }
        }
    }

    let url = q!(Url::parse(form.url.as_ref()));

    let mut remote_feeds_file = q!(std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .append(true)
        .open(root.path_to(REMOTE_FEEDS)));

    // Read the last byte of the file, if there is one, and if it is not a newline
    // character, add one.
    let end = q!(remote_feeds_file.seek(std::io::SeekFrom::End(0)));
    if end != 0 {
        q!(remote_feeds_file.seek(std::io::SeekFrom::End(-1)));

        let byte = &mut [0];
        q!(remote_feeds_file.read(byte));

        if byte[0] != b'\n' {
            q!(writeln!(remote_feeds_file));
        }
    }

    q!(writeln!(remote_feeds_file, "{url}"));

    q!(remote_feeds_file.flush());

    q!(remote_feeds_file.seek(std::io::SeekFrom::Start(0)));

    load_remote_feed_urls(
        &mut remote_feeds_file,
        remote_feeds,
        utc_offset,
    ).map_err(|e| (form, Box::from(e)))
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
    local_posts: &'posts LocalPosts,
    remote_posts: &'posts Posts,
    remote_feeds: &'posts RemoteFeeds,
}

impl <'root> render::RootDisplay for Data<'root, '_> {
    type RootDisplay = std::path::Display<'root>;

    fn root_display(&self) -> Self::RootDisplay {
        self.root.display()
    }
}

type Section<'a> = render::Section<
    std::slice::Iter<'a, syndicated::Post>,
    Timestamp
>;

impl <'posts> render::Data<'_> for Data<'_, 'posts> {
    type Link = String;
    type PostHolder = &'posts Post;
    type Posts = std::slice::Iter<'posts, Post>;
    type RefreshTimestamps = std::array::IntoIter<
        render::RefreshTimestamp<Self::Timestamp>,
        3
    >;
    type Sections = Box<dyn Iterator<Item = Section<'posts>> + 'posts>;
    type Timestamp = Timestamp;

    fn post_sections(&self) -> Self::Sections {
        Box::new(
            self.local_posts.values()
            .map(to_local_section)
            .chain(std::iter::once(render::Section {
                kind: render::SectionKind::Remote,
                posts: self.remote_posts.posts.iter(),
                timestamp: self.remote_posts.fetched_at,
            }))
        )
    }

    fn refresh_timestamps(&self) -> Self::RefreshTimestamps {
        [
            render::RefreshTimestamp {
                kind: render::RefreshKind::Local,
                timestamp: self.local_posts.values()
                    .fold(Timestamp::MAX, |acc, post|
                        if acc > post.fetched_at {
                            post.fetched_at
                        } else {
                            acc
                        }
                    ),
            },
            render::RefreshTimestamp {
                kind: render::RefreshKind::Remote,
                timestamp: self.remote_posts.fetched_at,
            },
            render::RefreshTimestamp {
                kind: render::RefreshKind::RemoteUrls,
                timestamp: self.remote_feeds.fetched_at,
            },
        ].into_iter()
    }
}

fn to_local_section<'posts>(posts: &'posts Posts) -> Section {
    render::Section {
        kind: render::SectionKind::Local,
        posts: posts.posts.iter(),
        timestamp: posts.fetched_at,
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

#[derive(Debug)]
pub enum RemoteFeedUrlsError {
    Io(std::io::Error),
    UrlParse(fetch::UrlParseError),
}

impl core::fmt::Display for RemoteFeedUrlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::UrlParse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RemoteFeedUrlsError {}

fn load_remote_feed_urls(
    remote_feeds_file: &mut File,
    remote_feeds: &mut RemoteFeeds,
    utc_offset: UtcOffset,
) -> Result<(), RemoteFeedUrlsError> {
    use RemoteFeedUrlsError as E;
    let mut remote_feeds_string = String::with_capacity(1024);

    std::io::Read::read_to_string(
        remote_feeds_file,
        &mut remote_feeds_string,
    ).map_err(E::Io)?;

    remote_feeds.fetched_at = Timestamp::now_at_offset(utc_offset);

    remote_feeds.feeds.clear();

    for line in remote_feeds_string.lines() {
        remote_feeds.feeds.push(
            Url::parse(line)
                .map_err(E::UrlParse)?
        );
    }

    Ok(())
}