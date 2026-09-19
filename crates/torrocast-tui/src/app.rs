//! What the user is looking at, and what a key does to it. The app never
//! fetches: it queues [`Command`]s for the core and is told the [`Event`]s.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use torrocast_core::settings::COUNTRIES;
use torrocast_core::{
    Category, Chapter, Command, Document, Episode, Event, Podcast, PodcastRef, Problem, ProviderId, Settings, merge,
    merge_chapters, notes,
};

use crate::i18n::Lang;

/// Apple allows about twenty searches a minute; waiting for a pause in the
/// typing keeps a whole word to a single search.
pub const SEARCH_DELAY: Duration = Duration::from_millis(600);
const MIN_QUERY: usize = 3;
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
const MENU_FIRST_ROW: u16 = 3;
pub const MENU_WIDTH: u16 = 26;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Discover,
    Settings,
    Help,
}

impl Section {
    pub const ALL: [Self; 3] = [Self::Discover, Self::Settings, Self::Help];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Discover => "Discover",
            Self::Settings => "Settings",
            Self::Help => "Help",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Charts,
    Categories,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Search, Self::Charts, Self::Categories];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Search => "Search",
            Self::Charts => "Charts",
            Self::Categories => "Categories",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Idle,
    Loading,
    Ready,
    Failed(Problem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    pub input: String,
    /// The input field has the keyboard: every character lands in it.
    pub editing: bool,
    typed_at: Option<Instant>,
    /// The query the current results belong to.
    pub sent: String,
    request: u64,
    pub pending: Vec<ProviderId>,
    batches: Vec<(ProviderId, Vec<PodcastRef>)>,
    pub failures: Vec<(ProviderId, Problem)>,
    pub results: Vec<PodcastRef>,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Charts {
    request: u64,
    /// `None` is the country's overall chart.
    pub category: Option<Category>,
    pub list: Vec<PodcastRef>,
    pub index: usize,
    pub load: Load,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Categories {
    pub list: Vec<Category>,
    pub index: usize,
    pub load: Load,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PodcastView {
    /// What the directory said; shown until the feed has arrived.
    pub reference: PodcastRef,
    request: u64,
    pub load: Load,
    pub podcast: Option<Arc<Podcast>>,
    /// Position within [`PodcastView::visible`].
    pub index: usize,
    pub newest_first: bool,
    pub filter: String,
    pub filtering: bool,
    pub expanded: bool,
}

impl PodcastView {
    /// The episodes the list shows, as positions in the feed, in display order.
    #[must_use]
    pub fn visible(&self) -> Vec<usize> {
        let Some(podcast) = &self.podcast else {
            return Vec::new();
        };
        let needle = self.filter.to_lowercase();
        let mut visible: Vec<usize> = podcast
            .episodes
            .iter()
            .enumerate()
            .filter(|(_, episode)| needle.is_empty() || episode.title.to_lowercase().contains(&needle))
            .map(|(position, _)| position)
            .collect();
        // Feeds list the newest episode first; that is the order we keep by default.
        if !self.newest_first {
            visible.reverse();
        }
        visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Notes,
    Chapters,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeView {
    pub podcast: Arc<Podcast>,
    pub position: usize,
    pub notes: Document,
    pub chapters: Vec<Chapter>,
    /// Chapters may still arrive from outside the feed.
    pub looking: Option<Looking>,
    request: u64,
    pub focus: Focus,
    pub scroll: u16,
    pub chapter_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looking {
    ChaptersFile,
    AudioFile,
}

impl EpisodeView {
    #[must_use]
    pub fn episode(&self) -> &Episode {
        &self.podcast.episodes[self.position]
    }

    /// Chapters that are entries of the list; silent marks are not.
    #[must_use]
    pub fn listed_chapters(&self) -> Vec<&Chapter> {
        self.chapters.iter().filter(|chapter| !chapter.hidden).collect()
    }
}

#[derive(Debug)]
pub struct App {
    pub lang: Lang,
    pub settings: Settings,
    pub section: Section,
    pub tab: Tab,
    pub search: Search,
    pub charts: Charts,
    pub categories: Categories,
    pub podcast: Option<PodcastView>,
    pub episode: Option<EpisodeView>,
    pub settings_index: usize,
    /// A sentence for the bottom of the content area; gone with the next key.
    pub notice: Option<String>,
    /// The directories a search reaches right now, told by whoever owns the core.
    pub providers: Vec<ProviderId>,
    requests: u64,
    // What the main loop has to do on the app's behalf.
    pub commands: Vec<Command>,
    pub open_urls: Vec<String>,
    pub settings_changed: bool,
    pub should_quit: bool,
}

impl App {
    #[must_use]
    pub fn new(lang: Lang, settings: Settings) -> Self {
        let mut providers = vec![ProviderId::Apple];
        if settings.sources.fyyd {
            providers.push(ProviderId::Fyyd);
        }
        Self {
            lang,
            settings,
            section: Section::Discover,
            tab: Tab::Search,
            search: Search {
                input: String::new(),
                // A search tool that starts ready to be typed into.
                editing: true,
                typed_at: None,
                sent: String::new(),
                request: 0,
                pending: Vec::new(),
                batches: Vec::new(),
                failures: Vec::new(),
                results: Vec::new(),
                index: 0,
            },
            charts: Charts {
                request: 0,
                category: None,
                list: Vec::new(),
                index: 0,
                load: Load::Idle,
            },
            categories: Categories {
                list: Vec::new(),
                index: 0,
                load: Load::Idle,
            },
            podcast: None,
            episode: None,
            settings_index: 0,
            notice: None,
            providers,
            requests: 0,
            commands: Vec::new(),
            open_urls: Vec::new(),
            settings_changed: false,
            should_quit: false,
        }
    }

    fn next_request(&mut self) -> u64 {
        self.requests += 1;
        self.requests
    }

    /// Which screen is up — equal values mean the same screen, whatever its content.
    #[must_use]
    pub fn screen(&self) -> (Section, Tab, bool, bool) {
        (self.section, self.tab, self.podcast.is_some(), self.episode.is_some())
    }

    /// Whether characters typed right now go into a text field.
    #[must_use]
    pub fn is_typing(&self) -> bool {
        match self.section {
            Section::Discover if self.episode.is_some() => false,
            Section::Discover => match &self.podcast {
                Some(view) => view.filtering,
                None => self.tab == Tab::Search && self.search.editing,
            },
            _ => false,
        }
    }

    // ── time ────────────────────────────────────────────────────────────────

    /// Called regularly; starts the search once the typing has paused.
    pub fn tick(&mut self, now: Instant) {
        let due = self
            .search
            .typed_at
            .is_some_and(|typed| now.duration_since(typed) >= SEARCH_DELAY);
        if due {
            self.start_search();
        }
    }

    fn start_search(&mut self) {
        self.search.typed_at = None;
        let query = self.search.input.trim().to_owned();
        if query == self.search.sent || query.chars().count() < MIN_QUERY {
            return;
        }
        // An address is not a search: it is the feed itself.
        if query.starts_with("http://") || query.starts_with("https://") {
            self.search.editing = false;
            let reference = PodcastRef {
                title: query.clone(),
                feed_url: Some(query),
                ..PodcastRef::default()
            };
            self.open_podcast(reference);
            return;
        }
        let request = self.next_request();
        self.search.request = request;
        self.search.sent.clone_from(&query);
        self.search.pending.clone_from(&self.providers);
        self.search.batches.clear();
        self.search.failures.clear();
        self.search.results.clear();
        self.search.index = 0;
        self.commands.push(Command::Search { request, query });
    }

    // ── events from the core ────────────────────────────────────────────────

    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::SearchBatch {
                request,
                provider,
                outcome,
            } if request == self.search.request => {
                self.search.pending.retain(|pending| *pending != provider);
                match outcome {
                    Ok(batch) => self.search.batches.push((provider, batch)),
                    Err(problem) => self.search.failures.push((provider, problem)),
                }
                // Apple first, whoever answered first: the order must not jump.
                self.search.batches.sort_by_key(|(provider, _)| *provider);
                let selected = self.search.results.get(self.search.index).cloned();
                let batches: Vec<Vec<PodcastRef>> =
                    self.search.batches.iter().map(|(_, batch)| batch.clone()).collect();
                self.search.results = merge(&batches);
                self.search.index = selected
                    .and_then(|selected| {
                        self.search
                            .results
                            .iter()
                            .position(|result| same_show(result, &selected))
                    })
                    .unwrap_or(0);
            }
            Event::Charts { request, outcome } if request == self.charts.request => match outcome {
                Ok(list) => {
                    self.charts.list = list;
                    self.charts.index = 0;
                    self.charts.load = Load::Ready;
                }
                Err(problem) => self.charts.load = Load::Failed(problem),
            },
            Event::Categories { outcome } => match outcome {
                Ok(list) => {
                    self.categories.list = list;
                    self.categories.load = Load::Ready;
                }
                Err(problem) => self.categories.load = Load::Failed(problem),
            },
            Event::Feed { request, outcome } => {
                if let Some(view) = self.podcast.as_mut().filter(|view| view.request == request) {
                    match outcome {
                        Ok(podcast) => {
                            view.podcast = Some(podcast);
                            view.load = Load::Ready;
                            view.index = 0;
                        }
                        Err(problem) => view.load = Load::Failed(problem),
                    }
                }
            }
            Event::Chapters { request, chapters } => {
                if let Some(view) = self.episode.as_mut().filter(|view| view.request == request) {
                    view.looking = None;
                    view.chapters = merge_chapters(std::mem::take(&mut view.chapters), chapters);
                }
            }
            // An answer to a question nobody is asking any more.
            Event::SearchBatch { .. } | Event::Charts { .. } => {}
        }
    }

    // ── navigation ──────────────────────────────────────────────────────────

    fn open_podcast(&mut self, reference: PodcastRef) {
        let Some(feed_url) = reference.feed_url.clone() else {
            self.notice = Some(
                self.lang
                    .t("This podcast has no open feed. It can only be heard inside the platform that hosts it.")
                    .to_owned(),
            );
            return;
        };
        let request = self.next_request();
        self.commands.push(Command::OpenFeed {
            request,
            feed_url,
            reload: false,
        });
        self.podcast = Some(PodcastView {
            reference,
            request,
            load: Load::Loading,
            podcast: None,
            index: 0,
            newest_first: true,
            filter: String::new(),
            filtering: false,
            expanded: false,
        });
    }

    fn open_episode(&mut self) {
        let Some(view) = &self.podcast else { return };
        let (Some(podcast), Some(position)) = (view.podcast.clone(), view.visible().get(view.index).copied()) else {
            return;
        };
        let episode = &podcast.episodes[position];
        let notes = episode.notes_html.as_deref().map(notes::document).unwrap_or_default();
        let chapters = episode.chapters.clone();
        let chapters_url = episode.chapters_url.clone();
        // The audio file is only asked when nothing else promises chapters.
        let mp3_url = episode
            .enclosure
            .as_ref()
            .filter(|enclosure| enclosure.is_mp3() && !episode.announces_chapters())
            .map(|enclosure| enclosure.url.clone());
        let looking = if chapters_url.is_some() {
            Some(Looking::ChaptersFile)
        } else if mp3_url.is_some() {
            Some(Looking::AudioFile)
        } else {
            None
        };
        let request = self.next_request();
        if looking.is_some() {
            self.commands.push(Command::Chapters {
                request,
                chapters_url,
                mp3_url,
            });
        }
        self.episode = Some(EpisodeView {
            podcast,
            position,
            notes,
            chapters,
            looking,
            request,
            focus: Focus::Notes,
            scroll: 0,
            chapter_index: 0,
        });
    }

    fn load_charts(&mut self, category: Option<Category>) {
        let request = self.next_request();
        self.charts.request = request;
        self.charts.load = Load::Loading;
        self.charts.list.clear();
        self.commands.push(Command::Charts {
            request,
            category: category.as_ref().map(|category| category.id),
        });
        self.charts.category = category;
    }

    fn enter_tab(&mut self, tab: Tab) {
        self.tab = tab;
        match tab {
            Tab::Charts if self.charts.load == Load::Idle => self.load_charts(None),
            Tab::Categories if self.categories.load == Load::Idle => {
                self.categories.load = Load::Loading;
                self.commands.push(Command::Categories);
            }
            _ => {}
        }
    }

    fn cycle_tab(&mut self, step: isize) {
        let position = Tab::ALL.iter().position(|tab| *tab == self.tab).unwrap_or(0);
        let next = (position as isize + step).rem_euclid(Tab::ALL.len() as isize) as usize;
        self.enter_tab(Tab::ALL[next]);
    }

    // ── keys ────────────────────────────────────────────────────────────────

    pub fn on_key(&mut self, key: KeyEvent) {
        self.notice = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c' | 'q') => self.should_quit = true,
                KeyCode::Char('u') if self.is_typing() => self.edit(|text| text.clear()),
                _ => {}
            }
            return;
        }
        if self.is_typing() {
            self.on_typing_key(key.code);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.section = Section::Help,
            // Inside an episode the digits belong to the links.
            KeyCode::Char(digit @ '1'..='3') if !(self.section == Section::Discover && self.episode.is_some()) => {
                self.section = Section::ALL[digit as usize - '1' as usize];
            }
            code => match self.section {
                Section::Discover => self.on_discover_key(code),
                Section::Settings => self.on_settings_key(code),
                Section::Help => {
                    if matches!(code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Left) {
                        self.section = Section::Discover;
                    }
                }
            },
        }
    }

    /// Applies `change` to whichever text field has the keyboard.
    fn edit(&mut self, change: impl FnOnce(&mut String)) {
        if let Some(view) = self.podcast.as_mut().filter(|view| view.filtering) {
            change(&mut view.filter);
            view.index = 0;
        } else {
            change(&mut self.search.input);
            self.search.typed_at = Some(Instant::now());
        }
    }

    fn on_typing_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(character) => self.edit(|text| text.push(character)),
            KeyCode::Backspace => self.edit(|text| {
                text.pop();
            }),
            KeyCode::Enter | KeyCode::Down | KeyCode::Esc => {
                if let Some(view) = self.podcast.as_mut().filter(|view| view.filtering) {
                    view.filtering = false;
                    if code == KeyCode::Esc {
                        view.filter.clear();
                    }
                } else {
                    self.search.editing = false;
                    if code == KeyCode::Enter {
                        self.start_search();
                    }
                }
            }
            KeyCode::Tab if self.podcast.is_none() => {
                self.search.editing = false;
                self.cycle_tab(1);
            }
            KeyCode::BackTab if self.podcast.is_none() => {
                self.search.editing = false;
                self.cycle_tab(-1);
            }
            _ => {}
        }
    }

    fn on_discover_key(&mut self, code: KeyCode) {
        if self.episode.is_some() {
            self.on_episode_key(code);
        } else if self.podcast.is_some() {
            self.on_podcast_key(code);
        } else {
            self.on_root_key(code);
        }
    }

    fn on_root_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('/') => {
                self.tab = Tab::Search;
                self.search.editing = true;
            }
            KeyCode::Tab => self.cycle_tab(1),
            KeyCode::BackTab => self.cycle_tab(-1),
            KeyCode::Char('r') => match self.tab {
                Tab::Charts => self.load_charts(self.charts.category.clone()),
                Tab::Categories => {
                    self.categories.load = Load::Loading;
                    self.commands.push(Command::Categories);
                }
                Tab::Search => {
                    self.search.sent.clear();
                    self.start_search();
                }
            },
            KeyCode::Esc | KeyCode::Backspace if self.tab == Tab::Charts && self.charts.category.is_some() => {
                self.load_charts(None)
            }
            KeyCode::Up | KeyCode::Char('k') if self.tab == Tab::Search && self.search.index == 0 => {
                self.search.editing = true
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => match self.tab {
                Tab::Search => {
                    if let Some(reference) = self.search.results.get(self.search.index).cloned() {
                        self.open_podcast(reference);
                    }
                }
                Tab::Charts => {
                    if let Some(reference) = self.charts.list.get(self.charts.index).cloned() {
                        self.open_podcast(reference);
                    }
                }
                Tab::Categories => {
                    if let Some(category) = self.categories.list.get(self.categories.index).cloned() {
                        self.load_charts(Some(category));
                        self.tab = Tab::Charts;
                    }
                }
            },
            code => {
                let (index, count) = match self.tab {
                    Tab::Search => (&mut self.search.index, self.search.results.len()),
                    Tab::Charts => (&mut self.charts.index, self.charts.list.len()),
                    Tab::Categories => (&mut self.categories.index, self.categories.list.len()),
                };
                move_selection(index, count, code);
            }
        }
    }

    fn on_podcast_key(&mut self, code: KeyCode) {
        let Some(view) = self.podcast.as_mut() else { return };
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.podcast = None,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.open_episode(),
            KeyCode::Char('/') => view.filtering = true,
            KeyCode::Char('o') => {
                view.newest_first = !view.newest_first;
                view.index = 0;
            }
            KeyCode::Char('m') => view.expanded = !view.expanded,
            KeyCode::Char('w') => {
                let website = view
                    .podcast
                    .as_ref()
                    .and_then(|podcast| podcast.website.clone())
                    .or_else(|| view.reference.website.clone());
                self.open_urls.extend(website);
            }
            KeyCode::Char('r') => {
                if let Some(feed_url) = view.reference.feed_url.clone() {
                    view.load = Load::Loading;
                    let request = view.request;
                    self.commands.push(Command::OpenFeed {
                        request,
                        feed_url,
                        reload: true,
                    });
                }
            }
            code => {
                let count = view.visible().len();
                move_selection(&mut view.index, count, code);
            }
        }
    }

    fn on_episode_key(&mut self, code: KeyCode) {
        let Some(view) = self.episode.as_mut() else { return };
        let chapters = view.listed_chapters().len();
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.episode = None,
            KeyCode::Tab | KeyCode::BackTab if chapters > 0 => {
                view.focus = if view.focus == Focus::Notes {
                    Focus::Chapters
                } else {
                    Focus::Notes
                };
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let link = view.notes.links.get(digit as usize - '1' as usize).cloned();
                self.open_urls.extend(link);
            }
            KeyCode::Char('w') => {
                let link = view.episode().link.clone();
                self.open_urls.extend(link);
            }
            KeyCode::Enter if view.focus == Focus::Chapters => {
                let link = view
                    .listed_chapters()
                    .get(view.chapter_index)
                    .and_then(|chapter| chapter.url.clone());
                self.open_urls.extend(link);
            }
            code if view.focus == Focus::Chapters => move_selection(&mut view.chapter_index, chapters, code),
            // The end of the notes is known only to the drawing code, which clamps.
            KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
            KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
            KeyCode::PageDown | KeyCode::Char(' ') => view.scroll = view.scroll.saturating_add(10),
            KeyCode::Home | KeyCode::Char('g') => view.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => view.scroll = u16::MAX,
            _ => {}
        }
    }

    /// The rows of the settings: 0 Apple, 1 Podcast Index, 2 fyyd, 3 country.
    pub const SETTINGS_ROWS: usize = 4;

    fn on_settings_key(&mut self, code: KeyCode) {
        match (self.settings_index, code) {
            (2, KeyCode::Char(' ') | KeyCode::Enter) => {
                self.settings.sources.fyyd = !self.settings.sources.fyyd;
                self.settings_changed = true;
            }
            (3, KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')) => self.cycle_country(1),
            (3, KeyCode::Left | KeyCode::Char('h')) => self.cycle_country(-1),
            (_, code) => move_selection(&mut self.settings_index, Self::SETTINGS_ROWS, code),
        }
    }

    fn cycle_country(&mut self, step: isize) {
        let position = COUNTRIES.iter().position(|country| *country == self.settings.country);
        let next = position.map_or(0, |position| {
            (position as isize + step).rem_euclid(COUNTRIES.len() as isize) as usize
        });
        self.settings.country = COUNTRIES[next].to_owned();
        self.settings_changed = true;
        // Charts and categories belong to the country they were loaded for.
        self.charts = Charts {
            request: 0,
            category: None,
            list: Vec::new(),
            index: 0,
            load: Load::Idle,
        };
        self.categories = Categories {
            list: Vec::new(),
            index: 0,
            load: Load::Idle,
        };
        self.search.sent.clear();
    }

    // ── mouse ───────────────────────────────────────────────────────────────

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.on_wheel(KeyCode::Up),
            MouseEventKind::ScrollDown => self.on_wheel(KeyCode::Down),
            MouseEventKind::Down(MouseButton::Left) if mouse.column < MENU_WIDTH => {
                let entry = usize::from(mouse.row.saturating_sub(MENU_FIRST_ROW));
                if mouse.row >= MENU_FIRST_ROW && entry < Section::ALL.len() {
                    self.section = Section::ALL[entry];
                }
            }
            _ => {}
        }
    }

    fn on_wheel(&mut self, code: KeyCode) {
        match self.section {
            Section::Discover => {
                // The wheel moves what is shown, never the text being typed.
                if self.search.editing && self.podcast.is_none() {
                    self.search.editing = false;
                }
                self.on_discover_key(code);
            }
            Section::Settings => self.on_settings_key(code),
            Section::Help => {}
        }
    }
}

fn same_show(left: &PodcastRef, right: &PodcastRef) -> bool {
    (left.itunes_id.is_some() && left.itunes_id == right.itunes_id)
        || (left.feed_url.is_some() && left.feed_url == right.feed_url)
}

fn move_selection(index: &mut usize, count: usize, code: KeyCode) {
    let last = count.saturating_sub(1);
    *index = match code {
        KeyCode::Up | KeyCode::Char('k') => index.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => index.saturating_add(1),
        KeyCode::PageUp => index.saturating_sub(10),
        KeyCode::PageDown => index.saturating_add(10),
        KeyCode::Home | KeyCode::Char('g') => 0,
        KeyCode::End | KeyCode::Char('G') => last,
        _ => *index,
    }
    .min(last);
}
