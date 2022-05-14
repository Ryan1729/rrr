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

impl State {
    pub fn perform(&mut self, task: Task) -> Output {
        use Task::*;

        match task {
            ShowHomePage => {
                main_template(
                    &self.root.display().to_string()
                )
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