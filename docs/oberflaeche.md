# Oberfläche

Entwurf für die TUI. Ausgangspunkt sind die TorroMail-TUI (`crates/torromail-tui`) und
die Designsprache der Torro-Apps (`torro-design/app-design.md`). TorroCast soll als
Mitglied derselben Familie erkennbar sein: gleicher Rahmen, gleiche Farben, gleiche
Bedienung – nur der Inhalt wechselt.

Die Mockups sind 104 Spalten breit. Farben lassen sich in Markdown nicht zeigen; wo
sie eine Rolle spielen, steht es im Text.

## Was von TorroMail übernommen wird

| Baustein | TorroMail | TorroCast |
|---|---|---|
| Markenleiste oben | rote Zeile, Wortmarke `\_ TORROMAIL _/` („TORRO“ weiß, Produktname silber), Version rechts | identisch, `\_ TORROCAST _/` |
| Menü links | 26 Spalten, nummerierte Einträge, aktiver Eintrag rot hinterlegt, Claim am Fuß | identisch |
| Inhalt rechts | abgerundete Panels; das Panel mit dem Fokus trägt den Akzentrand | identisch |
| Reiter im Inhalt | `tab` / `shift+tab` | identisch |
| Tastenzeile unten | Tasten als kleine Kacheln, daneben das Wort | identisch, wechselt je Ansicht |
| Farbpalette | `theme.rs`: Torro-Rot, Silber, gedämpfte Grautöne, Grün/Amber/Cyan für Status | unverändert übernehmen |
| Hintergrund | bleibt der des Terminals | identisch |
| Sprache | Deutsch und Englisch nach `LANG`; englischer Text ist der Schlüssel | identisch |
| Tests | Render-Tests gegen `TestBackend` | identisch, plus Snapshots mit `insta` |

Neu gegenüber TorroMail sind drei Dinge: ein **Eingabefeld** (Suche), eine
**Hierarchie zum Hineingehen** (Suche → Podcast → Folge) und ab v0.2 die
**Wiedergabeleiste**.

## Haltung

Aus `app-design.md`, übertragen aufs Terminal:

1. **Rot ist Marke, nie Status.** Rot erscheint in der Markenleiste und am aktiven
   Menüeintrag. Der Fokusrand nutzt den helleren Akzent. Zustände tragen Grün (in
   Ordnung), Amber (wartet auf dich), Grau (nicht eingerichtet) – und **immer Symbol
   plus Wort**, nie die Farbe allein: `● Aktiv`, `○ Ausgeschaltet`, `▲ Nicht erreichbar`.
2. **Ein lauter Markenmoment.** Das ist die rote Leiste. Alles darunter bleibt ruhig.
3. **Sätze aus Sicht des Nutzers.** „Apple antwortet gerade nicht. Du siehst die
   Treffer von fyyd.“ statt „HTTP 503“. Technische Details gehören ins Log.
4. **Fehler stehen dort, wo sie entstehen** – als Zeile im betroffenen Panel, nicht als
   Dialog, der die Bedienung blockiert.
5. **Leere Flächen sagen, was hier erscheinen wird.** „Noch keine Suche. Tippe `/` und
   einen Begriff, oder wechsle zu den Charts.“
6. **Listen sind Kartenlisten.** Ein Eintrag hat zwei Zeilen: Titel, darunter gedämpft
   die Nebeninformation. Tabellen gibt es nur dort, wo Spalten wirklich helfen
   (Folgenliste: Datum und Dauer).

## Rahmen und Bedienmodell

### Zwei Modi

TorroMail kennt nur Befehlstasten. Mit einem Suchfeld braucht es eine klare Trennung:

- **Navigieren** (Normalzustand): Jede Taste ist ein Befehl. `1`–`7` wechselt das
  Menü, `j`/`k` bewegt die Auswahl.
- **Eingeben**: Das Suchfeld hat den Fokus und den Akzentrand, alle Zeichen landen im
  Feld. `esc` oder `↓` verlässt das Feld, `enter` startet die Suche sofort.

`/` springt von überall ins Suchfeld. In Listen ohne Suchfeld (Folgen) filtert `/` die
Liste.

### Hineingehen und zurück

`enter` oder `→` öffnet den gewählten Eintrag, `esc`, `←` oder `backspace` geht eine
Ebene zurück. Der Pfad steht im Panel-Titel: `Entdecken › Lage der Nation › LdN 412`.
Beim Zurückgehen steht die Auswahl dort, wo sie war.

### Tasten

| Taste | Wirkung |
|---|---|
| `↑` `↓` / `j` `k` | Auswahl bewegen |
| `bild↑` `bild↓`, `pos1` `ende` / `g` `G` | blättern, Anfang, Ende |
| `enter` / `→` / `l` | öffnen |
| `esc` / `←` / `h` / `backspace` | zurück |
| `tab` / `shift+tab` | Reiter oder Panel wechseln |
| `/` | suchen oder filtern |
| `1`–`7` | Menü |
| `o` | Reihenfolge umkehren |
| `w` | im Browser öffnen |
| `1`–`9` in Shownotes | nummerierten Link öffnen |
| `m` | Beschreibung aufklappen |
| `r` | neu laden |
| `?` | alle Tasten |
| `q` / `ctrl+c` | beenden |

Alle Befehle kommen ohne ungewöhnliche Modifier aus; das hält die Belegung unter
Windows verlässlich. Die Maus funktioniert zusätzlich: Klick wählt, Doppelklick öffnet,
das Rad blättert, Menü und Reiter sind anklickbar.

Ab v0.2, solange etwas läuft: `leertaste` Pause, `b` / `f` 30 s zurück / vor, `<` / `>`
Kapitel, `-` / `+` Tempo, `a` in die Warteschlange, `s` abonnieren.

### Breite

| Terminalbreite | Verhalten |
|---|---|
| ab 100 Spalten | Menü + zwei Panels nebeneinander (Liste und Vorschau) |
| 80–99 | Menü + ein Panel; die Vorschau entfällt, `enter` öffnet direkt |
| unter 80 | Menü schrumpft auf die Ziffernspalte |
| unter 60 × 16 | Hinweis „Das Fenster ist zu klein“ statt zerbrochener Darstellung |

## Ansichten v0.1

### Entdecken › Suche

```
  \_ TORROCAST _/                                                                               v0.1.0
╭ Menü ──────────────────╮╭ Entdecken ─────────────────────────────────────────────────────────────────╮
│                        ││  Suche    Charts    Kategorien                                             │
│▌1  Entdecken           ││  ━━━━━                                                                     │
│ 2  Einstellungen       ││                                                                            │
│ 3  Hilfe               ││  ⌕ lage der nation▏                                                        │
│                        │╰────────────────────────────────────────────────────────────────────────────╯
│                        │╭ 12 Treffer ────────────────────────────╮╭ Vorschau ────────────────────────╮
│                        ││▌Lage der Nation                        ││ Lage der Nation                  │
│                        ││▌Philip Banse & Ulf Buermeyer · Politik ││ Der Politik-Podcast aus Berlin   │
│                        ││                                        ││ Philip Banse & Ulf Buermeyer     │
│                        ││ Die Lage – International               ││                                  │
│                        ││ DER SPIEGEL · Nachrichten              ││ Politik · Nachrichten            │
│                        ││                                        ││ 497 Folgen · zuletzt heute       │
│                        ││ Zur Lage der Nation (Archiv)           ││                                  │
│                        ││ Deutschlandfunk · Politik              ││ Quelle        Apple              │
│                        ││                                        ││ Website       lagedernation.org  │
│                        ││ Lagebesprechung                        ││                                  │
│                        ││ Table.Media · Wirtschaft               ││ Beschreibung und Folgen          │
│                        ││                                        ││ erscheinen, sobald du den        │
│                        ││ Nation of Plebs                        ││ Podcast öffnest.                 │
│                        ││ Studio Bummens · Comedy                ││                                  │
│ Podcasts im Terminal.  ││                                        ││                                  │
│                        ││                                        ││                                  │
╰────────────────────────╯╰────────────────────────────────────────╯╰──────────────────────────────────╯
 [↑↓] Auswahl   [enter] Öffnen   [tab] Reiter   [esc] Eingabe verlassen   [1-3] Menü
```

- Drei Reiter: **Suche**, **Charts** (Apple-Top-Liste des eingestellten Landes),
  **Kategorien** (Apple-Genres; `enter` zeigt die Charts der Kategorie).
- Die Suche startet 600 ms nach dem letzten Tastendruck und ab drei Zeichen, mit
  `enter` sofort. Das hält das Apple-Limit von rund 20 Anfragen pro Minute ein.
- Sind mehrere Quellen aktiv, erscheinen Treffer, sobald die erste Quelle antwortet.
  Der Panel-Titel zeigt den Stand: `12 Treffer · fyyd lädt noch`.
- Die Vorschau zeigt nur, was die Quelle schon geliefert hat. Kein Feed-Abruf, solange
  man nur durch die Liste blättert.

### Podcast

```
  \_ TORROCAST _/                                                                               v0.1.0
╭ Menü ──────────────────╮╭ Entdecken › Lage der Nation ───────────────────────────────────────────────╮
│                        ││ ▄▄▄▄▄▄▄▄▄▄▄▄   Lage der Nation – der Politik-Podcast aus Berlin            │
│▌1  Entdecken           ││ █          █   Philip Banse & Ulf Buermeyer                                │
│ 2  Einstellungen       ││ █  Cover   █                                                               │
│ 3  Hilfe               ││ █          █   Politik · Nachrichten · Deutsch · 497 Folgen                │
│                        ││ █          █   lagedernation.org                                           │
│                        ││ ▀▀▀▀▀▀▀▀▀▀▀▀   ♥ Unterstützen: plus.lagedernation.org                      │
│                        ││                                                                            │
│                        ││ Jede Woche besprechen der Journalist Philip Banse und der Jurist Ulf       │
│                        ││ Buermeyer die politischen Ereignisse der Woche …                 [m] mehr  │
│                        │╰────────────────────────────────────────────────────────────────────────────╯
│                        │╭ Folgen · neueste zuerst ───────────────────────────────────────────────────╮
│                        ││▌LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen     19.09.  1:34:10  ▤ ¶ │
│                        ││ LdN 411 · Sommerinterview, Bahn, Chatkontrolle        12.09.  1:28:45  ▤ ¶ │
│                        ││ LdN 410 · Spezial: Wie funktioniert der Bundesrat?    05.09.    58:02  ▤   │
│                        ││ LdN 409 · Stromsteuer, Richterwahl, Ukraine           29.08.  1:41:33  ▤ ¶ │
│                        ││ LdN 408 · Sommerpause – Hörerfragen                   22.08.  1:12:09  ▤   │
│                        ││ LdN 407 · Koalitionsausschuss, Hitzeschutz            15.08.  1:30:51  ▤ ¶ │
│ Podcasts im Terminal.  ││                                                                            │
│                        ││ ▤ Kapitel   ¶ Transkript                                                   │
╰────────────────────────╯╰────────────────────────────────────────────────────────────────────────────╯
 [↑↓] Folge   [enter] Öffnen   [/] Filtern   [o] Reihenfolge   [w] Website   [esc] Zurück
```

- Kopf mit Cover, Titel, Autor, Kategorien, Sprache, Website und
  Unterstützungs-Link. Die Beschreibung ist auf zwei Zeilen gekürzt, `m` klappt sie auf.
- **Cover:** echtes Bild, wenn das Terminal es kann (Kitty, Sixel, iTerm2), sonst
  Block-Zeichen. In den Einstellungen abschaltbar; dann rückt der Text nach links.
- Folgenliste mit Datum, Dauer und Kennzeichen für Kapitel (`▤`) und Transkript (`¶`).
  Die Legende steht unter der Liste, die Zeichen erklären sich nicht von selbst.
- Während der Feed lädt: Kopf aus den Suchdaten, in der Liste „Lade Folgen …“.

### Folge

```
  \_ TORROCAST _/                                                                               v0.1.0
╭ Menü ──────────────────╮╭ Entdecken › Lage der Nation › LdN 412 ─────────────────────────────────────╮
│                        ││ LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen                          │
│▌1  Entdecken           ││ 19. September 2026 · 1:34:10 · Staffel –, Folge 412                        │
│ 2  Einstellungen       │╰────────────────────────────────────────────────────────────────────────────╯
│ 3  Hilfe               │╭ Shownotes ─────────────────────────────╮╭ Kapitel · 7 ─────────────────────╮
│                        ││ Begrüßung und Hausmitteilungen. Wir    ││▌00:00:00  Begrüßung              │
│                        ││ sind im Oktober live in Leipzig –      ││ 00:03:12  Hausmitteilungen       │
│                        ││ Karten gibt es hier [1].               ││ 00:06:40  Haushalt 2027          │
│                        ││                                        ││ 00:31:05  Rentenpaket            │
│                        ││ Haushalt 2027                          ││ 00:58:47  Wahl in Norwegen    ↗  │
│                        ││ • Kabinettsbeschluss im Überblick [2]  ││ 01:17:20  Chatkontrolle          │
│                        ││ • Kritik des Bundesrechnungshofs [3]   ││ 01:29:02  Verabschiedung         │
│                        ││                                        ││                                  │
│                        ││ Rentenpaket                            ││ Aus dem Feed (Podlove)           │
│                        ││ Was im Gesetzentwurf steht und was     ││                                  │
│                        ││ die Kommission noch klären soll [4].   ││                                  │
│                        ││                                        ││                                  │
│                        ││ Links                                  ││                                  │
│                        ││ [1] lagedernation.org/live             ││                                  │
│ Podcasts im Terminal.  ││ [2] bundesfinanzministerium.de/…       ││                                  │
│                        ││                                        ││                                  │
╰────────────────────────╯╰────────────────────────────────────────╯╰──────────────────────────────────╯
 [tab] Shownotes/Kapitel   [↑↓] Blättern   [1-9] Link öffnen   [w] Im Browser   [esc] Zurück
```

- Links die Shownotes als lesbarer Text: Absätze, Zwischenüberschriften, Listen. Links
  werden nummeriert und unten aufgelistet; die Ziffer öffnet den Link.
- Rechts die Kapitel. `↗` heißt: Das Kapitel hat einen Link. Unter der Liste steht die
  Herkunft („Aus dem Feed“, „Aus der Audiodatei“). Eingebettete Kapitel werden erst
  beim Öffnen der Folge geholt; solange steht dort „Suche Kapitel in der Audiodatei …“.
- Ohne Kapitel nehmen die Shownotes die volle Breite ein.
- `tab` wechselt den Fokus zwischen Shownotes und Kapiteln.

### Einstellungen › Quellen

```
  \_ TORROCAST _/                                                                               v0.1.0
╭ Menü ──────────────────╮╭ Einstellungen ─────────────────────────────────────────────────────────────╮
│                        ││  Quellen    Bibliothek    Darstellung    Tasten                            │
│ 1  Entdecken           ││  ━━━━━━━                                                                   │
│▌2  Einstellungen       │╰────────────────────────────────────────────────────────────────────────────╯
│ 3  Hilfe               │╭ Wo TorroCast nach Podcasts sucht ──────────────────────────────────────────╮
│                        ││                                                                            │
│                        ││▌●  Apple Podcasts                                              Aktiv       │
│                        ││▌   Suche, Charts und Kategorien. Braucht keine Einrichtung.                │
│                        ││                                                                            │
│                        ││ ○  Podcast Index                               Kein Schlüssel hinterlegt   │
│                        ││    Offenes Verzeichnis mit Trending und Kapitel-Hinweisen.                 │
│                        ││    Du brauchst einen eigenen, kostenlosen Schlüssel.                       │
│                        ││                                                                            │
│                        ││ ○  fyyd                                                 Ausgeschaltet      │
│                        ││    Deutschsprachiges Verzeichnis. Braucht keine Einrichtung.               │
│                        ││                                                                            │
│                        ││ Land für Suche und Charts      Deutschland                                 │
│ Podcasts im Terminal.  ││                                                                            │
│                        ││                                                                            │
╰────────────────────────╯╰────────────────────────────────────────────────────────────────────────────╯
 [↑↓] Quelle   [leertaste] An/Aus   [enter] Einrichten   [tab] Reiter   [1-3] Menü
```

- Apple ist immer aktiv und lässt sich nicht abschalten, solange keine andere Quelle
  aktiv ist.
- `enter` auf Podcast Index öffnet ein kleines Formular für Schlüssel und Secret, mit
  dem Hinweis, wo man beides kostenlos bekommt. Beim Speichern wird die Verbindung
  geprüft; der Zustand wechselt auf `● Aktiv` oder `▲ Schlüssel wird nicht angenommen`.
  Der Schlüssel liegt im Schlüsselbund des Betriebssystems und wird in der Oberfläche
  maskiert angezeigt.
- Weitere Reiter: **Bibliothek** (ab v0.2: Ordner wählen, bekannte Geräte),
  **Darstellung** (Cover an/aus, Sprache), **Tasten**.

## Rahmen ab v0.2

```
  \_ TORROCAST _/                                                                               v0.1.0
╭ Menü ──────────────────╮╭ Abos · 23 ─────────────────────────────────────────────────────────────────╮
│                        ││▌Lage der Nation                                  2 neu   zuletzt heute     │
│ 1  Entdecken           ││ Logbuch:Netzpolitik                              1 neu   zuletzt gestern   │
│▌2  Abos                ││ Methodisch inkorrekt                                     zuletzt 14.09.    │
│ 3  Neue Folgen         ││ Chaosradio                                               zuletzt 28.08.    │
│ 4  Warteschlange       ││ Freak Show                                       3 neu   zuletzt 11.09.    │
│ 5  Downloads           ││                                                                            │
│ 6  Einstellungen       ││                                                                            │
│ 7  Hilfe               ││                                                                            │
│                        ││                                                                            │
│                        ││                                                                            │
│ Podcasts im Terminal.  ││                                                                            │
│                        ││                                                                            │
╰────────────────────────╯╰────────────────────────────────────────────────────────────────────────────╯
────────────────────────────────────────────────────────────────────────────────────────────────────────
 ▶  LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen — Lage der Nation           1,3×   ⏾ 30 min
    00:41:20 ━━━━━━━━━━━━━━━━━━━┿━━━━━━━━━━━━●────────────────────────────── 1:34:10   Rentenpaket
 [leertaste] Pause   [b f] ±30 s   [< >] Kapitel   [- +] Tempo   [a] In Warteschlange   [1-7] Menü
```

- Das Menü wächst auf sieben Einträge, wie bei TorroMail. **Neue Folgen** und
  **Warteschlange** tragen einen Zähler am Eintrag, sobald etwas darin liegt.
- Die **Wiedergabeleiste** sitzt über der Tastenzeile und ist nur sichtbar, wenn etwas
  geladen ist. Zeile eins: Zustand, Folge, Podcast, rechts Tempo und Schlaf-Timer.
  Zeile zwei: Position, Fortschrittsbalken mit Kapitelgrenzen (`┿`), Dauer, aktuelles
  Kapitel.
- Der Fortschrittsbalken ist die eine Stelle im Inhalt, an der der Akzent als Fläche
  auftritt.

## Offene Fragen

| # | Frage | Vorschlag |
|---|---|---|
| 1 | Gemeinsame TUI-Bausteine (Palette, Panel, Tastenzeile, Menü, Sprachumschaltung) für TorroMail und TorroCast: kopieren oder eigenes Crate? | Für v0.1 kopieren. Sobald beide TUIs stabil sind, ein gemeinsames Crate `torro-tui` herausziehen – erst dann ist klar, was wirklich gemeinsam ist. |
| 2 | Menü in v0.1: nur die drei vorhandenen Einträge oder schon alle sieben, die kommenden ausgegraut? | Nur die vorhandenen. Ausgegraute Einträge versprechen etwas, das noch nicht da ist. |
| 3 | Befehlszeile mit `:` wie in podliner (`:open <url>`)? | Nicht in v0.1. „Feed-URL öffnen“ geht über das Suchfeld: Eine eingegebene URL wird als Feed geöffnet. |
| 4 | Cover in der Trefferliste? | Nein, nur im Podcast-Kopf. Viele kleine Bilder machen das Blättern träge. |
| 5 | Helles Terminal: Die TorroMail-Palette ist auf dunklen Grund abgestimmt. | In v0.1 wie TorroMail; ein helles Schema später für beide gemeinsam. |
