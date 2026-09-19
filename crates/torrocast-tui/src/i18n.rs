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
        if locale.to_lowercase().starts_with("de") { Self::De } else { Self::En }
    }

    #[must_use]
    pub fn t(self, english: &'static str) -> &'static str {
        if self == Self::En {
            return english;
        }
        GERMAN.iter().find(|(key, _)| *key == english).map_or(english, |(_, german)| german)
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

    /// The mark between whole and tenths: `1,3×` or `1.3×`.
    #[must_use]
    pub fn decimal(self) -> &'static str {
        match self {
            Self::De => ",",
            Self::En => ".",
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
    ("Make the window larger — TorroCast keeps running.", "Zieh das Fenster größer – TorroCast läuft weiter."),
    // new episodes
    ("New Episodes", "Neue Folgen"),
    ("still loading feeds", "Feeds laden noch"),
    ("feeds did not answer", "Feeds haben nicht geantwortet"),
    ("Looking through your subscriptions …", "Sehe deine Abos durch …"),
    (
        "Nothing new in the last two weeks. r looks again.",
        "Nichts Neues in den letzten zwei Wochen. r sieht noch einmal nach.",
    ),
    (
        "New episodes of your subscriptions appear here. Subscribe to a podcast with s first.",
        "Hier erscheinen neue Folgen deiner Abos. Abonniere zuerst einen Podcast mit s.",
    ),
    // subscriptions and library
    ("Subscriptions", "Abos"),
    ("Subscribed", "Abonniert"),
    ("subscribe", "Abonnieren"),
    ("subscribe to the podcast, or end the subscription", "Podcast abonnieren oder Abo beenden"),
    ("No subscriptions yet. Open a podcast and press s.", "Noch keine Abos. Öffne einen Podcast und drück s."),
    (
        "They are kept in the library folder and appear on every device that shares it.",
        "Sie liegen im Bibliotheks-Ordner und erscheinen auf jedem Gerät, das ihn teilt.",
    ),
    ("Library folder", "Bibliotheks-Ordner"),
    (
        "Subscriptions, Up Next and positions live here. Set library_dir in config.toml to move it into a synced folder.",
        "Hier liegen Abos, „Als Nächstes“ und Hörpositionen. Mit library_dir in der config.toml legst du ihn in einen synchronisierten Ordner.",
    ),
    (
        "The library could not be opened; nothing is kept beyond this session.",
        "Die Bibliothek ließ sich nicht öffnen; nichts bleibt über diese Sitzung hinaus erhalten.",
    ),
    // playback
    ("Up Next", "Als Nächstes"),
    ("Now playing", "Läuft gerade"),
    ("Loading …", "Lade …"),
    ("Chapter", "Kapitel"),
    ("Tempo", "Tempo"),
    ("left", "noch"),
    ("Cannot be played. ␣ tries again.", "Nicht abspielbar. ␣ versucht es neu."),
    (
        "This episode cannot be played right now. ␣ tries again.",
        "Diese Folge lässt sich gerade nicht abspielen. ␣ versucht es noch einmal.",
    ),
    ("After this the player falls silent.", "Danach wird es still."),
    ("After this, number 1 follows without a pause.", "Danach geht es ohne Pause mit Platz 1 weiter."),
    ("Nothing is playing. Press p on an episode.", "Es läuft nichts. Drück p auf einer Folge."),
    (
        "Nothing queued. In any list of episodes, a puts one at the end and A at the front.",
        "Nichts eingereiht. In jeder Folgenliste legt a eine Folge ans Ende und A an den Anfang.",
    ),
    ("Press C again to empty the list.", "Drück noch einmal C, um die Liste zu leeren."),
    ("playing", "läuft"),
    ("number", "Platz"),
    ("Podcasts", "Podcasts"),
    ("Searching for episodes …", "Suche Folgen …"),
    // key hints
    ("pause", "Pause"),
    ("chapter", "Kapitel"),
    ("next episode", "Nächste Folge"),
    ("stop", "Stopp"),
    ("tempo", "Tempo"),
    ("jump", "Springen"),
    ("player", "Player"),
    ("play", "Spielen"),
    ("to the end", "Ans Ende"),
    ("to the front", "An den Anfang"),
    ("podcasts/episodes", "Podcasts/Folgen"),
    ("podcasts", "Podcasts"),
    ("move", "Verschieben"),
    ("remove", "Entfernen"),
    ("empty", "Leeren"),
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
    ("A feed address works here too.", "Eine Feed-Adresse funktioniert hier auch."),
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
    ("This feed has no episodes yet.", "Dieser Feed enthält noch keine Folgen."),
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
    ("Looking for chapters in the audio file …", "Suche Kapitel in der Audiodatei …"),
    ("Loading the chapters …", "Lade die Kapitel …"),
    ("From the feed", "Aus dem Feed"),
    ("From the chapters file", "Aus der Kapiteldatei"),
    ("From the audio file", "Aus der Audiodatei"),
    ("No chapters.", "Keine Kapitel."),
    // settings
    ("Where TorroCast looks for podcasts", "Wo TorroCast nach Podcasts sucht"),
    ("Active", "Aktiv"),
    ("Off", "Ausgeschaltet"),
    ("Search, charts and categories. Needs no setup.", "Suche, Charts und Kategorien. Braucht keine Einrichtung."),
    ("German-language directory. Needs no setup.", "Deutschsprachiges Verzeichnis. Braucht keine Einrichtung."),
    (
        "Open directory with trending shows. Needs your own free key.",
        "Offenes Verzeichnis mit Trending. Braucht deinen eigenen, kostenlosen Schlüssel.",
    ),
    ("Comes with a later version", "Kommt mit einer späteren Version"),
    ("Country for search and charts", "Land für Suche und Charts"),
    ("The settings could not be saved.", "Die Einstellungen konnten nicht gespeichert werden."),
    // help
    ("Keys", "Tasten"),
    ("Everywhere", "Überall"),
    ("In lists", "In Listen"),
    ("In an episode", "In einer Folge"),
    ("Playback", "Wiedergabe"),
    ("On an episode", "Auf einer Folge"),
    ("In Up Next", "In „Als Nächstes“"),
    ("pause and resume", "Pause und weiter"),
    ("stop: remember the place, close the player", "Stopp: Stelle merken, Player schließen"),
    ("previous and next chapter", "Kapitel zurück und vor"),
    ("next episode from Up Next", "nächste Folge aus „Als Nächstes“"),
    ("30 seconds back and forward", "30 Sekunden zurück und vor"),
    ("slower and faster, at the same pitch", "langsamer und schneller, bei gleicher Tonhöhe"),
    ("open and close the large player", "großen Player öffnen und schließen"),
    ("play now; what was playing moves to Up Next", "jetzt spielen; was lief, rückt in „Als Nächstes“"),
    ("to the end of Up Next", "ans Ende von „Als Nächstes“"),
    ("to the front of Up Next", "an den Anfang von „Als Nächstes“"),
    ("search for episodes instead of podcasts", "Folgen statt Podcasts suchen"),
    ("move the episode down and up", "Folge nach unten und oben schieben"),
    ("take the episode out", "Folge herausnehmen"),
    ("empty the list (asks once)", "Liste leeren (fragt einmal nach)"),
    (
        "The mouse works too: the wheel scrolls, a click picks a menu entry or a button of the player.",
        "Die Maus geht auch: Das Rad blättert, ein Klick wählt einen Menüeintrag oder einen Knopf des Players.",
    ),
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
    ("menu (in an episode the digits open links)", "Menü (in einer Folge öffnen die Ziffern Links)"),
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
    ("This address does not lead to a podcast feed.", "Diese Adresse führt nicht zu einem Podcast-Feed."),
    ("The answer could not be read.", "Die Antwort ließ sich nicht lesen."),
];
