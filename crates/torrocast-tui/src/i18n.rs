//! Two languages, as in every Torro app. The English text is the key, so a
//! sentence without a translation still reads as a sentence.

use torrocast_core::{Problem, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// From `LC_ALL`, `LC_MESSAGES` or `LANG`, in that order.
    #[must_use]
    pub fn from_locale(locale: &str) -> Self {
        if locale.to_lowercase().starts_with("de") {
            Self::De
        } else {
            Self::En
        }
    }

    #[must_use]
    pub fn t(self, english: &'static str) -> &'static str {
        if self == Self::En {
            return english;
        }
        GERMAN
            .iter()
            .find(|(key, _)| *key == english)
            .map_or(english, |(_, german)| german)
    }

    /// What went wrong, as a sentence about the user's situation — not about HTTP.
    #[must_use]
    pub fn problem(self, problem: Problem) -> &'static str {
        self.t(match problem {
            Problem::Unreachable => "No connection. Check the network and press r to try again.",
            Problem::Refused(404 | 410) => "This address no longer exists.",
            Problem::Refused(_) => "The server is not answering properly right now. Press r to try again.",
            Problem::RateLimited => {
                "That was a lot of searching. Apple asks for a short break — try again in a minute."
            }
            Problem::NotAFeed => "This address does not lead to a podcast feed.",
            Problem::Unreadable => "The answer could not be read.",
        })
    }

    /// One directory failed while others may have answered.
    #[must_use]
    pub fn provider_problem(self, provider: ProviderId, problem: Problem) -> String {
        let name = provider.name();
        match (self, problem) {
            (Self::De, Problem::RateLimited) => format!("{name} bittet um eine kurze Pause."),
            (Self::De, _) => format!("{name} antwortet gerade nicht."),
            (Self::En, Problem::RateLimited) => format!("{name} asks for a short break."),
            (Self::En, _) => format!("{name} is not answering right now."),
        }
    }

    #[must_use]
    pub fn date_format(self) -> &'static str {
        match self {
            Self::De => "%d.%m.%Y",
            Self::En => "%Y-%m-%d",
        }
    }

    #[must_use]
    pub fn country(self, code: &str) -> String {
        let english = match code {
            "de" => "Germany",
            "at" => "Austria",
            "ch" => "Switzerland",
            "us" => "United States",
            "gb" => "United Kingdom",
            "fr" => "France",
            "es" => "Spain",
            "it" => "Italy",
            "nl" => "Netherlands",
            "se" => "Sweden",
            "dk" => "Denmark",
            "pl" => "Poland",
            other => return other.to_uppercase(),
        };
        self.t(english).to_owned()
    }
}

const GERMAN: &[(&str, &str)] = &[
    // frame
    ("Menu", "Menü"),
    ("Discover", "Entdecken"),
    ("Settings", "Einstellungen"),
    ("Help", "Hilfe"),
    ("Podcasts in the terminal.", "Podcasts im Terminal."),
    ("The window is too small", "Das Fenster ist zu klein"),
    ("Now", "Jetzt"),
    ("Needed", "Benötigt"),
    ("Width", "Breite"),
    ("Height", "Höhe"),
    (
        "Make the window larger — TorroCast keeps running.",
        "Zieh das Fenster größer – TorroCast läuft weiter.",
    ),
    // key hints
    ("select", "Auswahl"),
    ("open", "Öffnen"),
    ("tab", "Reiter"),
    ("leave input", "Eingabe verlassen"),
    ("search", "Suchen"),
    ("search now", "Sofort suchen"),
    ("menu", "Menü"),
    ("quit", "Beenden"),
    ("back", "Zurück"),
    ("filter", "Filtern"),
    ("order", "Reihenfolge"),
    ("website", "Website"),
    ("more", "mehr"),
    ("less", "weniger"),
    ("reload", "Neu laden"),
    ("notes/chapters", "Shownotes/Kapitel"),
    ("scroll", "Blättern"),
    ("open link", "Link öffnen"),
    ("in browser", "Im Browser"),
    ("on/off", "An/Aus"),
    ("change", "Ändern"),
    ("all charts", "Alle Charts"),
    ("done", "Fertig"),
    // discover
    ("Search", "Suche"),
    ("Charts", "Charts"),
    ("Categories", "Kategorien"),
    ("Results", "Treffer"),
    ("Preview", "Vorschau"),
    (
        "Nothing searched yet. Type a word, or switch to the charts with tab.",
        "Noch keine Suche. Tippe einen Begriff, oder wechsle mit tab zu den Charts.",
    ),
    (
        "A feed address works here too.",
        "Eine Feed-Adresse funktioniert hier auch.",
    ),
    ("Searching …", "Suche …"),
    ("Nothing found for this.", "Dazu wurde nichts gefunden."),
    ("still loading", "lädt noch"),
    ("Loading the charts …", "Lade die Charts …"),
    ("Loading the categories …", "Lade die Kategorien …"),
    ("Source", "Quelle"),
    ("Website", "Website"),
    ("Language", "Sprache"),
    ("episodes", "Folgen"),
    ("last", "zuletzt"),
    (
        "Description and episodes appear once you open the podcast.",
        "Beschreibung und Folgen erscheinen, sobald du den Podcast öffnest.",
    ),
    (
        "This podcast has no open feed. It can only be heard inside the platform that hosts it.",
        "Dieser Podcast hat keinen offenen Feed. Er ist nur auf der Plattform zu hören, die ihn anbietet.",
    ),
    // podcast
    ("Episodes", "Folgen"),
    ("newest first", "neueste zuerst"),
    ("oldest first", "älteste zuerst"),
    ("Loading the episodes …", "Lade Folgen …"),
    (
        "This feed has no episodes yet.",
        "Dieser Feed enthält noch keine Folgen.",
    ),
    ("No episode matches the filter.", "Keine Folge passt zum Filter."),
    ("Chapters", "Kapitel"),
    ("Transcript", "Transkript"),
    ("Support", "Unterstützen"),
    ("Filter", "Filter"),
    ("explicit", "explizit"),
    // episode
    ("Show notes", "Shownotes"),
    ("Links", "Links"),
    ("Season", "Staffel"),
    ("Episode", "Folge"),
    ("This episode has no show notes.", "Diese Folge hat keine Shownotes."),
    (
        "Looking for chapters in the audio file …",
        "Suche Kapitel in der Audiodatei …",
    ),
    ("Loading the chapters …", "Lade die Kapitel …"),
    ("From the feed", "Aus dem Feed"),
    ("From the chapters file", "Aus der Kapiteldatei"),
    ("From the audio file", "Aus der Audiodatei"),
    ("No chapters.", "Keine Kapitel."),
    // settings
    ("Where TorroCast looks for podcasts", "Wo TorroCast nach Podcasts sucht"),
    ("Active", "Aktiv"),
    ("Off", "Ausgeschaltet"),
    (
        "Search, charts and categories. Needs no setup.",
        "Suche, Charts und Kategorien. Braucht keine Einrichtung.",
    ),
    (
        "German-language directory. Needs no setup.",
        "Deutschsprachiges Verzeichnis. Braucht keine Einrichtung.",
    ),
    (
        "Open directory with trending shows. Needs your own free key.",
        "Offenes Verzeichnis mit Trending. Braucht deinen eigenen, kostenlosen Schlüssel.",
    ),
    ("Comes with a later version", "Kommt mit einer späteren Version"),
    ("Country for search and charts", "Land für Suche und Charts"),
    (
        "The settings could not be saved.",
        "Die Einstellungen konnten nicht gespeichert werden.",
    ),
    // help
    ("Keys", "Tasten"),
    ("Everywhere", "Überall"),
    ("In lists", "In Listen"),
    ("In an episode", "In einer Folge"),
    ("move the selection", "Auswahl bewegen"),
    ("page, first, last", "blättern, Anfang, Ende"),
    ("open the selected entry", "gewählten Eintrag öffnen"),
    ("one level back", "eine Ebene zurück"),
    ("switch tab or panel", "Reiter oder Panel wechseln"),
    ("search, or filter a list", "suchen oder Liste filtern"),
    ("reverse the order", "Reihenfolge umkehren"),
    ("open the website in the browser", "Website im Browser öffnen"),
    ("unfold the description", "Beschreibung aufklappen"),
    ("open the numbered link", "nummerierten Link öffnen"),
    ("open the chapter's link", "Link des Kapitels öffnen"),
    (
        "menu (in an episode the digits open links)",
        "Menü (in einer Folge öffnen die Ziffern Links)",
    ),
    (
        "The mouse works too: the wheel scrolls, a click picks a menu entry.",
        "Die Maus geht auch: Das Rad blättert, ein Klick wählt einen Menüeintrag.",
    ),
    // countries
    ("Germany", "Deutschland"),
    ("Austria", "Österreich"),
    ("Switzerland", "Schweiz"),
    ("United States", "USA"),
    ("United Kingdom", "Großbritannien"),
    ("France", "Frankreich"),
    ("Spain", "Spanien"),
    ("Italy", "Italien"),
    ("Netherlands", "Niederlande"),
    ("Sweden", "Schweden"),
    ("Denmark", "Dänemark"),
    ("Poland", "Polen"),
    // problems
    (
        "No connection. Check the network and press r to try again.",
        "Keine Verbindung. Prüf das Netz und drück r für einen neuen Versuch.",
    ),
    ("This address no longer exists.", "Diese Adresse gibt es nicht mehr."),
    (
        "The server is not answering properly right now. Press r to try again.",
        "Der Server antwortet gerade nicht richtig. Drück r für einen neuen Versuch.",
    ),
    (
        "That was a lot of searching. Apple asks for a short break — try again in a minute.",
        "Das waren viele Suchen. Apple bittet um eine kurze Pause – versuch es in einer Minute wieder.",
    ),
    (
        "This address does not lead to a podcast feed.",
        "Diese Adresse führt nicht zu einem Podcast-Feed.",
    ),
    ("The answer could not be read.", "Die Antwort ließ sich nicht lesen."),
];
