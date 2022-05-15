use std::path::PathBuf;

pub struct State {
    root: PathBuf,
}

pub struct MustBeDirError();

impl TryFrom<PathBuf> for State {
    type Error = MustBeDirError;

    fn try_from(root: PathBuf) -> Result<Self, Self::Error> {
        if !root.is_dir() {
            Err(MustBeDirError())
        } else {
            Ok(Self {
                root
            })
        }
    }
}

pub enum Output {
    Html(String),
}

pub enum Task {
    ShowHomePage
}

const REMOTE_FEEDS: &str = "remote-feeds";

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

impl State {
    fn path_to(&self, file_name: impl AsRef<std::path::Path>) -> PathBuf {
        self.root.join(file_name)
    }
}

// Might make this a struct that impls Display or something later.
type Feeds = String;

fn render_feeds(state: &State) -> Feeds {
    fn inner(state: &State) -> std::io::Result<Feeds> {
        let mut output = Feeds::with_capacity(1024);
        // TODO move I/O out of this module
        let remote_feeds = std::fs::read_to_string(state.path_to(REMOTE_FEEDS))?;

        // TODO fetch feed (elsewhere), parse content render it.
        for line in remote_feeds.lines() {
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