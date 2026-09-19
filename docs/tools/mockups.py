"""Builds the aligned TUI mockups for docs/oberflaeche.md."""
import json, sys

W = 104          # full terminal width of the mockups
MENU = 26        # same menu width as TorroMail


def pad(text, width):
    text = text[:width]
    return text + " " * (width - len(text))


def box(title, lines, width, height=None):
    inner = width - 2
    top = "╭ " + title + " " + "─" * (inner - len(title) - 2) + "╮"
    body = [("│" + pad(line, inner) + "│") for line in lines]
    if height:
        while len(body) < height - 2:
            body.append("│" + " " * inner + "│")
        body = body[: height - 2]
    return [top] + body + ["╰" + "─" * inner + "╯"]


def hjoin(*boxes):
    height = max(len(b) for b in boxes)
    out = []
    for i in range(height):
        out.append("".join(b[i] if i < len(b) else " " * len(b[0]) for b in boxes))
    return out


def vjoin(*boxes):
    out = []
    for b in boxes:
        out.extend(b)
    return out


def bar(version="v0.1.0"):
    left = "  \\_ TORROCAST _/"
    return [left + " " * (W - len(left) - len(version) - 2) + version + "  "]


def hints(pairs):
    return [" " + "   ".join(f"[{k}] {v}" for k, v in pairs)]


def menu(active, entries, height, tagline=True):
    lines = [""]
    for i, e in enumerate(entries, 1):
        mark = "▌" if e == active else " "
        lines.append(f"{mark}{i}  {e}")
    b = box("Menü", lines, MENU, height)
    if tagline:
        b[-3] = "│" + pad(" Podcasts im Terminal.", MENU - 2) + "│"
    return b


def screen(active, entries, content, hint_pairs, player=None):
    height = len(content)
    rows = bar() + hjoin(menu(active, entries, height), content)
    if player:
        rows += player
    rows += hints(hint_pairs)
    return rows


CW = W - MENU
V01 = ["Entdecken", "Einstellungen", "Hilfe"]
V02 = ["Entdecken", "Abos", "Neue Folgen", "Warteschlange", "Downloads", "Einstellungen", "Hilfe"]

# ── 1. Suche ────────────────────────────────────────────────────────────────
LW = 42
search = vjoin(
    box("Entdecken", [
        "  Suche    Charts    Kategorien",
        "  ━━━━━",
        "",
        "  ⌕ lage der nation▏",
    ], CW),
    hjoin(
        box("12 Treffer", [
            "▌Lage der Nation",
            "▌Philip Banse & Ulf Buermeyer · Politik",
            "",
            " Die Lage – International",
            " DER SPIEGEL · Nachrichten",
            "",
            " Zur Lage der Nation (Archiv)",
            " Deutschlandfunk · Politik",
            "",
            " Lagebesprechung",
            " Table.Media · Wirtschaft",
            "",
            " Nation of Plebs",
            " Studio Bummens · Comedy",
        ], LW, 18),
        box("Vorschau", [
            " Lage der Nation",
            " Der Politik-Podcast aus Berlin",
            " Philip Banse & Ulf Buermeyer",
            "",
            " Politik · Nachrichten",
            " 497 Folgen · zuletzt heute",
            "",
            " Quelle        Apple",
            " Website       lagedernation.org",
            "",
            " Beschreibung und Folgen",
            " erscheinen, sobald du den",
            " Podcast öffnest.",
        ], CW - LW, 18),
    ),
)
S1 = screen("Entdecken", V01, search,
            [("↑↓", "Auswahl"), ("enter", "Öffnen"), ("tab", "Reiter"), ("esc", "Eingabe verlassen"), ("1-3", "Menü")])

# ── 2. Podcast ──────────────────────────────────────────────────────────────
podcast = vjoin(
    box("Entdecken › Lage der Nation", [
        " ▄▄▄▄▄▄▄▄▄▄▄▄   Lage der Nation – der Politik-Podcast aus Berlin",
        " █          █   Philip Banse & Ulf Buermeyer",
        " █  Cover   █",
        " █          █   Politik · Nachrichten · Deutsch · 497 Folgen",
        " █          █   lagedernation.org",
        " ▀▀▀▀▀▀▀▀▀▀▀▀   ♥ Unterstützen: plus.lagedernation.org",
        "",
        " Jede Woche besprechen der Journalist Philip Banse und der Jurist Ulf",
        " Buermeyer die politischen Ereignisse der Woche …                 [m] mehr",
    ], CW),
    box("Folgen · neueste zuerst", [
        "▌LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen     19.09.  1:34:10  ▤ ¶",
        " LdN 411 · Sommerinterview, Bahn, Chatkontrolle        12.09.  1:28:45  ▤ ¶",
        " LdN 410 · Spezial: Wie funktioniert der Bundesrat?    05.09.    58:02  ▤",
        " LdN 409 · Stromsteuer, Richterwahl, Ukraine           29.08.  1:41:33  ▤ ¶",
        " LdN 408 · Sommerpause – Hörerfragen                   22.08.  1:12:09  ▤",
        " LdN 407 · Koalitionsausschuss, Hitzeschutz            15.08.  1:30:51  ▤ ¶",
        "",
        " ▤ Kapitel   ¶ Transkript",
    ], CW, 10),
)
S2 = screen("Entdecken", V01, podcast,
            [("↑↓", "Folge"), ("enter", "Öffnen"), ("/", "Filtern"), ("o", "Reihenfolge"), ("w", "Website"), ("esc", "Zurück")])

# ── 3. Folge ────────────────────────────────────────────────────────────────
KW = 36
episode = vjoin(
    box("Entdecken › Lage der Nation › LdN 412", [
        " LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen",
        " 19. September 2026 · 1:34:10 · Staffel –, Folge 412",
    ], CW),
    hjoin(
        box("Shownotes", [
            " Begrüßung und Hausmitteilungen. Wir",
            " sind im Oktober live in Leipzig –",
            " Karten gibt es hier [1].",
            "",
            " Haushalt 2027",
            " • Kabinettsbeschluss im Überblick [2]",
            " • Kritik des Bundesrechnungshofs [3]",
            "",
            " Rentenpaket",
            " Was im Gesetzentwurf steht und was",
            " die Kommission noch klären soll [4].",
            "",
            " Links",
            " [1] lagedernation.org/live",
            " [2] bundesfinanzministerium.de/…",
        ], CW - KW, 18),
        box("Kapitel · 7", [
            "▌00:00:00  Begrüßung",
            " 00:03:12  Hausmitteilungen",
            " 00:06:40  Haushalt 2027",
            " 00:31:05  Rentenpaket",
            " 00:58:47  Wahl in Norwegen    ↗",
            " 01:17:20  Chatkontrolle",
            " 01:29:02  Verabschiedung",
            "",
            " Aus dem Feed (Podlove)",
        ], KW, 18),
    ),
)
S3 = screen("Entdecken", V01, episode,
            [("tab", "Shownotes/Kapitel"), ("↑↓", "Blättern"), ("1-9", "Link öffnen"), ("w", "Im Browser"), ("esc", "Zurück")])

# ── 4. Quellen ──────────────────────────────────────────────────────────────
sources = vjoin(
    box("Einstellungen", [
        "  Quellen    Bibliothek    Darstellung    Tasten",
        "  ━━━━━━━",
    ], CW),
    box("Wo TorroCast nach Podcasts sucht", [
        "",
        "▌●  Apple Podcasts                                              Aktiv",
        "▌   Suche, Charts und Kategorien. Braucht keine Einrichtung.",
        "",
        " ○  Podcast Index                               Kein Schlüssel hinterlegt",
        "    Offenes Verzeichnis mit Trending und Kapitel-Hinweisen.",
        "    Du brauchst einen eigenen, kostenlosen Schlüssel.",
        "",
        " ○  fyyd                                                 Ausgeschaltet",
        "    Deutschsprachiges Verzeichnis. Braucht keine Einrichtung.",
        "",
        " Land für Suche und Charts      Deutschland",
    ], CW, 16),
)
S4 = screen("Einstellungen", V01, sources,
            [("↑↓", "Quelle"), ("leertaste", "An/Aus"), ("enter", "Einrichten"), ("tab", "Reiter"), ("1-3", "Menü")])

# ── 5. Rahmen v0.2 mit Wiedergabeleiste ─────────────────────────────────────
subs = vjoin(
    box("Abos · 23", [
        "▌Lage der Nation                                  2 neu   zuletzt heute",
        " Logbuch:Netzpolitik                              1 neu   zuletzt gestern",
        " Methodisch inkorrekt                                     zuletzt 14.09.",
        " Chaosradio                                               zuletzt 28.08.",
        " Freak Show                                       3 neu   zuletzt 11.09.",
    ], CW, 14),
)
player = [
    "─" * W,
    pad(" ▶  LdN 412 · Haushalt, Rentenpaket, Wahl in Norwegen — Lage der Nation", W - 22) + pad("1,3×   ⏾ 30 min", 22),
    pad("    00:41:20 ━━━━━━━━━━━━━━━━━━━┿━━━━━━━━━━━━●────────────────────────────── 1:34:10   Rentenpaket", W),
]
S5 = screen("Abos", V02, subs,
            [("leertaste", "Pause"), ("b f", "±30 s"), ("< >", "Kapitel"), ("- +", "Tempo"), ("a", "In Warteschlange"), ("1-7", "Menü")],
            player=player)

out = {"suche": S1, "podcast": S2, "folge": S3, "quellen": S4, "rahmen": S5}
for name, rows in out.items():
    rows[:] = [r.rstrip() for r in rows]
    widths = {len(r) for r in rows}
    assert max(widths) <= W, (name, max(widths))
json.dump(out, sys.stdout, ensure_ascii=False)
