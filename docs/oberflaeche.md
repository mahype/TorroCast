# Oberfläche

> **Stand 19.09.2026:** Der Entwurf ist umgesetzt. Abweichungen der gebauten TUI von den Mockups:
>
> - Das Menü hat acht Einträge: Entdecken, Abos, Neue Folgen, Als Nächstes, Playlists, Downloads,
>   Einstellungen, Hilfe (`1`–`8`).
> - Die Einstellungen sind eine Seite ohne Reiter: Quellen, Land, Bibliotheks-Ordner, Cover.
> - Der Schlaf-Timer liegt auf `t`, Downloads auf `D`, „in Playlist legen“ auf `L`, Abonnieren auf `s`,
>   die Folgensuche auf `e` (im Suchfeld `ctrl+e`).
> - Der Fortschrittsbalken im kleinen Player zeigt keine Kapitelgrenzen (zu eng), der im großen schon.
> - Alle Tasten stehen in der App unter `?`.

Entwurf für die TUI. Ausgangspunkt sind die TorroMail-TUI (`crates/torromail-tui`) und
die Designsprache der Torro-Apps (`torro-design/app-design.md`). TorroCast soll als
Mitglied derselben Familie erkennbar sein: gleicher Rahmen, gleiche Farben, gleiche
Bedienung – nur der Inhalt wechselt.

Die Mockups zeigen die Farben aus TorroMails `theme.rs` auf einem dunklen Terminal, 104
Spalten breit. Sie entstehen aus [`tools/mockups.py`](tools/mockups.py); nach einer
Änderung dort `python3 docs/tools/mockups.py` ausführen.

## Farben

| Farbe | Wert | Wofür |
|---|---|---|
| Torro-Rot | `#D50C0C` | Markenleiste, aktiver Menüeintrag – sonst nirgends |
| Akzent | `#EE3A33` | Rand des Panels mit Fokus, aktiver Reiter, Eingabemarke, Fortschrittsbalken, Zähler am Menü |
| Silber | `#C4C3C3` | Produktname in der Wortmarke, Zwischenüberschriften in Shownotes |
| Gedämpft | `#978A8D` | zweite Zeile eines Eintrags, Datum, Dauer, Beschriftungen |
| Blass | `#6A5E61` | Menüziffern, Claim, Hinweise in leeren Flächen |
| Linie | `#5A4C50` | Panel-Ränder ohne Fokus |
| Auswahl | `#34272B` | Hintergrund der gewählten Zeile, dazu fett |
| Taste | `#3D3135` | Kacheln in der Tastenzeile |
| Cyan | `#79BFD3` | alles, was sich öffnen lässt: Links, Linknummern, Kapitel- und Transkript-Zeichen |
| Grün | `#86CF7D` | in Ordnung: Quelle aktiv, neue Folgen |
| Amber | `#ECB755` | wartet auf dich: Schlüssel fehlt |

Neu gegenüber TorroMail ist nur die feste Rolle von Cyan für Links. Der Hintergrund
bleibt der des Terminals.

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
**Hierarchie zum Hineingehen** (Suche → Podcast → Folge) und ab v0.2 der
**Player in der Menüspalte**.

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
| `1`–`9` | Menü; `0` öffnet den Player |
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

Ab v0.2 kommen die Tasten für Wiedergabe und Playlists dazu, siehe
[Wiedergabe und Playlists](#wiedergabe-und-playlists-ab-v02). `s` abonniert.

### Breite

| Terminalgröße | Verhalten |
|---|---|
| ab 100 Spalten | Menü + zwei Panels nebeneinander (Liste und Vorschau) |
| 80–99 Spalten | Menü + ein Panel; die Vorschau entfällt, `enter` öffnet direkt |
| unter 30 Zeilen | der Player in der Menüspalte verzichtet auf die Pegelanzeige |
| unter 80 × 24 | nur der Hinweis „Das Fenster ist zu klein“ mit aktueller und benötigter Größe, siehe unten |

## Ansichten v0.1

### Entdecken › Suche

![Entdecken › Suche](mockups/suche.svg)

- Drei Reiter: **Suche**, **Charts** (Apple-Top-Liste des eingestellten Landes),
  **Kategorien** (Apple-Genres; `enter` zeigt die Charts der Kategorie).
- Die Suche startet 600 ms nach dem letzten Tastendruck und ab drei Zeichen, mit
  `enter` sofort. Das hält das Apple-Limit von rund 20 Anfragen pro Minute ein.
- Sind mehrere Quellen aktiv, erscheinen Treffer, sobald die erste Quelle antwortet.
  Der Panel-Titel zeigt den Stand: `12 Treffer · fyyd lädt noch`.
- Die Vorschau zeigt nur, was die Quelle schon geliefert hat. Kein Feed-Abruf, solange
  man nur durch die Liste blättert.

### Podcast

![Podcast-Ansicht](mockups/podcast.svg)

- Kopf mit Cover, Titel, Autor, Kategorien, Sprache, Website und
  Unterstützungs-Link. Die Beschreibung ist auf zwei Zeilen gekürzt, `m` klappt sie auf.
- **Cover:** echtes Bild, wenn das Terminal es kann (Kitty, Sixel, iTerm2), sonst
  Block-Zeichen. In den Einstellungen abschaltbar; dann rückt der Text nach links.
- Folgenliste mit Datum, Dauer und Kennzeichen für Kapitel (`▤`) und Transkript (`¶`).
  Die Legende steht unter der Liste, die Zeichen erklären sich nicht von selbst.
- Während der Feed lädt: Kopf aus den Suchdaten, in der Liste „Lade Folgen …“.

### Folge

![Folge mit Shownotes und Kapiteln](mockups/folge.svg)

- Links die Shownotes als lesbarer Text: Absätze, Zwischenüberschriften, Listen. Links
  werden nummeriert und unten aufgelistet; die Ziffer öffnet den Link.
- Rechts die Kapitel. `↗` heißt: Das Kapitel hat einen Link. Unter der Liste steht die
  Herkunft („Aus dem Feed“, „Aus der Audiodatei“). Eingebettete Kapitel werden erst
  beim Öffnen der Folge geholt; solange steht dort „Suche Kapitel in der Audiodatei …“.
- Ohne Kapitel nehmen die Shownotes die volle Breite ein.
- `tab` wechselt den Fokus zwischen Shownotes und Kapiteln.

### Einstellungen › Quellen

![Einstellungen › Quellen](mockups/quellen.svg)

- Apple ist immer aktiv und lässt sich nicht abschalten, solange keine andere Quelle
  aktiv ist.
- `enter` auf Podcast Index öffnet ein kleines Formular für Schlüssel und Secret, mit
  dem Hinweis, wo man beides kostenlos bekommt. Beim Speichern wird die Verbindung
  geprüft; der Zustand wechselt auf `● Aktiv` oder `▲ Schlüssel wird nicht angenommen`.
  Der Schlüssel liegt im Schlüsselbund des Betriebssystems und wird in der Oberfläche
  maskiert angezeigt.
- Weitere Reiter: **Bibliothek** (ab v0.2: Ordner wählen, bekannte Geräte),
  **Darstellung** (Cover an/aus, Sprache), **Tasten**.

## Wiedergabe und Playlists (ab v0.2)

Vorlage ist Pocket Casts: ein kleiner Player, der immer da ist, ein großer Player zum
Aufklappen, und die Liste „Als Nächstes“ (dort „Up Next“), in die man von überall
Folgen legt. Alles ist mit der Tastatur bedienbar; die Maus ist nur eine Abkürzung.

### Der Player in der Menüspalte

![Rahmen ab v0.2 mit Player in der Menüspalte](mockups/rahmen.svg)

Der Player sitzt unter dem Menü, in derselben Spalte, und ist in jeder Ansicht zu
sehen, solange eine Folge geladen ist. Von oben nach unten:

- **Pegelanzeige:** zeigt live, was gerade zu hören ist. Im Terminal sind das
  Block-Zeichen in acht Höhenstufen (`▁▂▃▄▅▆▇█`). Bei Pause steht sie still, gedämpft.
  Eine echte Wellenform der ganzen Folge gibt es nicht: Dafür müsste die komplette
  Datei vorab geladen und durchgerechnet werden.
- Folge und Podcast, gekürzt auf die Spaltenbreite.
- Fortschrittsbalken mit Kapitelgrenzen, darunter Position, Dauer und das Kapitel.
- **Fünf Knöpfe, unter jedem seine Taste.** Die Knöpfe sind anklickbar, die Tasten
  wirken in jeder Ansicht:

| Knopf | Taste | Wirkung |
|---|---|---|
| `◀◀` | `,` | ein Kapitel zurück; ohne Kapitel zum Anfang der Folge |
| `▮▮` / `▶` | `leertaste` | Pause / weiter |
| `■` | `x` | Stopp: Position merken, Player leeren, der Block verschwindet |
| `▶▶` | `.` | ein Kapitel vor; ohne Kapitel 5 Minuten |
| `▶▮` | `n` | nächste Folge aus „Als Nächstes“ |

- Dazu ohne Knopf: `b` / `f` springt 30 s zurück / vor (Werte einstellbar, wie bei
  Pocket Casts), `-` / `+` ändert das Tempo, `t` stellt den Schlaf-Timer, `0` öffnet
  den großen Player.
- Alle Tasten liegen auf deutscher und englischer Tastatur direkt, ohne `AltGr`.
- Ist das Fenster niedriger als 30 Zeilen, entfällt die Pegelanzeige zuerst.

### Beim Suchen in die Playlist legen

![Suche nach Folgen, eine Folge wurde an den Anfang von „Als Nächstes“ gelegt](mockups/hinzufuegen.svg)

- Die Suche hat einen Umschalter **Podcasts / Folgen**. Die Folgensuche läuft über
  Apple und liefert Audio-Adresse, GUID und Feed gleich mit – eine gefundene Folge
  lässt sich also direkt einreihen, ohne den Podcast zu öffnen oder zu abonnieren.
- In **jeder** Folgenliste (Suche, Podcast, Abos, Neue Folgen) gilt:

| Taste | Wirkung | Pocket Casts |
|---|---|---|
| `a` | ans Ende von „Als Nächstes“ | Play Last |
| `A` | an den Anfang von „Als Nächstes“ | Play Next |
| `p` | jetzt spielen; die laufende Folge rückt auf Platz 1 | Play Now |
| `L` | in eine Playlist legen … (öffnet eine kleine Auswahl; ab mehreren Playlists) | – |

- Die Auswahl bleibt nach dem Einreihen stehen, damit man mehrere Folgen nacheinander
  einsammeln kann. Rechts in der Zeile steht, wo die Folge liegt; unten bestätigt ein
  Satz die Aktion. Dieselbe Taste auf einer schon eingereihten Folge nimmt sie wieder
  heraus.

### Als Nächstes

![Die Liste „Als Nächstes“](mockups/naechstes.svg)

- Oben die laufende Folge, darunter die Reihenfolge. Der Titel nennt Anzahl und
  Gesamtdauer.
- `J` / `K` verschiebt die gewählte Folge, `d` entfernt sie, `p` spielt sie sofort,
  `C` leert die Liste (fragt in der Zeile nach, kein Dialog).
- Ist eine Folge zu Ende, läuft Platz 1 ohne Pause weiter. Ist die Liste leer, stoppt
  der Player.
- Der Zähler am Menüeintrag zeigt, wie viele Folgen warten.

### Mehrere Playlists (später)

„Als Nächstes“ ist die eine Liste, die der Player abspielt. Eigene Playlists kommen
als Menüeintrag **Playlists** dazu: benannte Listen, die man mit `L` füllt und mit
einer Taste komplett oder teilweise in „Als Nächstes“ legt. Später auch Listen nach
Regeln („alles Ungehörte unter 30 Minuten“), wie die Smart Playlists in Pocket Casts.

Für das Datenformat heißt das: „Als Nächstes“ wird **von Anfang an als Playlist mit
fester Kennung** gespeichert, nicht als Sonderfall. Dann brauchen mehrere Playlists
später kein neues Format im Bibliotheks-Ordner.

### Der große Player

![Der große Player mit Kapitelliste](mockups/laeuft.svg)

- `0` oder ein Klick auf den kleinen Player öffnet ihn; `esc` geht zurück. Solange er
  offen ist, entfällt der kleine Player in der Menüspalte.
- Cover, Folge, aktuelles Kapitel, Einstellungen der Wiedergabe (Tempo, Stille kürzen,
  Schlaf-Timer, Sprungweiten), breiter Fortschrittsbalken mit Kapitelgrenzen.
- Darunter die Kapitel: Gehörtes blass, das laufende markiert. `enter` springt zum
  gewählten Kapitel. `tab` wechselt zu den Shownotes; Zeitmarken darin sind anspringbar.

## Wenn das Fenster zu klein ist

![Hinweis bei zu kleinem Fenster](mockups/zuklein.svg)

Wie in btop: Unterschreitet das Terminal die Mindestgröße, zeigt TorroCast **nur**
diesen Hinweis – die aktuelle Größe, die benötigte Größe, und pro Wert rot oder grün,
ob er reicht. Die Zahlen folgen dem Ziehen am Fenster live. Sobald beide Werte grün
sind, erscheint die Oberfläche wieder, genau dort, wo man war. Eine laufende Wiedergabe
geht währenddessen weiter, und die Wiedergabetasten wirken auch im Hinweis.

Mindestgröße: **80 × 24**.

## Offene Fragen

| # | Frage | Vorschlag |
|---|---|---|
| 1 | Gemeinsame TUI-Bausteine (Palette, Panel, Tastenzeile, Menü, Sprachumschaltung) für TorroMail und TorroCast: kopieren oder eigenes Crate? | Für v0.1 kopieren. Sobald beide TUIs stabil sind, ein gemeinsames Crate `torro-tui` herausziehen – erst dann ist klar, was wirklich gemeinsam ist. |
| 2 | Menü in v0.1: nur die drei vorhandenen Einträge oder schon alle sieben, die kommenden ausgegraut? | Nur die vorhandenen. Ausgegraute Einträge versprechen etwas, das noch nicht da ist. |
| 3 | Befehlszeile mit `:` wie in podliner (`:open <url>`)? | Nicht in v0.1. „Feed-URL öffnen“ geht über das Suchfeld: Eine eingegebene URL wird als Feed geöffnet. |
| 4 | Cover in der Trefferliste? | Nein, nur im Podcast-Kopf. Viele kleine Bilder machen das Blättern träge. |
| 5 | Helles Terminal: Die TorroMail-Palette ist auf dunklen Grund abgestimmt. | In v0.1 wie TorroMail; ein helles Schema später für beide gemeinsam. |
