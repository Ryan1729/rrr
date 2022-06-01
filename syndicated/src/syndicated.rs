use atom_syndication::Feed;
use rss::Channel;
use digest::Digest;
use meowhash::MeowHasher;

#[derive(Clone, Debug)]
pub struct Post {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    // TODO parsed URLs?
    pub links: Vec<String>,
}

impl render::PostHolder<String> for &Post {
    fn get_post(&self) -> render::Post<'_, String> {
        render::Post {
            title: self.title.as_deref(),
            summary: self.summary.as_deref(),
            content: self.content.as_deref(),
            links: &self.links,
        }
    }
}

pub type AddError = atom_syndication::Error;

pub fn add_post(
    write: impl std::io::Write,
    mut buf_read: impl std::io::BufRead,
    post: Post,
) -> Result<(), AddError> {
    // Could support RSS here, but for our current purposes Atom is sufficent.

    let mut feed = Feed::read_from(&mut buf_read)?;

    use atom_syndication::{Content, Entry, FixedDateTime, Link, Text};

    let mut entry = Entry::default();

    let mut hasher = MeowHasher::new();

    let now: FixedDateTime = chrono::offset::Local::now().into();

    // I don't think we need to bother hashing the offset.
    hasher.update(now.timestamp().to_le_bytes());
    hasher.update(now.timestamp_subsec_nanos().to_le_bytes());
    
    entry.updated = now;
    entry.published = Some(now);

    let title = post.title.unwrap_or_else(|| now.to_rfc3339());

    hasher.update(&title);
    for content in post.content.as_ref() {
        hasher.update(content);
    }
    for summary in post.summary.as_ref() {
        hasher.update(summary);
    }
    for link in &post.links {
        hasher.update(link);
    }

    entry.title = title.into();
    entry.content = post.content.map(|value| {
        let mut content = Content::default();

        content.set_value(value);

        content
    });
    entry.summary = post.summary.map(|value| {
        let mut summary = Text::default();

        summary.value = value;

        summary
    });
    entry.links = post.links.into_iter().map(|href| {
        let mut link = Link::default();

        link.href = href;

        link
    }).collect();

    let id = hasher.finalise().as_u128();

    // If I need to add some different prefix to make this into a valid URN or 
    // whatever later, then I can probably just update the feeds at that time.
    entry.id = format!("mh:{id:X}");

    feed.entries.push(entry);

    feed.write_to(write).map(|_| ())
}

pub fn parse_items(
    mut buf_read: impl std::io::BufRead + std::io::Seek,
    output: &mut Vec<Post>,
) {
    if let Ok(feed) = Feed::read_from(&mut buf_read) {
        for entry in feed.entries {
            output.push(Post {
                title: Some(entry.title.value),
                summary: entry.summary.map(|s| s.value),
                content: entry.content.and_then(|c| c.value),
                links: entry.links.into_iter().map(|l| l.href).collect(),
            });
        }

        return
    }

    let res = buf_read.seek(std::io::SeekFrom::Start(0));
    // I think this will not fail in practice
    debug_assert!(res.is_ok());

    if let Ok(channel) = Channel::read_from(&mut buf_read) {
        for item in channel.items {
            output.push(Post {
                title: item.title,
                summary: item.description,
                content: item.content,
                links: item.link.into_iter().collect(),
            });
        }

        return
    }
}
