//! Podcast covers in a terminal. Where the terminal can show pictures (Kitty,
//! Sixel, iTerm2) it gets one; elsewhere half-block characters give the
//! colours and the rough shape. Both come from `ratatui-image`.

use std::collections::HashMap;
use std::fmt;

use ratatui::layout::Size;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Resize};

/// Cells a cover takes beside a podcast's description, and in the large player.
/// In a list, before each episode: two rows high, as the entry is.
pub const TINY: Size = Size { width: 4, height: 2 };
pub const SMALL: Size = Size { width: 12, height: 6 };
pub const LARGE: Size = Size { width: 18, height: 9 };

/// Which picture protocol the terminal speaks, told from its environment.
///
/// The library could ask the terminal itself, but it asks on standard input:
/// a terminal that stays silent (tmux, for one) costs two seconds at the
/// start, and the abandoned reader then swallows the user's keys. The
/// environment answers at once and takes nothing.
#[must_use]
pub fn protocol_of(environment: &HashMap<String, String>) -> ProtocolType {
    let variable = |name: &str| environment.get(name).map(|value| value.to_lowercase()).unwrap_or_default();
    // A multiplexer stands between us and the terminal and passes no pictures on by default.
    if environment.contains_key("TMUX") || variable("TERM").starts_with("screen") {
        return ProtocolType::Halfblocks;
    }
    let (term, program) = (variable("TERM"), variable("TERM_PROGRAM"));
    if environment.contains_key("KITTY_WINDOW_ID")
        || term.contains("kitty")
        || program == "ghostty"
        || program == "wezterm"
    {
        ProtocolType::Kitty
    } else if program == "iterm.app" {
        ProtocolType::Iterm2
    } else if term.starts_with("foot") || term.contains("sixel") || environment.contains_key("WT_SESSION") {
        ProtocolType::Sixel
    } else {
        ProtocolType::Halfblocks
    }
}

/// A picker for this terminal. Real pictures need the size of a cell in
/// pixels; a terminal that does not tell gets half blocks, which need nothing.
#[must_use]
pub fn picker_for(
    environment: &HashMap<String, String>,
    window_pixels: (u16, u16),
    window_cells: (u16, u16),
) -> Picker {
    let protocol = protocol_of(environment);
    let (columns, rows) = (window_cells.0.max(1), window_cells.1.max(1));
    let cell = (window_pixels.0 / columns, window_pixels.1 / rows);
    if protocol == ProtocolType::Halfblocks || cell.0 == 0 || cell.1 == 0 {
        return Picker::halfblocks();
    }
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(cell.into());
    picker.set_protocol_type(protocol);
    picker
}

pub struct Cover {
    pub tiny: Protocol,
    pub small: Protocol,
    pub large: Protocol,
}

/// The covers known so far, by address. `None` is a cover asked for and not
/// here yet — or not to be had at all.
#[derive(Default)]
pub struct Covers {
    picker: Option<Picker>,
    known: HashMap<String, Option<Cover>>,
}

impl fmt::Debug for Covers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Covers({} known, pictures {})",
            self.known.len(),
            if self.picker.is_some() { "on" } else { "off" }
        )
    }
}

impl Covers {
    /// With a picker covers are shown; without one (tests, or switched off) nothing is even fetched.
    #[must_use]
    pub fn new(picker: Option<Picker>) -> Self {
        Self { picker, known: HashMap::new() }
    }

    /// Notes that `url` is wanted. `true` if it has to be fetched.
    pub fn want(&mut self, url: &str) -> bool {
        if self.picker.is_none() || self.known.contains_key(url) {
            return false;
        }
        self.known.insert(url.to_owned(), None);
        true
    }

    /// Takes the fetched bytes and turns them into something the terminal can show.
    pub fn arrived(&mut self, url: &str, bytes: &[u8]) {
        let Some(picker) = &self.picker else { return };
        let Ok(picture) = image::load_from_memory(bytes) else { return };
        // Feeds carry covers of 3000 pixels and more; a terminal cell grid has no use for them.
        let picture = picture.thumbnail(480, 480);
        let fitted = |size| picker.new_protocol(picture.clone(), size, Resize::Fit(Some(FilterType::Triangle))).ok();
        if let (Some(tiny), Some(small), Some(large)) = (fitted(TINY), fitted(SMALL), fitted(LARGE)) {
            self.known.insert(url.to_owned(), Some(Cover { tiny, small, large }));
        }
    }

    /// Whether pictures are shown at all — lists leave room for them only then.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.picker.is_some()
    }

    #[must_use]
    pub fn get(&self, url: Option<&str>) -> Option<&Cover> {
        self.known.get(url?)?.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ratatui_image::picker::ProtocolType;

    use super::{picker_for, protocol_of};

    fn environment(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect()
    }

    #[test]
    fn terminals_are_known_by_their_environment() {
        assert_eq!(protocol_of(&environment(&[("TERM", "foot")])), ProtocolType::Sixel);
        assert_eq!(protocol_of(&environment(&[("TERM", "xterm-kitty")])), ProtocolType::Kitty);
        assert_eq!(
            protocol_of(&environment(&[("TERM", "xterm-256color"), ("TERM_PROGRAM", "ghostty")])),
            ProtocolType::Kitty
        );
        assert_eq!(protocol_of(&environment(&[("TERM_PROGRAM", "iTerm.app")])), ProtocolType::Iterm2);
        assert_eq!(protocol_of(&environment(&[("WT_SESSION", "1")])), ProtocolType::Sixel);
        assert_eq!(protocol_of(&environment(&[("TERM", "alacritty")])), ProtocolType::Halfblocks);
        assert_eq!(
            protocol_of(&environment(&[("TERM", "foot"), ("TMUX", "/tmp/tmux")])),
            ProtocolType::Halfblocks,
            "tmux passes no pictures on"
        );
    }

    #[test]
    fn without_pixel_sizes_there_are_half_blocks() {
        let foot = environment(&[("TERM", "foot")]);
        assert_eq!(picker_for(&foot, (0, 0), (120, 40)).protocol_type(), ProtocolType::Halfblocks);
        let picker = picker_for(&foot, (1200, 880), (120, 40));
        assert_eq!(picker.protocol_type(), ProtocolType::Sixel);
        assert_eq!((picker.font_size().width, picker.font_size().height), (10, 22));
    }
}
