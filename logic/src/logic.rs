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

pub type RemoteFeeds = Vec<Url>;

pub struct State {
    root: Root,
    // We plan to re-fetch from the feeds during runtime, so we'll want to avoid
    // re-parsing.
    #[allow(unused)]
    remote_feeds: RemoteFeeds,
    posts: Vec<syndicated::Post>,
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

        for feed in &remote_feeds {
            use std::io::Read;
            let mut reader = fetch::get(feed)
                .map_err(Self::Error::Fetch)?;

            let mut buffer = String::with_capacity(4096);
            reader.read_to_string(&mut buffer)
                .map_err(Self::Error::Io)?;

            syndicated::parse_items(
                std::io::Cursor::new(&buffer),
                &mut posts,
            );
        }

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

pub enum Task {
    ShowHomePage
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

impl State {
    pub fn perform(&mut self, task: Task) -> render::Result<Output> {
        use Task::*;

        match task {
            ShowHomePage => {
                // 64k ought to be enough for anybody!
                let mut output = Output::Html(String::with_capacity(65536));

                render::home_page(
                    &mut output,
                    &Data { root: &self.root, posts: &self.posts }
                )?;

                Ok(output)
            }
        }
    }
}