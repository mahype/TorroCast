#!/usr/bin/env python3
"""Draws the TUI mockups for docs/oberflaeche.md as SVG.

The palette is TorroMail's `crates/torromail-tui/src/theme.rs`, so the pictures
show the colours the real TUI will use. Run from the repository root:

    python3 docs/tools/mockups.py
"""
import pathlib
from html import escape

OUT = pathlib.Path(__file__).resolve().parent.parent / "mockups"

# theme.rs
RED = "#d50c0c"
ACCENT = "#ee3a33"
SILVER = "#c4c3c3"
MUTED = "#978a8d"
FAINT = "#6a5e61"
LINE = "#5a4c50"
GREEN = "#86cf7d"
AMBER = "#ecb755"
CYAN = "#79bfd3"
SELECTION = "#34272b"
KEY = "#3d3135"
VERSION = "#ffd2ce"
WHITE = "#ffffff"
# The TUI keeps the terminal's own background; the pictures need one.
TERMINAL = "#171314"
TEXT = "#e4dfe0"

CW, CH, FONT = 8.4, 19, 14
W, MENU = 104, 26
FAMILY = "ui-monospace, 'SF Mono', 'JetBrains Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace"


class Canvas:
    def __init__(self, rows):
        self.rows = rows
        self.back, self.front = [], []

    def rect(self, x, y, w, h, fill, rx=0):
        self.back.append(
            f'<rect x="{x * CW:.1f}" y="{y * CH:.1f}" width="{w * CW:.1f}" height="{h * CH:.1f}" '
            f'fill="{fill}" rx="{rx}"/>'
        )

    def text(self, x, y, string, fg=TEXT, bold=False, bg=None):
        """One run of text; `textLength` pins it to the cell grid whatever font the viewer has."""
        if not string:
            return x
        n = len(string)
        if bg:
            self.rect(x, y, n, 1, bg)
        if string.strip():
            weight = ' font-weight="700"' if bold else ""
            self.front.append(
                f'<text x="{x * CW:.1f}" y="{y * CH + CH * 0.72:.1f}" textLength="{n * CW:.1f}" '
                f'lengthAdjust="spacingAndGlyphs" fill="{fg}"{weight} xml:space="preserve">{escape(string)}</text>'
            )
        return x + n

    def spans(self, x, y, spans):
        for span in spans:
            if isinstance(span, str):
                span = (span,)
            string, *style = span
            options = dict(zip(("fg", "bold", "bg"), style))
            x = self.text(x, y, string, **options)
        return x

    def panel(self, x, y, w, h, title, focused=False):
        colour = ACCENT if focused else LINE
        self.back.append(
            f'<rect x="{(x + 0.5) * CW:.1f}" y="{(y + 0.5) * CH:.1f}" width="{(w - 1) * CW:.1f}" '
            f'height="{(h - 1) * CH:.1f}" rx="7" fill="none" stroke="{colour}" stroke-width="1.3"/>'
        )
        self.back.append(
            f'<rect x="{(x + 1.5) * CW:.1f}" y="{y * CH:.1f}" width="{(len(title) + 2) * CW:.1f}" '
            f'height="{CH}" fill="{TERMINAL}"/>'
        )
        self.text(x + 2, y, " " + title + " ", bold=True)

    def svg(self):
        width, height = W * CW, self.rows * CH
        return "\n".join(
            [
                f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {height:.0f}" '
                f'width="{width:.0f}" height="{height:.0f}" font-family="{FAMILY}" font-size="{FONT}">',
                f'<rect width="100%" height="100%" fill="{TERMINAL}" rx="8"/>',
                *self.back,
                *self.front,
                "</svg>",
            ]
        )


# ── the frame every screen shares ────────────────────────────────────────────

V01 = ["Entdecken", "Einstellungen", "Hilfe"]
V02 = ["Entdecken", "Abos", "Neue Folgen", "Warteschlange", "Downloads", "Einstellungen", "Hilfe"]


def frame(rows, active, entries, hints, badges=None, player=False):
    """Brand bar, menu and key hints. Returns the canvas and the content area."""
    c = Canvas(rows)
    c.rect(0, 0, W, 1, RED)
    c.spans(2, 0, [("\\_ TORRO", WHITE, True), ("CAST", SILVER, True), (" _/", WHITE, True)])
    c.text(W - 8, 0, "v0.1.0", fg=VERSION)

    bottom = rows - 1 - (3 if player else 0)
    c.panel(0, 1, MENU, bottom - 1, "Menü")
    for index, entry in enumerate(entries):
        y = 3 + index
        if entry == active:
            c.rect(1, y, MENU - 2, 1, RED)
            c.text(2, y, f"{index + 1}  {entry}", fg=WHITE, bold=True)
        else:
            c.spans(2, y, [(f"{index + 1}  ", FAINT), entry])
        if badges and entry in badges:
            badge = f" {badges[entry]} "
            c.text(MENU - 2 - len(badge), y, badge, fg=WHITE, bold=True, bg=ACCENT)
    c.text(2, bottom - 2, "Podcasts im Terminal.", fg=FAINT)

    x = 1
    for key, label in hints:
        x = c.text(x, rows - 1, f" {key} ", bold=True, bg=KEY)
        x = c.text(x, rows - 1, f" {label}   ", fg=MUTED)
    return c, (MENU, 1, W - MENU, bottom - 1)


def tabs(c, x, y, names, active):
    for name in names:
        if name == active:
            c.text(x, y, name, bold=True)
            c.rect(x, y + 1.05, len(name), 0.12, ACCENT)
        else:
            c.text(x, y, name, fg=MUTED)
        x += len(name) + 4


def selected(c, x, y, w, h=1):
    c.rect(x, y, w, h, SELECTION)


# ── 1. Suche ─────────────────────────────────────────────────────────────────

def suche():
    c, (x, y, w, h) = frame(27, "Entdecken", V01,
                            [("↑↓", "Auswahl"), ("enter", "Öffnen"), ("tab", "Reiter"), ("esc", "Eingabe verlassen"), ("1-3", "Menü")])
    c.panel(x, y, w, 6, "Entdecken", focused=True)
    tabs(c, x + 3, y + 1, ["Suche", "Charts", "Kategorien"], "Suche")
    c.spans(x + 3, y + 4, [("⌕", MUTED), " lage der nation", ("▏", ACCENT)])

    lw = 42
    c.panel(x, y + 6, lw, h - 6, "12 Treffer")
    results = [
        ("Lage der Nation", "Philip Banse & Ulf Buermeyer · Politik"),
        ("Die Lage – International", "DER SPIEGEL · Nachrichten"),
        ("Zur Lage der Nation (Archiv)", "Deutschlandfunk · Politik"),
        ("Lagebesprechung", "Table.Media · Wirtschaft"),
        ("Nation of Plebs", "Studio Bummens · Comedy"),
    ]
    for index, (title, meta) in enumerate(results):
        row = y + 7 + index * 3
        if index == 0:
            selected(c, x + 1, row, lw - 2, 2)
        c.text(x + 2, row, title, bold=index == 0)
        c.text(x + 2, row + 1, meta, fg=MUTED)

    px = x + lw
    c.panel(px, y + 6, w - lw, h - 6, "Vorschau")
    c.text(px + 2, y + 7, "Lage der Nation", bold=True)
    c.text(px + 2, y + 8, "Der Politik-Podcast aus Berlin")
    c.text(px + 2, y + 9, "Philip Banse & Ulf Buermeyer", fg=MUTED)
    c.text(px + 2, y + 11, "Politik · Nachrichten")
    c.text(px + 2, y + 12, "497 Folgen · zuletzt heute")
    c.spans(px + 2, y + 14, [("Quelle        ", MUTED), "Apple"])
    c.spans(px + 2, y + 15, [("Website       ", MUTED), ("lagedernation.org", CYAN)])
    for offset, line in enumerate(["Beschreibung und Folgen", "erscheinen, sobald du den", "Podcast öffnest."]):
        c.text(px + 2, y + 17 + offset, line, fg=FAINT)
    return c


# ── 2. Podcast ───────────────────────────────────────────────────────────────

def podcast():
    c, (x, y, w, h) = frame(24, "Entdecken", V01,
                            [("↑↓", "Folge"), ("enter", "Öffnen"), ("/", "Filtern"), ("o", "Reihenfolge"), ("w", "Website"), ("esc", "Zurück")])
    c.panel(x, y, w, 11, "Entdecken › Lage der Nation")
    # The cover: a real image where the terminal can show one.
    c.back.append(
        f'<rect x="{(x + 2) * CW:.1f}" y="{(y + 1.2) * CH:.1f}" width="{12 * CW:.1f}" height="{5.6 * CH:.1f}" '
        f'rx="4" fill="#23384f"/>'
    )
    c.text(x + 6, y + 3, "LAGE", fg="#f0c94a", bold=True)
    c.text(x + 3, y + 4, "der Nation", fg=WHITE)
    tx = x + 17
    c.text(tx, y + 1, "Lage der Nation – der Politik-Podcast aus Berlin", bold=True)
    c.text(tx, y + 2, "Philip Banse & Ulf Buermeyer", fg=MUTED)
    c.text(tx, y + 4, "Politik · Nachrichten · Deutsch · 497 Folgen")
    c.text(tx, y + 5, "lagedernation.org", fg=CYAN)
    c.spans(tx, y + 6, [("♥", ACCENT), (" Unterstützen  ", MUTED), ("plus.lagedernation.org", CYAN)])
    c.text(x + 2, y + 8, "Jede Woche besprechen der Journalist Philip Banse und der Jurist Ulf")
    c.text(x + 2, y + 9, "Buermeyer die politischen Ereignisse der Woche …")
    c.spans(x + w - 12, y + 9, [(" m ", TEXT, True, KEY), (" mehr", MUTED)])

    c.panel(x, y + 11, w, h - 11, "Folgen · neueste zuerst", focused=True)
    episodes = [
        ("LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen", "19.09.", "1:34:10", "▤ ¶"),
        ("LdN 411 · Sommerinterview, Bahn, Chatkontrolle", "12.09.", "1:28:45", "▤ ¶"),
        ("LdN 410 · Spezial: Wie funktioniert der Bundesrat?", "05.09.", "  58:02", "▤"),
        ("LdN 409 · Stromsteuer, Richterwahl, Ukraine", "29.08.", "1:41:33", "▤ ¶"),
        ("LdN 408 · Sommerpause – Hörerfragen", "22.08.", "1:12:09", "▤"),
        ("LdN 407 · Koalitionsausschuss, Hitzeschutz", "15.08.", "1:30:51", "▤ ¶"),
    ]
    for index, (title, date, length, marks) in enumerate(episodes):
        row = y + 12 + index
        if index == 0:
            selected(c, x + 1, row, w - 2)
        c.text(x + 2, row, title, bold=index == 0)
        c.text(x + 56, row, date, fg=MUTED)
        c.text(x + 64, row, length, fg=MUTED)
        c.text(x + 73, row, marks, fg=CYAN)
    c.spans(x + 2, y + h - 2, [("▤", CYAN), (" Kapitel   ", FAINT), ("¶", CYAN), (" Transkript", FAINT)])
    return c


# ── 3. Folge ─────────────────────────────────────────────────────────────────

def folge():
    c, (x, y, w, h) = frame(24, "Entdecken", V01,
                            [("tab", "Shownotes/Kapitel"), ("↑↓", "Blättern"), ("1-9", "Link öffnen"), ("w", "Im Browser"), ("esc", "Zurück")])
    c.panel(x, y, w, 4, "Entdecken › Lage der Nation › LdN 412")
    c.text(x + 2, y + 1, "LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen", bold=True)
    c.text(x + 2, y + 2, "19. September 2026 · 1:34:10 · Folge 412", fg=MUTED)

    kw = 36
    nw = w - kw
    c.panel(x, y + 4, nw, h - 4, "Shownotes")
    notes = [
        ["Begrüßung und Hausmitteilungen. Wir"],
        ["sind im Oktober live in Leipzig –"],
        ["Karten gibt es hier ", ("[1]", CYAN), "."],
        [],
        [("Haushalt 2027", SILVER, True)],
        [("• ", MUTED), "Kabinettsbeschluss im Überblick ", ("[2]", CYAN)],
        [("• ", MUTED), "Kritik des Bundesrechnungshofs ", ("[3]", CYAN)],
        [],
        [("Rentenpaket", SILVER, True)],
        ["Was im Gesetzentwurf steht und was"],
        ["die Kommission noch klären soll ", ("[4]", CYAN), "."],
        [],
        [("Links", SILVER, True)],
        [("[1] ", CYAN), ("lagedernation.org/live", MUTED)],
        [("[2] ", CYAN), ("bundesfinanzministerium.de/…", MUTED)],
    ]
    for offset, line in enumerate(notes):
        c.spans(x + 2, y + 5 + offset, line)

    kx = x + nw
    c.panel(kx, y + 4, kw, h - 4, "Kapitel · 7", focused=True)
    chapters = [
        ("00:00:00", "Begrüßung", False), ("00:03:12", "Hausmitteilungen", False),
        ("00:06:40", "Haushalt 2027", False), ("00:31:05", "Rentenpaket", False),
        ("00:58:47", "Wahl in Norwegen", True), ("01:17:20", "Chatkontrolle", False),
        ("01:29:02", "Verabschiedung", False),
    ]
    for index, (time, title, link) in enumerate(chapters):
        row = y + 5 + index
        if index == 0:
            selected(c, kx + 1, row, kw - 2)
        c.text(kx + 2, row, time, fg=MUTED)
        c.text(kx + 12, row, title, bold=index == 0)
        if link:
            c.text(kx + kw - 4, row, "↗", fg=CYAN)
    c.text(kx + 2, y + 13, "Aus dem Feed (Podlove)", fg=FAINT)
    return c


# ── 4. Einstellungen › Quellen ───────────────────────────────────────────────

def quellen():
    c, (x, y, w, h) = frame(22, "Einstellungen", V01,
                            [("↑↓", "Quelle"), ("leertaste", "An/Aus"), ("enter", "Einrichten"), ("tab", "Reiter"), ("1-3", "Menü")])
    c.panel(x, y, w, 4, "Einstellungen")
    tabs(c, x + 3, y + 1, ["Quellen", "Bibliothek", "Darstellung", "Tasten"], "Quellen")

    c.panel(x, y + 4, w, h - 4, "Wo TorroCast nach Podcasts sucht", focused=True)
    sources = [
        ("●", GREEN, "Apple Podcasts", "Aktiv", TEXT,
         ["Suche, Charts und Kategorien. Braucht keine Einrichtung."]),
        ("○", AMBER, "Podcast Index", "Kein Schlüssel hinterlegt", AMBER,
         ["Offenes Verzeichnis mit Trending und Kapitel-Hinweisen.", "Du brauchst einen eigenen, kostenlosen Schlüssel."]),
        ("○", MUTED, "fyyd", "Ausgeschaltet", MUTED,
         ["Deutschsprachiges Verzeichnis. Braucht keine Einrichtung."]),
    ]
    row = y + 6
    for index, (mark, colour, name, state, state_colour, lines) in enumerate(sources):
        if index == 0:
            selected(c, x + 1, row, w - 2, 1 + len(lines))
        c.text(x + 2, row, mark, fg=colour)
        c.text(x + 5, row, name, bold=True)
        c.text(x + w - 3 - len(state), row, state, fg=state_colour)
        for offset, line in enumerate(lines):
            c.text(x + 5, row + 1 + offset, line, fg=MUTED)
        row += len(lines) + 2
    c.spans(x + 2, row, [("Land für Suche und Charts      ", MUTED), "Deutschland"])
    return c


# ── 5. Rahmen ab v0.2 ────────────────────────────────────────────────────────

def rahmen():
    rows = 20
    c, (x, y, w, h) = frame(rows, "Abos", V02,
                            [("leertaste", "Pause"), ("b f", "±30 s"), ("< >", "Kapitel"), ("- +", "Tempo"), ("a", "Warteschlange"), ("1-7", "Menü")],
                            badges={"Neue Folgen": 6, "Warteschlange": 3}, player=True)
    c.panel(x, y, w, h, "Abos · 23", focused=True)
    subscriptions = [
        ("Lage der Nation", "2 neu", "zuletzt heute"), ("Logbuch:Netzpolitik", "1 neu", "zuletzt gestern"),
        ("Methodisch inkorrekt", "", "zuletzt 14.09."), ("Chaosradio", "", "zuletzt 28.08."),
        ("Freak Show", "3 neu", "zuletzt 11.09."),
    ]
    for index, (title, new, last) in enumerate(subscriptions):
        row = y + 1 + index
        if index == 0:
            selected(c, x + 1, row, w - 2)
        c.text(x + 2, row, title, bold=index == 0)
        c.text(x + 50, row, new, fg=GREEN)
        c.text(x + 58, row, last, fg=MUTED)

    top = rows - 4
    c.rect(0, top + 0.45, W, 0.06, LINE)
    c.spans(1, top + 1, [("▶  ", ACCENT, True), ("LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen", TEXT, True),
                         (" — Lage der Nation", MUTED)])
    c.spans(W - 20, top + 1, [("1,3×", TEXT, True), "    ", ("⏾", MUTED), (" 30 min", MUTED)])
    c.text(4, top + 2, "00:41:20", fg=MUTED)
    start, length, played = 14, 62, 27
    c.rect(start, top + 2.42, length, 0.16, LINE)
    c.rect(start, top + 2.42, played, 0.16, ACCENT)
    for mark in (2, 4, 20, 39, 51, 59):   # chapter boundaries
        c.rect(start + mark, top + 2.25, 0.12, 0.5, TERMINAL)
    c.back.append(f'<circle cx="{(start + played) * CW:.1f}" cy="{(top + 2.5) * CH:.1f}" r="4.5" fill="{ACCENT}"/>')
    c.text(start + length + 2, top + 2, "1:34:10", fg=MUTED)
    c.text(start + length + 12, top + 2, "Rentenpaket", fg=SILVER)
    return c


if __name__ == "__main__":
    OUT.mkdir(exist_ok=True)
    for name, build in {"suche": suche, "podcast": podcast, "folge": folge, "quellen": quellen, "rahmen": rahmen}.items():
        (OUT / f"{name}.svg").write_text(build().svg() + "\n")
        print("wrote", OUT / f"{name}.svg")
