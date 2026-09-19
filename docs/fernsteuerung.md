# Fernsteuerung

TorroCast lässt sich von außen befragen und steuern — von einem Widget in der Leiste des Desktops,
einem Skript oder einem zweiten Terminal. Das Omarchy-Plugin
([omarchy-torrocast](https://github.com/mahype/omarchy-torrocast)) besteht aus nichts anderem.

## Drei Befehle

| Befehl | Was er tut |
| --- | --- |
| `torrocast daemon` | TorroCast ohne Fenster: spielt, merkt sich Stellen, hört auf Medientasten (MPRIS) und auf `ctl`. |
| `torrocast status` | Ein JSON-Dokument: was läuft, Up Next, neue Folgen. Läuft nichts: `{"running": false, …}`. |
| `torrocast ctl <verb>` | Steuert das laufende TorroCast. `torrocast --help` nennt alle Verben. |

`ctl start`, `ctl play` und `ctl toggle` starten TorroCast ohne Fenster, wenn keins läuft —
wer Ton will, soll ihn bekommen. Alle anderen Verben antworten dann mit `running: false` und
Exit-Code 3. Ein unbekanntes Verb oder eine unbekannte Folge ergibt `{"error": "…"}` und Exit-Code 1.

## Eins spielt, nicht zwei

Wer den Socket hält, spielt — und schreibt das Journal dieses Geräts in der Bibliothek.

- Startet die **TUI**, während TorroCast ohne Fenster läuft, bittet sie es zu gehen, wartet, bis
  der Socket frei ist, und spielt an derselben Stelle weiter.
- Eine **zweite TUI** startet nicht: „TorroCast ist schon in einem anderen Fenster offen.“
- Schließt man die TUI, endet die Wiedergabe wie bisher. (Die TUI als bloßes Fenster auf ein
  dauerhaft laufendes TorroCast wäre der nächste Schritt; siehe `offen.md`.)

## Der Socket

`$XDG_RUNTIME_DIR/torrocast.sock`, ohne dieses Verzeichnis (macOS) im Cache-Ordner. Eine Zeile JSON
hinein, eine Zeile JSON heraus; jede Antwort auf ein verstandenes Verb ist das Status-Dokument.
Der Socket wird aus derselben Schleife bedient, die den Core dreht — kein Thread, kein Lock.
Unter Windows gibt es ihn noch nicht; `status` meldet dort immer `running: false`.

```
{"do":"status"}                         {"do":"toggle"}  {"do":"pause"}  {"do":"stop"}
{"do":"play"}  {"do":"play","key":K}    {"do":"next"}  {"do":"next-chapter"}  {"do":"previous-chapter"}
{"do":"seek-by","seconds":-30}          {"do":"seek-to","ms":90000}
{"do":"speed-by","step":0.1}            {"do":"sleep"}
{"do":"enqueue","key":K,"first":true}   {"do":"remove","key":K}  {"do":"up","key":K}  {"do":"down","key":K}
{"do":"refresh"}                        {"do":"quit"}
```

Folgen werden mit ihrem `key` benannt, nie mit ihrem Platz in einer Liste: Zwischen dem letzten
Blick eines Widgets und dem Klick kann sich die Liste verschoben haben. `play` pausiert nie,
`pause` startet nie — beide sind gefahrlos mehrfach zu senden.

## Das Status-Dokument

```json
{
  "running": true, "mode": "daemon", "version": "0.1.0", "language": "de",
  "now": {
    "key": "…", "title": "…", "podcast": "…", "artwork_url": "…",
    "status": "playing", "problem": null,
    "position_ms": 166000, "duration_ms": 4729000,
    "chapter": { "index": 3, "count": 8, "title": "…" }
  },
  "speed": 1.2,
  "sleep": null,
  "up_next": [ { "key": "…", "title": "…", "podcast": "…", "duration_ms": 1, "artwork_url": "…", "heard": 0.017 } ],
  "new_episodes": [ { "…wie up_next…": "", "published": "2026-09-19T05:00:00+00:00", "queued": false } ],
  "new_pending": 0, "new_failed": 0,
  "subscriptions": 2
}
```

`status` ist `loading`, `playing`, `paused` oder `failed` (dann steht der Grund in `problem`).
`sleep` ist `null`, `{"minutes": n}` oder `"end-of-episode"`. Felder kommen hinzu, keine
verschwinden: Wer liest, ignoriert, was er nicht kennt.
