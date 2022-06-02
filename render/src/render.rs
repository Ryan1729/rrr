// We ant to control the allocation usage of this crate. So, we mark it
// as #![no_std], and don't pull in alloc directly. Instead, we expose
// traits so other crates can expose only what we need to use here.
#![no_std]

use core::fmt::Display;

pub type Error = core::fmt::Error;

pub type Result<A = ()> = core::result::Result<A, Error>;

/// A way to incrementally output HTML.
pub trait Output: core::fmt::Write {}

pub enum SectionKind {
    Local,
    Remote,
}

pub struct Section<Posts, Timestamp> {
    pub kind: SectionKind,
    pub posts: Posts,
    pub timestamp: Timestamp,
}

/// A way to access a `Post` which may or may not ultimately own it.
pub trait PostHolder<Link>
where
    Link: AsRef<str>
{
    fn get_post(&self) -> Post<'_, Link>;
}

pub trait RootDisplay
where
    Self::RootDisplay: Display,
{
    type RootDisplay;

    fn root_display(&self) -> Self::RootDisplay;
}

/// A way to access the data we need for rendering.
pub trait Data<'posts>: RootDisplay
where
    Self::Link: AsRef<str> + 'posts,
    Self::PostHolder: PostHolder<Self::Link>,
    Self::Posts: Iterator<Item = Self::PostHolder>,
    Self::Sections: Iterator<Item = Section<Self::Posts, Self::Timestamp>>,
    Self::Timestamp: Display,
{
    type Link;
    type PostHolder;
    type Posts;
    type Sections;
    type Timestamp;

    fn post_sections(&self) -> Self::Sections;
}

/// This may be an overly naive representation. But it seems best to go with the
/// simplest option that works for the feeds I want to read. We can extend/improve
/// this as needed.
pub struct Post<'post, Link> {
    pub title: Option<&'post str>,
    pub summary: Option<&'post str>,
    pub content: Option<&'post str>,
    pub links: &'post [Link]
}

fn controls<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    use SectionKind::*;
    for section in data.post_sections() {
        let (label, refresh_key) = match section.kind {
            Local => ("Refresh Local Posts", REFRESH_LOCAL),
            Remote => ("Refresh Remote Posts", REFRESH_REMOTE),
        };
        let timestamp = section.timestamp;

        write!(
            output,
            "\
            <form>\
              <button \
                type='submit' \
                title='Fresh as of {timestamp}'\
              >\
                {label}\
              </button>\
              <input type='hidden' name='{refresh_key}'>\
            </form>\
            "
        )?;
    }

    write!(output, "<a href='{LOCAL_ADD}'>Add local entry</a>")
}

fn feeds<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    use SectionKind::*;

    for section in data.post_sections() {
        let (name, letter) = match section.kind {
            Local => ("local posts", 'L'),
            Remote => ("remote posts", 'R'),
        };

        write!(
            output,
            "<details>\
                <summary>{name}</summary>\
            "
        )?;

        for (i, post) in section.posts.enumerate() {
            let post = post.get_post();
            write!(output, "<p>#{letter}{i}")?;

            let mut links = post.links;

            if let Some(title) = post.title {
                if let Some(link) = links.get(0) {
                    let link = link.as_ref();
                    write!(output, "<h2><a href=\"{link}\">{title}</a></h2>")?;
                    links = &links[1..];
                } else {
                    write!(output, "<h2>{title}</h2>")?;
                }
            }

            if let Some(summary) = post.summary {
                write!(output, "<h3>{summary}</h3>")?;
            }

            if let Some(content) = post.content {
                write!(output, "<p>{content}</p>")?;
            }

            for (i, link) in links.iter().enumerate() {
                let link = link.as_ref();
                write!(output, "<a href=\"{link}\">{}</a>", i + 1)?;
            }

            write!(output, "</p>")?;
        }

        write!(output, "</details>")?;
    }

    Ok(())
}

pub fn home_page<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    main_template(
        output,
        |o| {
            controls(o, data)?;

            feeds(o, data)?;

            footer(o, data)
        }
    )
}

pub trait Path
where
    Self::Display: Display
{
    type Display;

    fn display() -> Self::Display;
}

pub trait Target: PartialEq + Eq
where
    Self::Label: Display,
    Self::Value: Display,
{
    type Label;
    type Value;

    fn label(&self) -> Self::Label;
    fn value(&self) -> Self::Value;
}

pub struct LocalAddForm<
    'title,
    'summary,
    'content,
    'links,
    Target,
    S,
> {
    pub target: Target,
    pub title: &'title str,
    pub summary: &'summary str,
    pub content: &'content str,
    pub links: &'links [S],
}

pub fn local_add_form<
    'title,
    'summary,
    'content,
    'links,
    Trget: Target,
    S: AsRef<str>,
>(
    output: &mut impl Output,
    local_add_targets: impl Iterator<Item = Trget>,
    root_display: &impl RootDisplay,
    previous: Option<(
        LocalAddForm<
            'title,
            'summary,
            'content,
            'links,
            Trget,
            S,
        >,
        &str
    )>,
) -> Result {
    main_template(
        output,
        |o| {
            // TODO move this to the head tag, if it matters later, I guess?
            // The perf difference if any, doesn't seem significant.
            write!(
                o,
                "<style>\
                     form {{ display: table; }}\
                        p {{ display: table-row; }}\
                   select {{ display: table-cell; }}\
                    label {{ display: table-cell; text-align: right }}\
                    input {{ display: table-cell; }}\
                </style>"
            )?;

            write!(
                o,
                "\
                <form>"
            )?;

            if let Some((_, error_message)) = &previous {
                write!(o, "{error_message}")?;
            }

            let form = previous.map(|(form, _)| form);

            write!(
                o,
                "<p>\
                    <label for='{TARGET}'>Target file</label>\
                    <select name='{TARGET}'>"
            )?;

            for target in local_add_targets {
                let selected = if
                    Some(&target)
                    == form.as_ref().map(|form| &form.target) {
                    "selected"
                } else {
                    ""
                };

                write!(
                    o,
                    "<option value='{value}' {selected}>{label}</option>",
                    value = target.value(),
                    label = target.label(),
                )?;
            }

            let (title, summary, content, link_2, link_1) = form.map(|form| (
               form.title,
               form.summary,
               form.content,
               form.links.get(1).map(|s| s.as_ref()).unwrap_or_default(),
               form.links.get(0).map(|s| s.as_ref()).unwrap_or_default(),
            )).unwrap_or_default();

            write!(
                o,
                "\
                    </select>\
                </p>\
                <p>\
                    <label for='{TITLE}'>Title</label>\
                    <input \
                        name='{TITLE}' id='{TITLE}' size=128 value='{title}'\
                    >\
                </p>\
                <p>\
                    <label for='{SUMMARY}'>Summary</label>\
                    <input \
                        name='{SUMMARY}' id='{SUMMARY}' size=128 value='{summary}'\
                    >\
                </p>\
                <p>\
                    <label for='{CONTENT}'>Content</label>\
                    <textarea name='{CONTENT}' id='{CONTENT}' rows=5 cols=128>\
                        {content}\
                    </textarea>\
                </p>\
                <p>\
                    <label for='{LINK}1'>Link</label>\
                    <input \
                        type='url' \
                        id='{LINK}1' name='{LINK}' size=128 value='{link_1}'\
                    >\
                </p>\
                <p>\
                    <label for='{LINK}2'>Link</label>\
                    <input \
                        type='url' \
                        id='{LINK}2' name='{LINK}' size=128 value='{link_2}'\
                    >\
                </p>\
                <p>\
                    <label for='submit'></label>\
                    <input type='submit' id='submit' formmethod='post'>\
                </p>\
            </form>"
            )?;

            footer(o, root_display)
        }
    )
}

pub fn local_add_form_success(
    output: &mut impl Output,
) -> Result {
    main_template(
        output,
        |o| {
            write!(o, "Successfully added local post")
        }
    )
}

fn footer<'data>(
    o: &mut impl Output,
    root_display: &impl RootDisplay
) -> Result {
    write!(
        o,
        "<footer>{}</footer>",
        root_display.root_display()
    )
}

/// URL param keys.
pub mod param_keys {
    pub const REFRESH_LOCAL: &str = "refresh-local";
    pub const REFRESH_REMOTE: &str = "refresh-remote";
}
use param_keys::*;

/// Names for pages; AKA parts of URLs.
pub mod page_names {
    pub const LOCAL_ADD: &str = "/local-add";
}
use page_names::*;

/// Form element names.
pub mod form_names {
    pub const TARGET: &str = "target";

    pub const TITLE: &str = "title";
    pub const SUMMARY: &str = "summary";
    pub const CONTENT: &str = "content";
    pub const LINK: &str = "link";
}
use form_names::*;

fn main_template<O>(
    output: &mut O,
    body: impl FnOnce(&mut O) -> Result,
) -> Result
where O: Output
{
    // Many tags can officially be omitted. If the browsers display it properly, why
    // send extra bytes?
    // See https://html.spec.whatwg.org/multipage/syntax.html#syntax-tag-omission
    const HEADER: &str = "\
        <!DOCTYPE HTML>\
        <style> * { color: #eee; background-color: #222 } </style>
        <title>RRR</title>\n\
    ";

    output.write_str(HEADER)?;

    body(output)?;

    // TODO show estimated render time on floating thing that is hidden until you
    // hover over a small thing

    Ok(())
}

