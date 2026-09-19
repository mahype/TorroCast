//! OPML: how podcast clients hand their subscriptions to one another.
//! Reading is forgiving — every client writes it a little differently —
//! and writing is plain.

use roxmltree::{Document, Node, ParsingOptions};

use crate::{FeedError, entities};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    pub title: String,
    pub feed_url: String,
}

fn attribute<'a>(node: Node<'a, '_>, names: &[&str]) -> Option<&'a str> {
    // Attribute names come in every capitalisation: xmlUrl, xmlurl, XMLURL.
    node.attributes()
        .find(|attribute| names.iter().any(|name| attribute.name().eq_ignore_ascii_case(name)))
        .map(|attribute| attribute.value().trim())
        .filter(|value| !value.is_empty())
}

/// Every feed in the file, folders opened up, each address once.
pub fn parse(text: &str) -> Result<Vec<Outline>, FeedError> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let options = ParsingOptions { allow_dtd: true, ..ParsingOptions::default() };
    let repaired;
    let document = match Document::parse_with_options(text, options) {
        Ok(document) => document,
        Err(first) => {
            repaired = entities::repair_xml(text);
            Document::parse_with_options(&repaired, options).map_err(|_| FeedError::Malformed(first.to_string()))?
        }
    };
    if !document.root_element().tag_name().name().eq_ignore_ascii_case("opml") {
        return Err(FeedError::NotAFeed);
    }
    let mut outlines: Vec<Outline> = Vec::new();
    for node in document.descendants().filter(|node| node.tag_name().name().eq_ignore_ascii_case("outline")) {
        let Some(feed_url) = attribute(node, &["xmlUrl", "url"]).filter(|url| url.starts_with("http")) else {
            continue;
        };
        if outlines.iter().any(|known| known.feed_url.eq_ignore_ascii_case(feed_url)) {
            continue;
        }
        let title = attribute(node, &["title", "text"]).unwrap_or(feed_url);
        outlines.push(Outline { title: entities::decode(title), feed_url: feed_url.to_owned() });
    }
    Ok(outlines)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// An OPML 2.0 file any podcast client can read.
#[must_use]
pub fn write(title: &str, outlines: &[Outline]) -> String {
    let mut text = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head>\n    <title>{}</title>\n  </head>\n  <body>\n",
        escape(title)
    );
    for outline in outlines {
        let (name, url) = (escape(&outline.title), escape(&outline.feed_url));
        text.push_str(&format!("    <outline type=\"rss\" text=\"{name}\" title=\"{name}\" xmlUrl=\"{url}\"/>\n"));
    }
    text.push_str("  </body>\n</opml>\n");
    text
}

#[cfg(test)]
mod tests {
    use super::{Outline, parse, write};
    use crate::FeedError;

    #[test]
    fn folders_odd_capitalisation_and_duplicates() {
        let text = r#"<?xml version="1.0"?>
            <OPML version="1.1"><body>
              <outline text="Politik">
                <outline type="rss" text="Lage der Nation" xmlUrl="https://feeds.lagedernation.org/feeds/ldn-mp3.xml"/>
                <outline TYPE="RSS" TITLE="Logbuch &amp; Netzpolitik" XMLURL=" https://feeds.metaebene.me/lnp/mp3 "/>
              </outline>
              <outline text="Doppelt" xmlUrl="https://feeds.lagedernation.org/feeds/ldn-mp3.xml"/>
              <outline text="Ohne Adresse"/>
              <outline url="https://nur-url.example/feed"/>
              <outline text="Kein Feed" xmlUrl="mailto:someone@example.org"/>
            </body></OPML>"#;
        let outlines = parse(text).expect("readable");
        let titles: Vec<&str> = outlines.iter().map(|outline| outline.title.as_str()).collect();
        assert_eq!(titles, vec!["Lage der Nation", "Logbuch & Netzpolitik", "https://nur-url.example/feed"]);
        assert_eq!(outlines[1].feed_url, "https://feeds.metaebene.me/lnp/mp3");
    }

    #[test]
    fn what_is_written_is_read_back() {
        let outlines = vec![
            Outline { title: "Küche & \"Keller\" <live>".into(), feed_url: "https://a.example/feed?x=1&y=2".into() },
            Outline { title: "Zwei".into(), feed_url: "https://b.example/feed".into() },
        ];
        assert_eq!(parse(&write("TorroCast", &outlines)).expect("our own output is readable"), outlines);
    }

    #[test]
    fn a_feed_is_not_an_opml_file() {
        assert_eq!(parse("<rss><channel/></rss>"), Err(FeedError::NotAFeed));
        assert!(matches!(parse("no xml"), Err(FeedError::Malformed(_))));
    }
}
