// We ant to control the allocation usage of this crate. So, we mark it
// as #![no_std], and don't pull in alloc directly. Instead, we expose
// traits so other crates can expose only what we need to use here.
#![no_std]

use core::fmt::Display;

pub type Error = core::fmt::Error;

pub type Result<A = ()> = core::result::Result<A, Error>;

/// A way to incrementally output HTML.
pub trait Output: core::fmt::Write {}

pub enum RefreshKind {
    Local,
    Remote,
}

pub struct RefreshTimestamp<Timestamp> {
    pub kind: RefreshKind,
    pub timestamp: Timestamp,
}

pub enum SectionKind {
    Local,
    Remote,
}

pub struct Section<Posts> {
    pub kind: SectionKind,
    pub posts: Posts,
}

/// This may be an overly naive representation. But it seems best to go with the
/// simplest option that works for the feeds I want to read. We can extend/improve
/// this as needed.
pub struct Post<'post, Link> {
    pub title: Option<&'post str>,
    pub summary: Option<&'post str>,
    pub content: Option<&'post str>,
    pub links: &'post [Link],
}

/// A way to access a `Post` which may or may not ultimately own it.
pub trait PostHolder
where
    Self::Link: AsRef<str>,
    Self::Source: Display,
{
    type Link;
    type Source;

    fn get_post(&self) -> Post<'_, Self::Link>;

    fn source(&self) -> Self::Source;
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
    Self::PostHolder: PostHolder,
    Self::Posts: Iterator<Item = Self::PostHolder>,
    Self::RefreshTimestamps: Iterator<Item = RefreshTimestamp<Self::Timestamp>>,
    Self::Sections: Iterator<Item = Section<Self::Posts>>,
    Self::Timestamp: Display,
{
    type PostHolder;
    type Posts;
    type RefreshTimestamps;
    type Sections;
    type Timestamp;

    fn post_sections(&self) -> Self::Sections;

    fn refresh_timestamps(&self) -> Self::RefreshTimestamps;
}

fn controls<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    use RefreshKind::*;

    for r_t in data.refresh_timestamps() {
        let (label, refresh_key) = match r_t.kind {
            Local => ("Refresh Local Posts", REFRESH_LOCAL),
            Remote => ("Refresh Remote Posts", REFRESH_REMOTE),
        };
        let timestamp = r_t.timestamp;

        write!(
            output,
            "\
            <form>\
              <button \
                type='submit' \
                title='Fresh as of {timestamp} (taking oldest)'\
              >\
                {label}\
              </button>\
              <input type='hidden' name='{refresh_key}'>\
            </form>\
            "
        )?;
    }

    write!(output, "<div><a href='{LOCAL_ADD}'>Add local entry</a></div>")?;
    write!(output, "<div><a href='{REMOTE_ADD}'>Add remote feed</a></div>")
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
            let source = post.source();
            write!(output, "#{letter}{i} &ndash; <small>{source}</small>")?;

            let post = post.get_post();

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
            // TODO reduce duplication with similar style tags, if this gets
            // copy-pasted again?
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

pub struct RemoteFeedAddForm<'url> {
    pub url: &'url str,
}

pub fn remote_feed_add_form<'url>(
    output: &mut impl Output,
    root_display: &impl RootDisplay,
    previous: Option<(
        RemoteFeedAddForm<'url>,
        &str
    )>,
) -> Result {
    main_template(
        output,
        |o| {
            // TODO move this to the head tag, if it matters later, I guess?
            // The perf difference if any, doesn't seem significant.
            // TODO reduce duplication with similar style tags, if this gets
            // copy-pasted again?
            write!(
                o,
                "<style>\
                     form {{ display: table; }}\
                        p {{ display: table-row; }}\
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

            let url = form.map(|form| form.url).unwrap_or_default();

            write!(
                o,
                "\
                <p>\
                    <label for='{FEED_URL}'>Feed URL</label>\
                    <input \
                        name='{FEED_URL}' id='{FEED_URL}' size=128 value='{url}'\
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
        |o| write!(o, "Successfully added local post")
    )
}

pub fn remote_feed_add_form_success(
    output: &mut impl Output,
) -> Result {
    main_template(
        output,
        |o| write!(o, "Successfully added remote feed")
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
    pub const REFRESH_REMOTE_URLS: &str = "refresh-remote-urls";
}
use param_keys::*;

/// Names for pages; AKA parts of URLs.
pub mod page_names {
    pub const LOCAL_ADD: &str = "/local-add";
    pub const REMOTE_ADD: &str = "/remote-add";
}
use page_names::*;

/// Form element names.
pub mod form_names {
    pub const TARGET: &str = "target";

    pub const TITLE: &str = "title";
    pub const SUMMARY: &str = "summary";
    pub const CONTENT: &str = "content";
    pub const LINK: &str = "link";

    pub const FEED_URL: &str = "feed-url";
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

