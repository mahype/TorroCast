# Quellen für Podcast-Katalogdaten

Stand: 19.09.2026. Alle Live-Tests liefen an diesem Tag per curl. Aufgezeichnete
Antworten liegen unter [`fixtures/`](fixtures/) und eignen sich als Test-Vorlagen.

> **Entscheidung vom 19.09.2026:** Abweichend von der Empfehlung unten ist **Apple die
> Hauptquelle**. Podcast Index und fyyd schaltet der Nutzer selbst dazu; für Podcast
> Index trägt er seinen eigenen Key ein. TorroCast liefert keinen Key mit. Siehe
> [architektur.md](../architektur.md).

## Empfehlung der Recherche

- **Primärquelle: Podcast Index.** Offen, gezielt für Podcast-Apps gebaut, liefert
  immer die Feed-URL und die `podcastGuid` und deckt Podcasting 2.0 ab. Trending und
  Kategorien sind enthalten, dazu gibt es einen SQLite-Dump für Offline-Betrieb.
- **Sekundärquelle: Apple iTunes Search/Lookup plus Charts-Feeds.** Apple braucht
  keinen Key, hat die beste Suchrelevanz und bietet die einzigen echten
  Länder-Charts. `feedUrl` kommt mit.
- **Tertiärquelle: fyyd, optional.** Gut für deutschsprachige Inhalte, Kurationen und
  Episodensuche. Ebenfalls ohne Key, aber ein Ein-Personen-Projekt ohne auffindbare
  Nutzungsbedingungen.
- **Immer vorhanden: OPML-Import und direkte Feed-URL.** Unabhängig von allen
  Providern.
- **Nicht als Directory geeignet:**
  - gpodder.net: die Suche liefert unbrauchbare Treffer. Als Sync-Dienst bleibt es
    interessant.
  - Listen Notes: im Free-Plan kein RSS-Feld und Caching-Verbot.
  - Spotify: liefert nie eine Feed-URL.
  - YouTube: kein RSS-Katalog.
  - Taddy, Podchaser, Rephonic, Podseeker: bezahlte B2B-Dienste.
- **Key-Frage bei Podcast Index:** Die Nutzungsbedingungen verbieten wörtlich,
  Zugangsdaten in Open-Source-Projekte einzubetten. AntennaPod und Kasts tun es
  trotzdem. Das sollte vor dem Release per Mail mit Podcast Index geklärt werden.
  Technisch: Key zur Build-Zeit einspeisen und eine Nutzer-Konfiguration als
  Überschreibung anbieten.

---

## 1. Apple: iTunes Search/Lookup API, Charts, Genres

| Punkt | Befund |
|---|---|
| URLs | Suche: `https://itunes.apple.com/search?media=podcast&term=…&country=DE`. Lookup: `https://itunes.apple.com/lookup?id=…`, auch kommagetrennt mehrere IDs und mit `&entity=podcastEpisode`. |
| Auth | Keine. Live: HTTP 200 ohne Key, CORS `access-control-allow-origin: *`. |
| Kosten | Keine. |
| Rate Limit | „approximately 20 calls per minute (subject to change)“. Apple empfiehlt Caching und verweist für höheren Bedarf auf den Enterprise Partner Feed. Quelle: https://performance-partners.apple.com/search-api |
| Feed-URL | **Ja**, `feedUrl` kommt in Suche und Lookup zurück. Bei exklusiven oder Abo-Shows fehlt das Feld erfahrungsgemäß; nicht getestet. |
| Bedingungen | Die Doku ist auf **Promo- und Affiliate-Nutzung** zugeschnitten: Inhalte dürfen nur „promotional“ verwendet werden, nahe einem Apple-Badge platziert, mit „provided courtesy of iTunes“ bei Previews (Quelle wie oben). Eine ausdrückliche Erlaubnis für Podcast-Clients gibt es nicht. In der Praxis nutzen praktisch alle OSS-Clients die API. AntennaPod hat `ItunesPodcastSearcher.java` und `ItunesTopListLoader.java`: https://github.com/AntennaPod/AntennaPod/tree/develop/net/discovery/src/main/java/de/danoeh/antennapod/net/discovery. Das ist eine Grauzone: geduldet, aber ohne Rechtsanspruch und ohne zugesicherte Verfügbarkeit. |
| Charts (neu) | `https://rss.marketingtools.apple.com/api/v2/{cc}/podcasts/top/{10\|25\|50\|100}/podcasts.json`, auch `podcast-episodes.json`. Formate: JSON, RSS, Atom. Live 200 für DE. Der Eintrag enthält **kein** `feedUrl` und erlaubt **keinen Genre-Filter**. `top-subscriber` liefert 404. Auf der Seite stehen keine Nutzungsbedingungen. Quelle: https://rss.marketingtools.apple.com/ |
| Charts (alt) | `https://itunes.apple.com/{cc}/rss/toppodcasts/limit={1..200}/genre={id}/json` **funktioniert weiterhin**. Live 200, limit=200 liefert 200 Einträge, ein Genre-Filter ist möglich. Der Endpunkt ist undokumentiert und kann jederzeit verschwinden. |
| Genres | `https://itunes.apple.com/WebObjects/MZStoreServices.woa/ws/genres?id=26&cc=de` liefert den lokalisierten Genre-Baum. Live: „Podcasts“ (26) mit 19 Hauptgenres samt Subgenres, z. B. 1301 Kunst, 1321 Wirtschaft, 1303 Comedy, 1489 Nachrichten, 1527 Politik. Undokumentiert. |
| Charts zu Feeds | Die Charts liefern nur iTunes-IDs. Ein Batch-Lookup `lookup?id=1700432142,1092957894` löst sie zu `feedUrl` auf (live getestet, funktioniert). |

**Live-Beispiel Suche „lage der nation“ (gekürzt):**

```json
{"kind":"podcast","collectionId":1092957894,"artistName":"Philip Banse & Ulf Buermeyer",
 "collectionName":"Lage der Nation - der Politik-Podcast aus Berlin",
 "feedUrl":"https://feeds.lagedernation.org/feeds/ldn-mp3.xml",
 "collectionViewUrl":"https://podcasts.apple.com/de/podcast/…/id1092957894?uo=4",
 "artworkUrl30/60/100/600":"https://is1-ssl.mzstatic.com/…/600x600bb.jpg",
 "releaseDate":"2026-09-19T10:33:00Z","trackCount":497,"country":"DEU",
 "primaryGenreName":"Politik","genreIds":["1527","26","1489","1324"],
 "genres":["Politik","Podcasts","Nachrichten","Gesellschaft und Kultur"],
 "collectionExplicitness":"notExplicit","contentAdvisoryRating":"Clean"}
```

Die Suche liefert keine Beschreibung und keine Sprache. Ein Lookup mit
`entity=podcastEpisode` liefert `episodeGuid`, `episodeUrl`, `trackTimeMillis`,
`shortDescription`, `releaseDate`, `feedUrl`.

Felder pro Charts-Eintrag: `artistName, id, name, kind, artworkUrl100,
genres[{genreId,name,url}], url`.

Nicht verifiziert: ob Apple eine neue offizielle Podcasts-Katalog-API für Dritte
anbietet. Eine Abkündigung der iTunes Search API wurde nicht gefunden.

---

## 2. Podcast Index (podcastindex.org)

| Punkt | Befund |
|---|---|
| URL | `https://api.podcastindex.org/api/1.0`. Doku und OpenAPI-Spec v1.12.1: https://podcastindex-org.github.io/docs-api/ (`pi_api.json`, Kopie unter `fixtures/`). |
| Auth | Vier Pflicht-Header: `User-Agent`, `X-Auth-Key`, `X-Auth-Date` (Unix-Zeit, 3-Minuten-Fenster) und `Authorization = sha1(key + secret + unixTime)` als Hex in Kleinbuchstaben. Live: ohne Auth kommt 401. Generische User-Agents werden mit **403** abgewiesen. `TorroCast/0.1 (https://github.com/…)` wurde akzeptiert. Stolperfalle: Das Secret kann `$` enthalten, was Shell- und .env-Escaping nötig macht. |
| Kosten | Kostenlos: „Register for a free API key at https://api.podcastindex.org/“. Das Projekt ist spendenfinanziert. |
| Rate Limit | **Bewusst nicht veröffentlicht.** Dave Jones, 2020: bei Dauerlast unter 1 Request pro Sekunde bleiben; Search-as-you-type ist mit Debounce in Ordnung (https://github.com/Podcastindex-org/docs-api/issues/16). 2024: „We don't publish rate limits because we have no SLA's or guarantees… free project that runs on donations“. Bei Überschreitung kommt HTTP 429 (https://github.com/Podcastindex-org/docs-api/issues/30). |
| Abdeckung | Live aus https://stats.podcastindex.org/daily_counts.json: **4.727.989 Feeds, 167.090.147 Episoden**, davon 340.871 Feeds mit neuer Episode in den letzten 30 Tagen. |
| Feed-URL | **Ja, immer** (`url`, `originalUrl`). Dazu kommen `podcastGuid`, `itunesId`, `language`, `categories`, `medium`, `locked`, `dead` und `imageUrlHash`. |
| Endpunkte | Alle unten genannten Endpunkte stehen in der Spec; keiner wurde authentifiziert aufgerufen, weil noch kein eigener Key vorliegt. |
| Episoden pro Feed | `/episodes/byfeedid` ist möglich, aber für einen Client ist das direkte Parsen des RSS-Feeds vorzuziehen. Das entlastet Podcast Index und entspricht dem Rat von Dave Jones vom 06.05.2026 in Issue #30: Abo-Datenbank auf dem Gerät halten, Podcast Index und Apple nur für Suche und Abonnieren nutzen, „Castamatic/Antennapod are good examples“. |

**Endpunkte laut Spec:**

- Suche: `/search/byterm` (Parameter `q, val, max, aponly, clean, similar, fulltext`), `/search/bytitle`, `/search/byperson`, `/search/music/byterm`
- Podcasts: `/podcasts/byfeedid`, `/byfeedurl`, `/byitunesid`, `/byguid`, `/bytag`, `/bymedium`, `/trending` (Parameter `max, since, lang, cat, notcat`), `/dead`, `/batch/byguid`
- Episoden: `/episodes/byfeedid`, `/byfeedurl`, `/bypodcastguid`, `/byitunesid`, `/byid`, `/byguid`, `/live`, `/random`
- Aktuelles: `/recent/episodes`, `/feeds`, `/newfeeds`, `/newvaluefeeds`, `/data`, `/soundbites`
- Value: `/value/byfeedid`, `/byfeedurl`, `/bypodcastguid`, `/byepisodeguid`, `/batch/byepisodeguid`
- Sonstiges: `/categories/list`, `/stats/current`, `/hub/pubnotify`, `/add/byfeedurl`, `/add/byitunesid`

**Podcasting 2.0 in der API (Schema-Analyse der Spec):**

- Vorhanden: `transcripts`/`transcriptUrl`, `chaptersUrl`, `persons`, `soundbites`, `funding`, `value` (Value-Blocks), `socialInteract`, `liveItem` (`/episodes/live`), `medium`, `locked`, `podcastGuid`.
- **Nicht in der Spec:** `location`, `podroll`, `alternateEnclosure`. Diese Tags muss TorroCast selbst aus dem Feed parsen. Der eigene Feed-Parser ist ohnehin nötig; die API dient der Entdeckung, nicht als Metadaten-Ersatz.

**SQLite-Dump:**

- URL: `https://public.podcastindex.org/podcastindex_feeds.db.tgz`. Live 200 mit nicht-generischem UA, 403 mit curl-Standard-UA.
- Größe: **1,83 GB gepackt**.
- Die Spec sagt „Updated daily“, aber der `last-modified`-Header lautete am Testtag 12.09.2026, also eher wöchentlich.
- Inhalt: alle nicht-toten Feeds, „Some attributes excluded. No episodes included.“ Auch per IPFS erhältlich.
- Das Repo `Podcastindex-org/database` steht unter MIT (https://github.com/Podcastindex-org/database). Eine explizite Datenlizenz für den Dump wurde nicht gefunden.
- Für einen Desktop-Client ist der Dump zu groß zum Ausliefern. Als optionaler Offline-Suchindex, vom Nutzer selbst geladen und lokal per FTS5 durchsucht, ist er brauchbar.

**Nutzungsbedingungen (v1.1, 02.03.2021):** https://github.com/Podcastindex-org/legal/blob/main/TermsOfService.md

- §4.2.1, wörtlich: „You will keep your credentials confidential… **Developer credentials may not be embedded in open source projects.**“
- §5 verbietet, permanente Kopien der API-Inhalte anzulegen oder länger zu cachen, als der Cache-Header erlaubt.
- §7.2: Attributionen gemäß Doku anzeigen. Eine konkrete Attributionspflicht nennt die Doku nicht.
- §2.4: Limits liegen im Ermessen von Podcast Index.

**Wie andere OSS-Clients mit dem Key umgehen (per GitHub verifiziert):**

- **AntennaPod:** `net/discovery/build.gradle` setzt `PODCASTINDEX_API_KEY` und `PODCASTINDEX_API_SECRET` als BuildConfig-Felder. Gradle-Properties überschreiben sie. **Andernfalls steht ein Standard-Key samt Secret im Klartext im öffentlichen Repo.** https://github.com/AntennaPod/AntennaPod/blob/develop/net/discovery/build.gradle
- **Kasts (KDE):** Key und Secret sind **hart im C++-Quelltext** einkompiliert (`src/models/podcastsearchmodel.cpp`). Podcast Index ist dort die einzige Suchquelle. https://github.com/KDE/kasts/blob/master/src/models/podcastsearchmodel.cpp
- **Podverse:** Keys liegen serverseitig im eigenen Backend. Der Client spricht mit der Podverse-API, nicht direkt mit Podcast Index.
- **gPodder (Desktop):** keine Quelltext-Treffer für „podcastindex“ oder „itunes.apple.com“; hängt klassisch an gpodder.net. Nicht tiefer geprüft.

**Bewertung:** Wortlaut und gelebte Praxis widersprechen sich. Dave Jones nennt
AntennaPod ausdrücklich als Vorbild, obwohl dort der Key im Repo liegt. Vorgehen:

1. Einen eigenen Key „TorroCast“ registrieren und Podcast Index per Mail um ein kurzes schriftliches OK bitten.
2. Den Key wie AntennaPod zur Build-Zeit einspeisen (Umgebungsvariable oder `build.rs`), sodass Distributions-Pakete eigene Keys setzen können.
3. Eine Nutzer-Überschreibung in der Config vorsehen.
4. Wenn kein Key konfiguriert ist, deaktiviert sich der Podcast-Index-Provider sauber und Apple oder fyyd springen ein.

---

## 3. fyyd.de

| Punkt | Befund |
|---|---|
| Status | **Läuft.** Live: `https://api.fyyd.de/0.2/search/podcast?title=lage+der+nation` liefert 200 mit `"msg":"ok"`. Der Crawler ist aktiv. Hobbyprojekt einer einzelnen Person (Christian Bednarek) seit 2016; ein Server. |
| Doku | Das offizielle Repo `github.com/eazyliving/fyyd-api` liefert **404**, ist also gelöscht oder privat. Ein Fork der Doku (v0.15, 22.09.2020) liegt unter https://github.com/deda9/fyyd-api. Laut Doku ist API-Version 0.2 eingefroren und es kommen nur additive Änderungen. |
| Auth | Lesende Endpunkte brauchen keine Auth. OAuth2 ist nur für Account, Kurationen und Aktionen nötig. |
| Kosten, Rate Limit | Kostenlos. Ein Rate Limit ist **nicht dokumentiert** und es kommen keine RateLimit-Header. Die Antworten tragen `Cache-Control: no-store`. |
| Nutzungsbedingungen | **Nicht auffindbar.** `fyyd.de/terms` liefert 500, `/about` und `/dev/api` liefern 404. Für dauerhafte Nutzung den Betreiber fragen. |
| Feed-URL | **Ja**, im Feld `xmlURL`. |
| Abdeckung | Aktuelle Gesamtzahl nicht ermittelbar. Eigenes Ranking im Feld `rank`. |

**Live getestete Endpunkte, alle 200:**

- `/search/podcast?title=&count=`
- `/search/episode?title=`
- `/search/curation?term=`
- `/podcast?podcast_id=`
- `/podcast/latest`
- `/categories` (Baum mit `name` und `name_de`)
- `/category?category_id=&count=`
- `/feature/podcast/hot?language=de` (der Trending-Ersatz; `/podcast/hot` liefert 404)

**Live-Beispiel (gekürzt):**

```json
{"title":"Lage der Nation - der Politik-Podcast aus Berlin","id":44980,
 "xmlURL":"https://feeds.lagedernation.org/feeds/ldn-mp3.xml","htmlURL":"https://lagedernation.org",
 "imgURL":"…","layoutImageURL":"https://img-1.fyyd.de/pd/layout/….jpg","thumbImageURL":"…",
 "status":200,"status_since":"2026-04-17 12:00:07","slug":"lage-der-nation","language":"de",
 "generator":"Podlove Podcast Publisher v4.5.6","categories":[155,160,181],
 "lastpub":"2026-09-19T12:33:32+02:00","rank":640,"episode_count":509,
 "description":"…","subtitle":"…","author":"Philip Banse & Ulf Buermeyer",
 "paymentURL":"https://plus.lagedernation.org/","color":"#1c3b83"}
```

Episodenfelder: `title, id, guid, url, enclosure, podcast_id, imgURL, pubdate,
duration, num_season, num_episode`.

AntennaPod nutzt fyyd als einen von drei Suchanbietern (`FyydPodcastSearcher.java`).

---

## 4. gpodder.net

| Punkt | Befund |
|---|---|
| URLs | `https://gpodder.net/search.json?q=`, `/toplist/{1..100}.{json\|xml\|opml\|txt}`, `/api/2/tags/{n}.json`, `/api/2/tag/{tag}/{n}.json`, `/api/2/data/podcast.json?url=`. Doku: https://gpoddernet.readthedocs.io/en/latest/api/reference/directory.html |
| Auth, Kosten | Für die Directory-API keine. Kostenlos. Ein Rate Limit ist nicht dokumentiert. |
| Status live | Erreichbar: Suche 200 in 1,2 s, Toplist 200. |
| Feed-URL | Ja (`url`), dazu `subscribers` und `logo_url`. |
| Zuverlässigkeit | Historisch häufige 5xx-Fehler und Ausfälle (https://github.com/gpodder/mygpo/issues/527, https://github.com/AntennaPod/AntennaPod/issues/4717). Issue #832 vom Mai 2025: „No Backups Since 2024-December“. |

**Datenqualität ist schlecht:** Die Suche „lage der nation“ liefert 16 Treffer, der
erste ist „Medien-KuH“. Auf Platz 1 der Toplist steht „Linux Outlaws“, ein seit
Jahren eingestellter Podcast.

**Fazit:** Als Directory nicht empfehlenswert. Das gpodder.net-**Sync-Protokoll**
bleibt als späteres Sync-Backend interessant, vor allem über selbst gehostete
kompatible Server wie oPodSync oder Nextcloud-gPodder.

---

## 5. Kommerzielle und sonstige Quellen

| Quelle | Auth, Kosten, Limits | Feed-URL? | Eignung für OSS-Desktop-Client |
|---|---|---|---|
| **Listen Notes** (https://www.listennotes.com/api/pricing/) | API-Key. FREE: **150 Requests pro Monat**, maximal 30 Suchergebnisse. PRO: 200 $/Monat für 5.000 Requests. | **Im Free-Plan nein.** `rss` nur in PRO und Enterprise (https://www.listennotes.com/api/faq/). | Ungeeignet. Free verbietet Caching und verlangt das Logo „Powered by Listen Notes“. |
| **Taddy** (https://taddy.org/developers/pricing) | Free: 500 Requests pro Monat. Pro: 75 $ für 100k. | Vermutlich ja (`rssUrl`, GraphQL). **Nicht verifiziert.** | Nur als Provider mit vom Nutzer eingetragenem Key denkbar. |
| **Podchaser** (https://www.podchaser.com/api) | GraphQL, OAuth-Client-Credentials. Preisangaben widersprüchlich, aktueller Stand nicht sicher verifiziert. | Laut Sekundärquellen ja. | B2B-Ausrichtung. Nicht sinnvoll. |
| **Spotify Web API** | OAuth Pflicht. Seit Februar 2026 stark eingeschränkt (https://developer.spotify.com/documentation/web-api/references/changes/february-2026). | **Nein.** Das Show-Objekt hat kein RSS-Feld. | Unbrauchbar für einen offenen RSS-Player. |
| **YouTube / YouTube Music** | Übernimmt RSS-Feeds nur in Richtung YouTube. Kein offener Katalog. | Nein. | Nicht relevant. |
| **Rephonic, Podseeker, Podscan** | B2B-Dienste für PR und Booking. | Teilweise. | Nicht relevant. |
| **Podcast Addict, Player FM, Pocket Casts, Overcast** | Keine öffentlichen, dokumentierten Directory-APIs. | – | Nein. |

**Abschaltungen und Veränderungen:**

- Stitcher wurde am 29.08.2023 eingestellt.
- **Google Podcasts wurde am 02.04.2024 eingestellt**, damit entfiel auch dessen Index.
- Castro war im Januar 2024 kurz vor dem Aus, wurde übernommen und läuft weiter.
- Spotify hat die API 2025 und 2026 stark eingeschränkt.
- Das fyyd-Doku-Repo ist von GitHub verschwunden.
- Ein neuer offener Katalog als Konkurrenz zu Podcast Index wurde nicht gefunden. Podcast Index bleibt der einzige große, offene und kostenlose Index.

---

## 6. OPML-Import und -Export

- OPML 2.0 (http://opml.org/spec2.opml) ist der De-facto-Standard für Abo-Listen. Apple Podcasts ist die bekannte Ausnahme ohne nativen Export.
- Relevante Attribute: `<outline type="rss" text="…" title="…" xmlUrl="…" htmlUrl="…">`. **`xmlUrl` ist das einzige verlässliche Feld.**
- Der Import sollte tolerant sein:
  - verschachtelte `outline`-Gruppen und Ordner rekursiv durchlaufen,
  - `type` kann fehlen oder großgeschrieben sein,
  - `xmlUrl`, `xmlurl` und `url` akzeptieren,
  - kaputtes XML, fehlerhafte Entities und BOM verkraften,
  - Duplikate über die normalisierte URL erkennen.
- Nach dem Import im Hintergrund anreichern: Feed abrufen, `podcast:guid` lesen, optional `podcasts/byfeedurl` bei Podcast Index abfragen.
- Der Export sollte zusätzlich `text`, `title` und `htmlUrl` setzen.
- OPML braucht keine API, hat keine Nutzungsbedingungen und funktioniert offline. Es sollte **immer** verfügbar sein, zusammen mit „Feed-URL direkt einfügen“.

---

## 7. Architektur: Provider-Abstraktion

| Rolle | Quelle | Begründung |
|---|---|---|
| **Primär** | **Podcast Index** | Offene Mission, 4,7 Mio. Feeds, immer mit Feed-URL, `podcastGuid` und `itunesId`. Trending nach Sprache und Kategorie filterbar. Personen-Suche, Podcasting-2.0-Daten. Risiko: Key-Klausel. |
| **Sekundär** | **Apple iTunes Search/Lookup plus Charts** | Ohne Key nutzbar, funktioniert also auch in Builds ohne Podcast-Index-Key. Beste Treffer bei bekannten Shows. **Einzige Quelle für echte Länder-Charts** und lokalisierte Genres. Risiken: 20 Requests pro Minute, Promo-Bedingungen, undokumentierte Alt-Endpunkte. |
| **Tertiär, optional** | **fyyd** | Stark im deutschsprachigen Indie-Bereich, Episodensuche, Kurationen, „hot“ nach Sprache. Risiko: Ein-Personen-Betrieb, kein SLA. Deshalb fehlertolerant und mit kurzem Timeout. |
| **Immer vorhanden** | **OPML und direkte Feed-URL** | Netz- und providerunabhängig. |
| **Opt-in mit Nutzer-Key** | Taddy, Listen Notes PRO | Nur mit eigenem Key des Nutzers. Kein Standard. |
| **Nicht einbauen** | gpodder.net als Directory, Spotify, YouTube, Podchaser | Gründe siehe oben. |

Dasselbe Muster nutzt AntennaPod mit `CombinedSearcher`, `PodcastSearcherRegistry`
und je einem Searcher für iTunes, fyyd und Podcast Index.

### Trait-Entwurf `DirectoryProvider`

```rust
bitflags! { pub struct Caps: u32 {
    const SEARCH_PODCASTS = 1; const SEARCH_EPISODES = 2; const SEARCH_PERSON = 4;
    const TOP_CHARTS = 8; const TRENDING = 16; const CATEGORIES = 32;
    const LOOKUP_BY_FEED_URL = 64; const LOOKUP_BY_GUID = 128; const LOOKUP_BY_ITUNES_ID = 256;
    const RECENT = 512; const LIVE = 1024; const OFFLINE = 2048;
}}

pub struct PodcastRef {               // normalisiertes Ergebnis
    pub feed_url: Option<Url>,        // None ⇒ muss via resolve() aufgelöst werden (Apple-Charts)
    pub podcast_guid: Option<Uuid>,   // podcast:guid (PI liefert, sonst aus Feed)
    pub itunes_id: Option<u64>,
    pub provider_ids: SmallVec<[(ProviderId, String); 2]>,
    pub title: String, pub author: Option<String>, pub description: Option<String>,
    pub artwork: Vec<Artwork>,
    pub language: Option<LanguageTag>, pub categories: Vec<Category>, // auf Apple-Taxonomie gemappt
    pub episode_count: Option<u32>, pub last_published: Option<DateTime<Utc>>,
    pub explicit: Option<bool>, pub medium: Option<Medium>, pub dead: bool,
    pub rank: Option<Rank>,
    pub sources: ProviderSet,         // welche Provider den Treffer lieferten
}

#[async_trait]
pub trait DirectoryProvider: Send + Sync {
    fn id(&self) -> ProviderId;                  // "podcastindex" | "apple" | "fyyd" | "opml" | …
    fn capabilities(&self) -> Caps;
    fn attribution(&self) -> Option<Attribution>; // UI-Pflichttext/Link ("Powered by …")
    fn policy(&self) -> RatePolicy;              // min-Intervall, Burst, Cache-TTL
    fn is_configured(&self) -> bool;             // z.B. PI ohne Key ⇒ false ⇒ wird übersprungen

    async fn search(&self, q: &SearchQuery) -> Result<Page<PodcastRef>, DirError>;
    async fn search_episodes(&self, q: &SearchQuery) -> Result<Page<EpisodeRef>, DirError>;
    async fn charts(&self, q: &ChartQuery) -> Result<Vec<PodcastRef>, DirError>;
    async fn categories(&self, locale: &Locale) -> Result<CategoryTree, DirError>;
    async fn resolve(&self, key: &PodcastKey) -> Result<Option<PodcastRef>, DirError>;
}
// Default-Impls liefern Err(DirError::Unsupported); Aufrufer prüft capabilities().

pub enum DirError { Unsupported, NotConfigured, RateLimited{retry_after: Option<Duration>},
                    Auth, Network(..), Decode(..), Upstream{status:u16} }
```

### Aggregator `CombinedDirectory`

1. **Parallel abfragen.** Alle konfigurierten Provider mit passender Capability
   parallel abfragen, mit Timeout pro Provider (etwa 4 s). Teilergebnisse gehen
   sofort als Stream an die Oberfläche. Ein ausgefallener Provider blockiert nie; der
   Fehler erscheint nur als Statuszeile.
2. **Rate-Limiter pro Provider** als Token Bucket aus `policy()`: Apple höchstens 20
   pro Minute, Podcast Index unter 1 pro Sekunde, fyyd konservativ 1 pro Sekunde.
   Search-as-you-type mit 300–500 ms Debounce, mindestens 3 Zeichen, In-Flight-Requests
   werden abgebrochen. Bei 429 oder 403 exponentielles Backoff.
3. **De-Duplikation mit Union-Find über Identitätsschlüssel**, in dieser Priorität:
   - `podcast_guid`: stabil über Feed-Umzüge hinweg.
   - `itunes_id`: Apple und Podcast Index liefern sie, fyyd nicht.
   - Normalisierte `feed_url`: Host kleinschreiben, Schema ignorieren, Standard-Port
     und abschließenden `/` entfernen, bekannte Tracking-Parameter entfernen. Im
     Zweifel nichts weiter entfernen. Feedburner und ähnliche Dienste **nicht**
     auflösen.
   - Schwacher Fallback nur zur Anzeige-Gruppierung, nie zum Zusammenführen von Abos:
     normalisierter Titel plus Autor.
4. **Felder zusammenführen** nach Priorität pro Feld:
   - `feed_url`: Podcast Index vor Apple vor fyyd. Nach dem ersten Feed-Abruf gewinnt
     immer der Feed selbst, inklusive `itunes:new-feed-url` und HTTP 301.
   - Artwork: Apple-600px vor Podcast Index vor fyyd.
   - Beschreibung und Sprache: Podcast Index oder fyyd, denn Apple liefert in der
     Suche keine.
   - Kategorien: auf die Apple-Genre-Taxonomie als kanonische Taxonomie abbilden.
5. **Ranking:** Reciprocal Rank Fusion über die Provider-Ränge
   (`score = Σ 1/(k + rank_p)`, k≈60). Zuschläge für exakte Titel-Treffer, für Treffer
   aus mehreren Quellen und für kürzlich aktualisierte Feeds. `dead`-Feeds abwerten.
6. **Cache** in SQLite im Core, Schlüssel `(provider, query-hash)`: Suche 1 Stunde,
   Charts und Trending 6–12 Stunden, Kategorien 7 Tage, `resolve` 24 Stunden. Das
   respektiert das Podcast-Index-Verbot von Dauerkopien. Abonnierte Podcasts leben
   danach **ausschließlich vom RSS-Feed**; Provider dienen nur der Entdeckung.
7. **Charts-Strategie:** `kind=Top` geht an Apple (marketingtools-v2, alter Endpunkt
   als Fallback für Genre-Filter und 200 Einträge), IDs per Batch-Lookup zu `feed_url`
   auflösen. `kind=Trending` geht an Podcast Index (`lang=de`), `kind=Hot` an fyyd. In
   der Oberfläche als getrennte Reiter, nicht vermischt.
8. **Konfiguration:** `[directory] providers = ["podcastindex","apple","fyyd"]`,
   einzeln abschaltbar (Privatsphäre). User-Agent
   `TorroCast/<version> (+<repo-url>)`; für Podcast Index Pflicht.
9. **Offline-Provider später:** den Podcast-Index-SQLite-Dump als optionalen
   `OfflineIndexProvider`. Derselbe Trait, keine Sonderbehandlung.
10. **Tests:** je Provider aufgezeichnete Fixtures, dazu ein `MockProvider` für die
    Tests der Zusammenführung und De-Duplikation.

## Offene Punkte

1. Podcast Index: schriftlich klären, ob ein zur Build-Zeit gesetzter Key für ein OSS-Projekt geduldet wird (ToS §4.2.1 gegen die Praxis von AntennaPod und Kasts).
2. fyyd: Nutzungsbedingungen und Rate Limits nicht auffindbar. Betreiber kontaktieren.
3. Apple: keine ausdrückliche Lizenz für Podcast-Clients. Alter Charts-Endpunkt und Genres-Endpunkt sind undokumentiert. Ob `feedUrl` bei Abo-Shows fehlt, ist ungetestet.
4. Taddy und die aktuelle Podchaser-Preisstruktur nur teilweise verifiziert.
