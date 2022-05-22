// We ant to control the allocation usage of this crate. So, we mark it
// as #![no_std], and don't pull in alloc directly. Instead, we expose
// traits so other crates can expose only what we need to use here.
#![no_std]

use core::fmt::Display;

pub type Error = core::fmt::Error;

pub type Result<A = ()> = core::result::Result<A, Error>;

/// A way to incrementally output HTML.
pub trait Output: core::fmt::Write {}

/// A way to access a `Post` which may or may not ultimately own it.
pub trait PostHolder<Link>
where
    Link: AsRef<str>
{
    fn get_post(&self) -> Post<'_, Link>;
}

/// A way to access the data we need for rendering.
pub trait Data<'posts>
where
    Self::RootDisplay: Display,
    Self::PostHolder: PostHolder<Self::Link>,
    Self::Link: AsRef<str>,
{
    type RootDisplay;
    type PostHolder;
    type Link;

    fn posts(&self) -> &[Self::PostHolder];

    fn root_display(&self) -> Self::RootDisplay;
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

fn feeds<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    for (i, post) in data.posts().iter().enumerate() {
        let post = post.get_post();
        write!(output, "<p>#{i}")?;

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

    Ok(())
}

pub fn home_page<'data>(
    output: &mut impl Output,
    data: &impl Data<'data>
) -> Result {
    main_template(
        output,
        |o| {
            feeds(o, data)?;

            write!(
                o,
                "<footer>{}</footer>",
                data.root_display()
            )
        }
    )
}

pub mod keys {
    pub const REFRESH: &str = "refresh";
}
use keys::*;

fn main_template<O>(
    output: &mut O,
    body: impl FnOnce(&mut O) -> Result
) -> Result
where O: Output
{
    // Many tags can officially be omitted. If the browsers display it properly, why
    // send extra bytes?
    // See https://html.spec.whatwg.org/multipage/syntax.html#syntax-tag-omission
    const HEADER: &str = "\
        <!DOCTYPE HTML>\
        <style> body { color: #eee; background-color: #222 } </style>
        <title>RRR</title>\n\
    ";

    output.write_str(HEADER)?;

    write!(
        output,
        "\
        <form>\
          <div>\
            <button type='submit'>Refresh</button>\
          </div>\
          <input type='hidden' name='{REFRESH}'>\
        </form>\
        "
    )?;

    body(output)?;

    // TODO show estimated render time on floating thing that is hidden until you
    // hover over a small thing

    Ok(())
}

