//! Named character references. Feeds and show notes use HTML's names; XML knows five.

const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("ndash", '–'),
    ("mdash", '—'),
    ("hellip", '…'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("bdquo", '„'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("sbquo", '‚'),
    ("auml", 'ä'),
    ("ouml", 'ö'),
    ("uuml", 'ü'),
    ("Auml", 'Ä'),
    ("Ouml", 'Ö'),
    ("Uuml", 'Ü'),
    ("szlig", 'ß'),
    ("euro", '€'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("eacute", 'é'),
    ("egrave", 'è'),
    ("agrave", 'à'),
    ("aacute", 'á'),
    ("ccedil", 'ç'),
    ("ntilde", 'ñ'),
    ("oslash", 'ø'),
    ("aring", 'å'),
    ("middot", '·'),
    ("bull", '•'),
    ("deg", '°'),
    ("times", '×'),
    ("rarr", '→'),
    ("larr", '←'),
    ("shy", '\u{ad}'),
];

const XML: [&str; 5] = ["amp", "lt", "gt", "quot", "apos"];

fn named(name: &str) -> Option<char> {
    NAMED.iter().find(|(known, _)| *known == name).map(|(_, character)| *character)
}

fn numeric(reference: &str) -> Option<char> {
    let digits = reference.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse().ok()?,
    };
    char::from_u32(code)
}

/// Splits `text` at `&`: the reference's name if one follows, and how many bytes it spans.
fn reference(text: &str) -> Option<(&str, usize)> {
    let end = text.bytes().take(12).position(|byte| byte == b';')?;
    let name = &text[1..end];
    let plausible = !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'#');
    plausible.then_some((name, end + 1))
}

/// `Caf&eacute; &amp; Bar` → `Café & Bar`. What is not a reference stays as written.
#[must_use]
pub(crate) fn decode(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = rest.find('&') {
        decoded.push_str(&rest[..position]);
        rest = &rest[position..];
        let replacement =
            reference(rest).and_then(|(name, length)| Some((named(name).or_else(|| numeric(name))?, length)));
        match replacement {
            Some((character, length)) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

/// Makes a feed that leans on HTML acceptable to an XML parser: HTML's names
/// become numbers, and an `&` that starts nothing becomes `&amp;`.
#[must_use]
pub(crate) fn repair_xml(xml: &str) -> String {
    let mut repaired = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(position) = rest.find('&') {
        repaired.push_str(&rest[..position]);
        rest = &rest[position..];
        match reference(rest) {
            Some((name, length)) if XML.contains(&name) || numeric(name).is_some() => {
                repaired.push_str(&rest[..length]);
                rest = &rest[length..];
            }
            Some((name, length)) if named(name).is_some() => {
                let character = named(name).unwrap_or(' ');
                repaired.push_str(&format!("&#{};", u32::from(character)));
                rest = &rest[length..];
            }
            _ => {
                repaired.push_str("&amp;");
                rest = &rest[1..];
            }
        }
    }
    repaired.push_str(rest);
    repaired
}

#[cfg(test)]
mod tests {
    use super::{decode, repair_xml};

    #[test]
    fn decodes_names_and_numbers() {
        assert_eq!(decode("Caf&eacute; &amp; Bar &#8211; &#x2014; R&D"), "Café & Bar – — R&D");
    }

    #[test]
    fn repairs_only_what_xml_would_reject() {
        assert_eq!(repair_xml("a &amp; b&nbsp;c & d &#228; &bogus;"), "a &amp; b&#160;c &amp; d &#228; &amp;bogus;");
    }
}
