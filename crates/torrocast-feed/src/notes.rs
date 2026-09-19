//! Show notes arrive as HTML of every quality. A user interface wants
//! something it can lay out itself: paragraphs, headings, list items, and links
//! that can be opened by number. That is a [`Document`].

use crate::entities::decode;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub blocks: Vec<Block>,
    /// Link targets; an [`Inline::Link`] refers to them by position.
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading(Vec<Inline>),
    Item(Vec<Inline>),
}

impl Block {
    #[must_use]
    pub fn inlines(&self) -> &[Inline] {
        match self {
            Self::Paragraph(inlines) | Self::Heading(inlines) | Self::Item(inlines) => inlines,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    /// `target` indexes [`Document::links`].
    Link {
        text: String,
        target: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Paragraph,
    Heading,
    Item,
}

#[derive(Default)]
struct Builder {
    document: Document,
    inlines: Vec<Inline>,
    text: String,
    kind: Option<Kind>,
    /// Target and collected text of the `<a>` we are inside of.
    link: Option<(String, String)>,
}

impl Builder {
    fn target(&mut self, url: &str) -> usize {
        if let Some(known) = self.document.links.iter().position(|link| link == url) {
            return known;
        }
        self.document.links.push(url.to_owned());
        self.document.links.len() - 1
    }

    /// `text` is already collapsed; two pieces must not meet in a double space.
    fn push_text(&mut self, text: &str) {
        let buffer = match &mut self.link {
            Some((_, collected)) => collected,
            None => &mut self.text,
        };
        let text = if buffer.ends_with(' ') { text.trim_start_matches(' ') } else { text };
        buffer.push_str(text);
    }

    /// Text outside of `<a>` still contains addresses people typed out.
    fn flush_text(&mut self) {
        let text = std::mem::take(&mut self.text);
        let mut plain = String::new();
        for (index, word) in text.split(' ').enumerate() {
            if index > 0 {
                plain.push(' ');
            }
            let address = word.trim_end_matches(['.', ',', ';', ')', '!', '?']);
            let is_address = (address.starts_with("https://") || address.starts_with("http://")) && address.len() > 10;
            if !is_address {
                plain.push_str(word);
                continue;
            }
            if !plain.is_empty() {
                self.inlines.push(Inline::Text(std::mem::take(&mut plain)));
            }
            let target = self.target(address);
            self.inlines.push(Inline::Link { text: display(address), target });
            plain.push_str(&word[address.len()..]);
        }
        if !plain.is_empty() {
            self.inlines.push(Inline::Text(plain));
        }
    }

    fn open_link(&mut self, url: String) {
        self.flush_text();
        self.link = Some((url, String::new()));
    }

    fn close_link(&mut self) {
        let Some((url, text)) = self.link.take() else {
            return;
        };
        let text = collapse(&text);
        let text = text.trim();
        let target = self.target(&url);
        let shown = if text.is_empty() || text == url { display(&url) } else { text.to_owned() };
        self.inlines.push(Inline::Link { text: shown, target });
    }

    fn end_block(&mut self) {
        self.close_link();
        self.flush_text();
        let mut inlines = std::mem::take(&mut self.inlines);
        trim(&mut inlines);
        let kind = self.kind.take().unwrap_or(Kind::Paragraph);
        if inlines.is_empty() {
            return;
        }
        self.document.blocks.push(match kind {
            Kind::Paragraph => Block::Paragraph(inlines),
            Kind::Heading => Block::Heading(inlines),
            Kind::Item => Block::Item(inlines),
        });
    }

    fn begin(&mut self, kind: Kind) {
        self.end_block();
        self.kind = Some(kind);
    }
}

/// An address as a reader wants to see it: no scheme, no trailing slash, not endless.
fn display(url: &str) -> String {
    let short = url.split_once("://").map_or(url, |(_, rest)| rest);
    let short = short.strip_prefix("www.").unwrap_or(short).trim_end_matches('/');
    if short.chars().count() > 48 {
        let head: String = short.chars().take(47).collect();
        format!("{head}…")
    } else {
        short.to_owned()
    }
}

fn collapse(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut in_space = false;
    for character in text.chars() {
        if character.is_whitespace() && character != '\u{a0}' {
            in_space = true;
            continue;
        }
        if in_space {
            collapsed.push(' ');
        }
        in_space = false;
        collapsed.push(if character == '\u{a0}' { ' ' } else { character });
    }
    if in_space {
        collapsed.push(' ');
    }
    collapsed
}

fn trim(inlines: &mut Vec<Inline>) {
    if let Some(Inline::Text(text)) = inlines.first_mut() {
        *text = text.trim_start().to_owned();
    }
    if let Some(Inline::Text(text)) = inlines.last_mut() {
        *text = text.trim_end().to_owned();
    }
    inlines.retain(|inline| !matches!(inline, Inline::Text(text) if text.is_empty()));
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let mut search = 0;
    while let Some(found) = lower[search..].find(name) {
        let start = search + found;
        let before_ok = start == 0 || lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = tag[start + name.len()..].trim_start();
        if before_ok && let Some(value) = rest.strip_prefix('=') {
            let value = value.trim_start();
            let value = match value.chars().next() {
                Some(quote @ ('"' | '\'')) => value[1..].split(quote).next().unwrap_or(""),
                _ => value.split(|character: char| character.is_whitespace() || character == '>').next().unwrap_or(""),
            };
            return Some(decode(value.trim()));
        }
        search = start + name.len();
    }
    None
}

/// Turns show notes into blocks. Input without any markup is read as plain
/// text, one paragraph per line.
#[must_use]
pub fn document(source: &str) -> Document {
    let mut builder = Builder::default();
    if !source.contains('<') {
        for line in source.lines() {
            builder.push_text(&collapse(&decode(line)));
            builder.end_block();
        }
        return builder.document;
    }

    let mut rest = source;
    // Content of these is never text for a reader.
    let mut skipping: Option<&'static str> = None;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            if skipping.is_none() {
                builder.push_text(&collapse(&decode(rest)));
            }
            break;
        };
        if skipping.is_none() && open > 0 {
            builder.push_text(&collapse(&decode(&rest[..open])));
        }
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(close) = rest.find('>') else {
            break;
        };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];

        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if let Some(skipped) = skipping {
            if closing && name == skipped {
                skipping = None;
            }
            continue;
        }
        match (name.as_str(), closing) {
            ("script", false) => skipping = Some("script"),
            ("style", false) => skipping = Some("style"),
            ("a", false) => {
                if let Some(url) = attribute(tag, "href").filter(|url| url.starts_with("http")) {
                    builder.open_link(url);
                }
            }
            ("a", true) => builder.close_link(),
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => builder.begin(Kind::Heading),
            ("li", false) => builder.begin(Kind::Item),
            (
                "p" | "div" | "br" | "hr" | "ul" | "ol" | "li" | "blockquote" | "tr" | "table" | "section" | "article",
                _,
            )
            | ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => builder.end_block(),
            _ => {}
        }
    }
    builder.end_block();
    builder.document
}

/// The text alone, blocks separated by blank lines — for descriptions shown in a few lines.
#[must_use]
pub fn plain_text(source: &str) -> String {
    document(source)
        .blocks
        .iter()
        .map(|block| {
            let text: String = block
                .inlines()
                .iter()
                .map(|inline| match inline {
                    Inline::Text(text) | Inline::Link { text, .. } => text.as_str(),
                })
                .collect();
            match block {
                Block::Item(_) => format!("• {text}"),
                _ => text,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{Block, Inline, document, plain_text};

    fn text(value: &str) -> Inline {
        Inline::Text(value.to_owned())
    }

    #[test]
    fn paragraphs_headings_items_and_numbered_links() {
        let html = r#"<p>Wir sind live in   Leipzig &ndash; <a href="https://lagedernation.org/live">Karten</a>.</p>
            <h3>Haushalt</h3><ul><li>Beschluss <a href='https://example.org/a'>hier</a></li>
            <li>Noch einmal <a href="https://lagedernation.org/live">Karten</a></li></ul>
            <script>alert("no")</script><!-- nor this -->"#;
        let notes = document(html);

        assert_eq!(notes.links, vec!["https://lagedernation.org/live", "https://example.org/a"]);
        assert_eq!(
            notes.blocks[0],
            Block::Paragraph(vec![
                text("Wir sind live in Leipzig – "),
                Inline::Link { text: "Karten".into(), target: 0 },
                text(".")
            ])
        );
        assert_eq!(notes.blocks[1], Block::Heading(vec![text("Haushalt")]));
        assert_eq!(
            notes.blocks[2],
            Block::Item(vec![text("Beschluss "), Inline::Link { text: "hier".into(), target: 1 }])
        );
        assert!(
            matches!(&notes.blocks[3], Block::Item(inlines) if inlines[1] == Inline::Link { text: "Karten".into(), target: 0 })
        );
        assert_eq!(notes.blocks.len(), 4, "scripts and comments are not notes");
    }

    #[test]
    fn typed_out_addresses_become_links() {
        let notes = document("Mehr unter https://www.example.org/thema/, danke.<br>Tschüss");
        assert_eq!(notes.links, vec!["https://www.example.org/thema/"]);
        assert_eq!(
            notes.blocks[0],
            Block::Paragraph(vec![
                text("Mehr unter "),
                Inline::Link { text: "example.org/thema".into(), target: 0 },
                text(", danke.")
            ])
        );
        assert_eq!(notes.blocks[1], Block::Paragraph(vec![text("Tschüss")]));
    }

    #[test]
    fn plain_text_keeps_its_lines() {
        let notes = document("Erste Zeile\n\nZweite &amp; letzte");
        assert_eq!(
            notes.blocks,
            vec![Block::Paragraph(vec![text("Erste Zeile")]), Block::Paragraph(vec![text("Zweite & letzte")])]
        );
    }

    #[test]
    fn descriptions_lose_their_markup() {
        assert_eq!(
            plain_text("<p>Politik aus <b>Berlin</b></p><ul><li>wöchentlich</li></ul>"),
            "Politik aus Berlin\n• wöchentlich"
        );
    }
}
