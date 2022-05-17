// We ant to control the allocation usage of this crate. So, we mark it
// as #![no_std], and don't pull in alloc directly. Instead, we expose
// traits so other crates can expose only what we need to use here.
#![no_std]

use core::fmt::Display;

/// A way to incrementally output HTML.
pub trait Output: core::fmt::Write {}

/// A way to access the data we need for rendering.
pub trait Data
where
    Self::Error: Display,
    Self::RootDisplay: Display,
 {
    type Error;
    type RootDisplay;

    fn feeds(&self) -> Result<&str, Self::Error>;

    fn root_display(&self) -> Self::RootDisplay;
}

pub fn home_page(output: &mut impl Output, data: &impl Data) {
    main_template(
        output,
        |o| {
            feeds(o, data);

            write!(
                o,
                "<footer>{}</footer>",
                data.root_display()
            );
        }
    );
}

fn main_template<O>(output: &mut O, body: impl FnOnce(&mut O))
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

    output.write_str(HEADER);

    body(output);
}

pub fn feeds(output: &mut impl Output, data: &impl Data) {
    match data.feeds() {
        Ok(feeds) => {
            // TODO fetch feed (elsewhere), parse content, render it.
            for line in feeds.lines() {
                write!(output, "<a href={line}>{line}</a>");
            }
        },
        Err(e) => {
            write!(output, "{e}");
        },
    }
}



