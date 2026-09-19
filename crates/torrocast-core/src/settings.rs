//! What the user has chosen, kept in one small TOML file on this machine.
//! The synced library folder is a different thing and comes later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Two-letter country for search and charts.
    pub country: String,
    pub sources: Sources,
    /// The folder the library lives in — inside Dropbox, Syncthing, a NAS mount,
    /// wherever it should be backed up and shared from. `None` is the default place.
    pub library_dir: Option<String>,
    /// Where downloaded episodes are kept. Never inside the library folder: audio is large and is not synced.
    pub download_dir: Option<String>,
    /// This device's name in the library. Made up once, then kept.
    pub device_id: Option<String>,
}

/// Apple is always on: it needs no setup and nothing works without a directory.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sources {
    pub fyyd: bool,
    pub podcast_index: bool,
    /// The user's own key and secret from api.podcastindex.org. TorroCast ships none.
    pub podcast_index_key: String,
    pub podcast_index_secret: String,
}

impl Sources {
    /// Switched on, and with what it takes to be asked.
    #[must_use]
    pub fn uses_podcast_index(&self) -> bool {
        self.podcast_index && !self.podcast_index_key.trim().is_empty() && !self.podcast_index_secret.trim().is_empty()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            country: "us".to_owned(),
            sources: Sources::default(),
            library_dir: None,
            download_dir: None,
            device_id: None,
        }
    }
}

/// The countries offered in the settings, in the order they are cycled through.
pub const COUNTRIES: [&str; 12] = ["de", "at", "ch", "us", "gb", "fr", "es", "it", "nl", "se", "dk", "pl"];

impl Settings {
    /// Defaults for a first start: the country of the user's locale.
    #[must_use]
    pub fn for_locale(locale: &str) -> Self {
        let country = locale
            .split(['.', '@'])
            .next()
            .and_then(|language| language.split_once('_'))
            .map(|(_, country)| country.to_lowercase())
            .filter(|country| country.len() == 2 && country.chars().all(|letter| letter.is_ascii_lowercase()));
        Self { country: country.unwrap_or_else(|| "us".to_owned()), ..Self::default() }
    }

    /// A missing or unreadable file means defaults — never a refusal to start.
    #[must_use]
    pub fn load(file: &Path, locale: &str) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|text| toml::from_str::<Self>(&text).ok())
            .unwrap_or_else(|| Self::for_locale(locale))
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        if let Some(directory) = file.parent() {
            std::fs::create_dir_all(directory)?;
        }
        // Written beside the target and renamed, so a crash never leaves half a file.
        let temporary = file.with_extension("toml.tmp");
        std::fs::write(&temporary, text)?;
        // The file may hold a directory key: for the user's eyes only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(temporary, file)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Where the settings live. A pure function of platform and environment, so
/// every platform's answer can be tested on any machine.
#[must_use]
pub fn config_file(platform: Platform, environment: &HashMap<String, String>) -> Option<PathBuf> {
    let variable = |name: &str| environment.get(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let directory = match platform {
        Platform::Linux => {
            variable("XDG_CONFIG_HOME").or_else(|| Some(variable("HOME")?.join(".config")))?.join("torrocast")
        }
        // The same place a native app will use, so neither has to move later.
        Platform::MacOs => variable("HOME")?.join("Library").join("Application Support").join("TorroCast"),
        Platform::Windows => variable("APPDATA")?.join("TorroCast"),
    };
    Some(directory.join("config.toml"))
}

/// Where the library lives unless the user chose a folder.
#[must_use]
pub fn default_library_dir(platform: Platform, environment: &HashMap<String, String>) -> Option<PathBuf> {
    let variable = |name: &str| environment.get(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let directory = match platform {
        Platform::Linux => variable("XDG_DATA_HOME")
            .or_else(|| Some(variable("HOME")?.join(".local").join("share")))?
            .join("torrocast"),
        Platform::MacOs => variable("HOME")?.join("Library").join("Application Support").join("TorroCast"),
        Platform::Windows => variable("APPDATA")?.join("TorroCast"),
    };
    Some(directory.join("library"))
}

/// Where downloads go unless the user chose a folder: beside the library, not in it.
#[must_use]
pub fn default_download_dir(platform: Platform, environment: &HashMap<String, String>) -> Option<PathBuf> {
    default_library_dir(platform, environment).map(|library| library.with_file_name("downloads"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::{Platform, Settings, config_file, default_library_dir};

    fn environment(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect()
    }

    #[test]
    fn every_platform_has_its_place() {
        let home = environment(&[("HOME", "/home/ada")]);
        assert_eq!(config_file(Platform::Linux, &home), Some(PathBuf::from("/home/ada/.config/torrocast/config.toml")));
        assert_eq!(
            config_file(Platform::MacOs, &home),
            Some(PathBuf::from("/home/ada/Library/Application Support/TorroCast/config.toml"))
        );
        let xdg = environment(&[("HOME", "/home/ada"), ("XDG_CONFIG_HOME", "/cfg")]);
        assert_eq!(config_file(Platform::Linux, &xdg), Some(PathBuf::from("/cfg/torrocast/config.toml")));
        assert_eq!(config_file(Platform::Windows, &home), None, "no APPDATA, no guess");
        assert_eq!(
            default_library_dir(Platform::Linux, &home),
            Some(PathBuf::from("/home/ada/.local/share/torrocast/library"))
        );
        assert_eq!(
            default_library_dir(Platform::MacOs, &home),
            Some(PathBuf::from("/home/ada/Library/Application Support/TorroCast/library"))
        );
    }

    #[test]
    fn the_locale_picks_the_country() {
        assert_eq!(Settings::for_locale("de_DE.UTF-8").country, "de");
        assert_eq!(Settings::for_locale("en_GB").country, "gb");
        assert_eq!(Settings::for_locale("C").country, "us");
        assert_eq!(Settings::for_locale("").country, "us");
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let directory = std::env::temp_dir().join(format!("torrocast-settings-{}", std::process::id()));
        let file = directory.join("nested").join("config.toml");
        let mut settings = Settings::for_locale("de_AT.UTF-8");
        settings.sources.fyyd = true;
        settings.save(&file).expect("temp dir is writable");
        assert_eq!(Settings::load(&file, "en_US"), settings);
        assert_eq!(Settings::load(&directory.join("missing.toml"), "fr_FR").country, "fr");
        let _ = std::fs::remove_dir_all(directory);
    }
}
