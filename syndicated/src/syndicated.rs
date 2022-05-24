use atom_syndication::Feed;
use rss::Channel;

#[derive(Debug)]
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
