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

pub enum Task {
    ShowHomePage
}

impl State {
    pub fn perform(&mut self, task: Task) -> Output {
        use Task::*;

        match task {
            ShowHomePage => {
                let feeds = render_feeds(&self);

                main_template(&format!(
                    "{feeds}\
                     <footer>{}</footer>",
                    &self.root.display().to_string()
                ))
            }
        }
    }
}

fn main_template(body: &str) -> Output {
    // Many tags can officially be omitted. If the browsers display it properly, why
    // send extra bytes?
    // See https://html.spec.whatwg.org/multipage/syntax.html#syntax-tag-omission
    const HEADER: &str = "\
        <!DOCTYPE HTML>\
        <style> body { color: #eee; background-color: #222 } </style>
        <title>RRR</title>\n\
    ";

    let mut output = String::with_capacity(HEADER.len() + body.len());

    output.push_str(HEADER);
    output.push_str(body);

    Output::Html(
        output
    )
}

// Might make this a struct that impls Display or something later.
type Feeds = String;

fn render_feeds(state: &State) -> Feeds {
    fn inner(state: &State) -> std::io::Result<Feeds> {
        let mut output = Feeds::with_capacity(1024);

        // TODO fetch feed (elsewhere), parse content, render it.
        for line in state.remote_feeds.lines() {
            output.push_str("<a href=");
            output.push_str(line);
            output.push_str(">");
            output.push_str(line);
            output.push_str("</a>");
        }

        Ok(output)
    }
    match inner(state) {
        Ok(v) => v,
        Err(e) => e.to_string(),
    }
}