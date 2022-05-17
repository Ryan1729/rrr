use std::path::PathBuf;

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

// TODO struct containing a collection of parsed feeds.
pub type RemoteFeeds = String;

pub struct State {
    root: Root,
    remote_feeds: RemoteFeeds,
}

impl render::Data for State {
    type Error = String;
    type RootDisplay = String;

    fn feeds(&self) -> Result<&str, Self::Error> {
        // We expect we might have a parse error here instead or something.

        Ok(&self.remote_feeds)
    }

    fn root_display(&self) -> Self::RootDisplay {
        format!("{}", self.root.display())
    }
}

#[derive(Debug)]
pub enum StateCreationError {
    RootMustBeDir,
    Io(std::io::Error)
}

impl core::fmt::Display for StateCreationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RootMustBeDir => write!(f, "Root dir must be a dir"),
            Self::Io(e) => write!(f, "{e}"),
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

        let mut remote_feeds = String::with_capacity(1024);

        std::io::Read::read_to_string(
            &mut remote_feeds_file,
            &mut remote_feeds
        ).map_err(Self::Error::Io)?;

        Ok(Self {
            root,
            remote_feeds,
        })
    }
}

pub enum Output {
    Html(String),
}

impl core::fmt::Write for Output {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self {
            Self::Html(ref mut output) => {
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

impl State {
    pub fn perform(&mut self, task: Task) -> Output {
        use Task::*;

        match task {
            ShowHomePage => {
                // 64k ought to be enough for anybody!
                let mut output = Output::Html(String::with_capacity(65536));

                render::home_page(&mut output, self);

                output
            }
        }
    }
}