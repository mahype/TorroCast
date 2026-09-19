# Datenablage und Sync über einen frei wählbaren Ordner

Stand: 19.09.2026. Webrecherche, soweit Quellen abrufbar waren. Nicht verifizierte Aussagen sind mit **[nicht verifiziert]** markiert und in Abschnitt 8 gesammelt.

Kurzfassung der Empfehlung:

- Im synchronisierten Ordner liegt **keine SQLite-Datei**.
- Der "Library-Ordner" enthält pro Gerät ein eigenes Unterverzeichnis mit einem Append-only-Journal (JSONL) und einem kompaktierten Geräte-Snapshot. Nur dieses Gerät schreibt dort. So schreiben nie zwei Geräte dieselbe Datei, und Sync-Konflikte sind konstruktiv ausgeschlossen.
- Der globale Zustand ist die Vereinigung aller Geräte-Dateien: ein zustandsbasierter CRDT aus LWW-Registern mit Hybrid Logical Clocks (HLC).
- Die lokale SQLite-DB im XDG-Verzeichnis ist nur eine materialisierte Sicht plus Cache. Sie ist jederzeit neu aufbaubar.
- Eine vollwertige CRDT-Bibliothek (Automerge, Loro, cr-sqlite) ist für diesen Datentyp Overkill.

---

## 1. Warum eine Live-SQLite-DB in Dropbox/Syncthing/iCloud gefährlich ist

### 1.1 Was SQLite selbst sagt

**Der DB-Zustand ist auf mehrere Dateien verteilt.** "How To Corrupt An SQLite Database File", §1.4: "The state of an SQLite database is controlled by both the database file and the journal file." Als "likely to lead to corruption" nennt das Dokument u. a. "Copying a database file without also copying its journal" und "Overwriting a database file with another without also deleting any hot journal". Ein Datei-Sync-Dienst tut genau das: Er überträgt `db`, `db-wal` und `db-shm` unabhängig voneinander, zu verschiedenen Zeitpunkten und ohne Transaktionsgrenzen zu kennen. https://www.sqlite.org/howtocorrupt.html

**Kopieren während einer Transaktion (§1.2).** "Systems that run automatic backups in the background might try to make a backup copy of an SQLite database file while it is in the middle of a transaction. The backup copy then might contain some old and some new content, and thus be corrupt." Sichere Wege sind nur `sqlite3_rsync` (ab 3.47.0), `VACUUM INTO` und die Backup-API. Ein Sync-Client nutzt keinen davon. https://www.sqlite.org/howtocorrupt.html

**WAL.**
- "The WAL file is part of the persistent state of the database and should be kept with the database … If a database file is separated from its WAL file, then transactions that were previously committed to the database might be lost, or the database file might become corrupted."
- "All processes using a database must be on the same host computer; WAL does not work over a network filesystem", weil der `-shm`-Index Shared Memory ist.

https://www.sqlite.org/wal.html

**Locking (§2.1).** SQLite verlässt sich auf Dateisystem-Locks. "This is especially true of network filesystems and NFS in particular." Das betrifft den NAS-Mount-Fall. Über Dropbox oder Syncthing hinweg gibt es überhaupt keine geräteübergreifenden Locks. https://www.sqlite.org/howtocorrupt.html

**Netzwerk-Dateisysteme allgemein.** "Rely upon it at your (and your customers') peril." Genannte Gründe: unzuverlässiges `fsync`, fehlerhafte Locks und Out-of-order-Writes. https://www.sqlite.org/useovernet.html

**Umbenennen oder Ersetzen einer geöffneten DB (§2.5).** Das Verhalten ist "undefined and probably undesirable". Genau das passiert, wenn der Sync-Client eine von remote empfangene Version per Rename über die lokal geöffnete DB legt. https://www.sqlite.org/howtocorrupt.html

### 1.2 Was in der Praxis passiert

**Zotero** warnt offiziell: "Storing your Zotero data directory in a cloud storage folder (Dropbox, Google Drive, OneDrive, etc) is extremely likely to corrupt your database … cloud storage systems generally don't honor such locks … The Zotero Forums contain countless reports over many years of database corruption". https://www.zotero.org/support/kb/data_directory_in_cloud_storage_folder

Weitere Berichte über "conflicted copy" von SQLite-Dateien samt `-journal`:
- QOwnNotes: https://github.com/pbek/QOwnNotes/issues/1589
- Zotero-Forum: https://forums.zotero.org/discussion/7260/conflict-copy-of-sqlite
- sqlite-users-Thread: https://sqlite-users.sqlite.narkive.com/Xovt5OLG/sqlite-database-on-dropbox-google-drive-ms-skydrive-ubuntu-one-or-samba-share
- ActivityWatch-Diskussion: https://github.com/ActivityWatch/activitywatch/issues/35

Selbst ohne Korruption bleibt das Grundproblem bestehen. Bei gleichzeitiger Änderung auf zwei Geräten entsteht eine "conflicted copy" der gesamten Binärdatei. Einer der beiden Stände ist faktisch verloren, weil niemand zwei SQLite-Dateien von Hand zusammenführt.
- Dropbox hängt Nutzername, "conflicted copy" und Datum an. Als Konfliktkopie erscheint immer die zuletzt gespeicherte Version. https://help.dropbox.com/organize/conflicted-copy
- Syncthing benennt um in `<filename>.sync-conflict-<date>-<time>-<modifiedBy>.<ext>`. Die Datei mit der älteren mtime wird zur Konfliktkopie. Bei "Änderung gegen Löschung" wird die geänderte Datei zur Konfliktkopie, falls die Löschung gewinnt. Konfliktdateien werden wie normale Dateien weiter synchronisiert. https://docs.syncthing.net/users/syncing.html

**Fazit:** SQLite gehört ins lokale, nicht synchronisierte App-Verzeichnis. Das Sync-Format muss für "dumme" Dateireplikation entworfen sein: kleine Dateien, ein einziger Schreiber pro Datei, mergebar, tolerant gegenüber veralteten und doppelten Dateien.

---

## 2. Wie andere Apps "Sync über einen dummen Ordner" lösen

### 2.1 Instruktive Beispiele

| App | Ansatz | Lehre für TorroCast |
|---|---|---|
| **Joplin** (Filesystem-/WebDAV-Target) | Ein Item pro Datei auf dem Sync-Target **[Item-pro-Datei-Layout aus Vorwissen, in der abgerufenen Seite nicht bestätigt]**. `info.json` hält Target-weite Eigenschaften: "used to ensure all clients work with the same sync settings" (E2EE-Status, Mindest-App-Version). Locks sind Dateien `<lockType>_<clientType>_<clientId>.json` mit Ablaufzeit. Sync-Locks sind parallel erlaubt, ein Exclusive-Lock nur für Target-Upgrades. Die Gültigkeit hängt an Datei-Zeitstempeln, also halbwegs synchronen Uhren. https://joplinapp.org/help/dev/spec/sync/ · https://joplinapp.org/help/dev/spec/sync_lock/ | Eine `format.json` mit Mindestversionen ist sinnvoll. Lock-Dateien auf einem asynchron replizierten Ordner sind prinzipiell unzuverlässig, denn der Lock kommt eventuell erst nach Minuten an. Besser ist ein Design, das keine Locks braucht. |
| **1Password OPVault** (altes Dropbox-Format) | `profile.js`, `folders.js` und 16 Band-Dateien `band_0.js`…`band_F.js` nach UUID-Präfix. Begründung: eine Datei pro Item "creates difficulties for some filesystems … There is an overhead for syncing each individual file regardless of its size." Löschungen sind das Flag `trashed: true`, also ein Tombstone. Pro Item gibt es einen `tx`-Timestamp. https://support.1password.com/cs/opvault-design/ | Nicht tausende Minidateien anlegen, sondern Dateien in vernünftiger Größe bündeln. Tombstones statt echter Löschung. Bands werden aber von mehreren Geräten beschrieben und sind damit konfliktanfällig. Das vermeiden wir. |
| **KeePass "Synchronize"** | Merge auf Entry-Ebene per Last-Modification-Time. Bewusst kein Feld-Merge: "combinations could become inconsistent". Die unterlegene Version wandert in die History. Löschungen werden separat als "deleted objects" geführt **[Letzteres aus Vorwissen]**. https://keepass.info/help/v2/sync.html | Das Register auf der fachlich konsistenten Einheit wählen, z. B. {Position, gespielt} gemeinsam. Der LWW-Verlierer sollte nicht spurlos verschwinden. Bei uns bleibt er im Journal. |
| **Taskwarrior 3 / TaskChampion** | Operationen `Create`, `Delete`, `Update(uuid, property, value, timestamp)` sind idempotent. Es gibt eine lineare, verzweigungsfreie Versionskette. Der Server nimmt eine Version nur an, wenn deren Parent die neueste ist. Replicas rebasen per Operational Transform. https://gothenburgbitfactory.org/taskchampion/sync-model.html | Das Operationsmodell (property-weises Update mit Timestamp) passt gut. Die lineare Kette braucht aber Compare-and-Swap auf dem Server. Ein dummer Ordner kann das nicht, deshalb setzen wir auf einen kommutativen Merge ohne Reihenfolge. |
| **Actual Budget / James Long, "CRDTs for Mortals"** | Nachrichten `(dataset, row, column, value, HLC-timestamp)`. LWW pro Zelle, Merge kommutativ und idempotent. Der HLC ist ca. 250 Zeilen groß. https://github.com/jlongster/crdt-example-app · https://imfeld.dev/writing/crdts_for_mortals · https://jaredforsyth.com/posts/hybrid-logical-clocks/ (Die offizielle Actual-Doku beschreibt nur "sync id" und Reset und nennt HLC/CRDT nicht explizit: https://actualbudget.org/docs/getting-started/sync/) | Das ist die Blaupause: LWW pro Feld, HLC und ein Nachrichtenlog. Actual nutzt einen Server als Relais. Wir ersetzen ihn durch Journale pro Gerät. |
| **Super Productivity** (Op-Log, 2025/26) | Event-Sourcing mit Vector Clocks. Datei-Provider (Dropbox, WebDAV, LocalFile) arbeiten mit einem Single-File-Snapshot. Die Doku unterscheidet ausdrücklich: Dropbox und OneDrive können API-seitig CAS, "WebDAV/Nextcloud is atomic only when the server supplies strong ETags". Lehre aus einem Vorfall im Februar 2026: "Never prune a vector clock before using it in a comparison". https://github.com/super-productivity/super-productivity/tree/master/docs/sync-and-op-log (dort `vector-clocks.md`) | Eine gemeinsame Datei braucht CAS. Das gibt es nur über Provider-APIs, nicht über einen beliebigen Ordner. Das spricht klar für Dateien mit einem Schreiber pro Gerät. Vector Clocks bringen Pruning-Komplexität. Bei LWW-Registern reicht HLC. |
| **automerge-repo (nodefs-Storage)** | Schlüssel `<docId>/incremental/<change-hash>` und `<docId>/snapshot/<heads-hash>`. "safe for multiple processes to use the same data directory". Die Kompaktierung kommt ohne Locks aus: inhaltsadressierte, unveränderliche Dateien, und gelöscht wird nur, was man selbst geladen hat. https://automerge.org/docs/reference/repositories/storage/ · https://patternist.xyz/posts/concurrent-compaction-in-automerge-repo/ | Die Kompaktierungsregel übernehmen wir sinngemäß: unveränderliche Segmente, und es wird nur gelöscht, was nachweislich im eigenen Snapshot enthalten ist. |
| **Logseq / Obsidian** (Markdown im Sync-Ordner) | Logseq empfiehlt offiziell, den Graph aus iCloud/Dropbox/OneDrive herauszunehmen und nur Logseq Sync zu nutzen, sonst "might result in data loss". Bei iCloud entstehen Duplikate und Event-Probleme. https://blog.logseq.com/how-to-setup-and-use-logseq-sync/ · https://discuss.logseq.com/t/im-using-logseq-with-icloud-but-experiencing-data-loss-or-file-conflict-errors-whats-going-on/13393 | Auch reine Textdateien verlieren Daten, wenn zwei Geräte dieselbe Datei schreiben. Der Grund ist der Mehrfach-Schreiber, nicht SQLite. |
| **Zotero** | Siehe 1.2. Eigener Server-Sync, das Datenverzeichnis ausdrücklich nicht in die Cloud. | Negativbeispiel. |
| **Podcast-Welt** | **AntennaPod:** Sync nur über Server (gpodder.net oder Nextcloud-gpoddersync). Ohne Server bleibt manueller DB-Export/-Import. Feature-Requests nach automatischem Export in Cloud-Ordner sind seit Jahren offen. https://antennapod.org/documentation/general/synchronization · https://github.com/AntennaPod/AntennaPod/issues/4850 <br>**Kasts:** gpodder.net oder nextcloud-gpodder, Subscriptions und Positionen. https://apps.kde.org/kasts/ <br>**gPodder, newsboat, Pocket Casts:** lokale DB plus Server-API bzw. proprietärer Account-Sync **[nicht im Detail recherchiert]**. | Robusten Ordner-Sync bietet praktisch kein Podcast-Client. Das ist eine Marktlücke, und es gibt kein fertiges Format zum Übernehmen. Für Interop zählen OPML und das gpodder-Datenmodell (Feed-URL plus Media-URL). |

Nicht vertieft wurden Standard Notes, Enpass, Anytype, Organice und Buku. Sie nutzen serverbasierten Sync, verschlüsselte Einzeldateien oder git und fügen den Mustern oben nichts Wesentliches hinzu **[nicht recherchiert]**.

### 2.2 CRDT-Bibliotheken – Overkill?

**Local-first-Essay (Ink & Switch).** "Dateien + Dropbox" erfüllt das Multi-Device-Ideal nur teilweise: "if the same file is edited on two different devices, the result is a conflict that needs to be merged manually". CRDTs gelten als Fundament. https://www.inkandswitch.com/essay/local-first/

**Automerge, Yjs, Loro.** Ihre Stärke liegt bei Text, Listen und Bäumen mit feingranularen Einfügungen. Loro 1.0 bietet MovableList und MovableTree und ist in Rust geschrieben: https://loro.dev/blog/v1.0

Diese Bibliotheken kosten drei Dinge:
- ein binäres, nicht menschenlesbares Format, ohne sinnvolles `git diff`;
- die Historie wächst;
- das Dateiformat hängt an einer Bibliotheksversion, was über mehrere Frontends (Rust-Core, später Swift und C#) relevant wird.

**cr-sqlite/vlcn.** Konzeptionell passend (CRDT-Spalten in SQLite, Changesets). Das letzte npm-Release 0.16.3 stammt aber vom Januar 2024. Der Wartungsstand ist fraglich **[nicht abschließend verifiziert]**. https://github.com/vlcn-io/cr-sqlite

**Electric** benötigt Postgres und einen Server und widerspricht damit "kein eigener Server".

**Bewertung.** TorroCasts Daten sind fast ausschließlich Register (Wert pro Schlüssel) plus eine kleine geordnete Liste, die Queue. Dafür genügen ca. 300–500 Zeilen eigener Merge-Code (LWW-Map plus HLC), vollständig per Property-Tests abgesichert. Ein Listen-CRDT wäre nur für die Queue nötig. Dort reicht Fractional Indexing pro Eintrag (siehe 3.5). **CRDT-Theorie ja, CRDT-Bibliothek nein.**

### 2.3 Bausteine des einfachen Designs

- **Ein Schreiber pro Datei.** Jedes Gerät schreibt nur unter `devices/<device-id>/`. Sync-Dienste erzeugen Konflikte nur bei konkurrierenden Änderungen derselben Datei. Konflikte sind damit konstruktiv ausgeschlossen, solange keine Device-ID geklont wird (siehe 3.7).
- **Journal plus Snapshot.** Das Journal ist append-only und segmentiert. Der Snapshot ist der kompaktierte eigene Zustand. Beide sind Darstellungen desselben zustandsbasierten CRDT.
- **HLC statt Wanduhr.** `(physische ms, Zähler, device-id)` ist monoton pro Gerät, robust gegen kleine Uhrenabweichungen und total geordnet. Paper: Kulkarni/Demirbas et al., "Logical Physical Clocks", OPODIS 2014. https://cse.buffalo.edu/tech-reports/2014-04.pdf
- **Tombstones statt Löschen.** `Unsubscribed` ist ein Ereignis, kein Entfernen einer Zeile.
- **Atomare Writes.** Temp-Datei im selben Verzeichnis, `fsync`, `rename`, `fsync(dir)`. Für Appends gilt: eine Zeile entspricht einem `write()` auf `O_APPEND`. Leser ignorieren eine unvollständige letzte Zeile.
- **Keine geräteübergreifenden Locks.** Sie wären über asynchrone Replikation wirkungslos (siehe Joplin oben). Es gibt nur einen lokalen Lock gegen eine zweite Instanz auf demselben Rechner. Dieser liegt im lokalen State-Verzeichnis, nicht im Library-Ordner.

---

## 3. Konkretes Design für TorroCast

### 3.1 Zwei Welten: lokaler Cache gegen synchronisierter Library-Ordner

| | (a) Lokal, nie synchronisiert | (b) Library-Ordner, vom Nutzer gewählt |
|---|---|---|
| Inhalt | SQLite (WAL-Modus ist hier unproblematisch): Feed-Cache, Episoden-Metadaten, Shownotes, Suchindex, materialisierter Merge-Zustand inkl. HLC je Register, Lese-Offsets fremder Journale, letzter eigener HLC; Cover-Art; Downloads (eigener, separat wählbarer Ordner); gerätelokale Settings (Audio-Device, Theme, Download-Pfad, Pfad zum Library-Ordner) | Abos, Episodenstatus, Positionen, Queue, Favoriten/Playlists, geräteübergreifende Settings |
| Größe | MB bis GB | typischerweise unter 1–2 MB |
| Format | binär, intern, frei migrierbar | JSON/JSONL, UTF-8, LF, dokumentiert und versioniert |
| Verlust | harmlos, aus (b) plus Feeds rekonstruierbar | das ist das eigentliche Nutzerdatum |
| Secrets | OS-Keychain (`keyring`-Crate: Secret Service, macOS Keychain, Windows Credential Manager) **[Crate-Details nicht geprüft]** | **niemals** |

Grundsatz: Die lokale DB ist eine View. Geht sie verloren oder wird ein neues Gerät eingerichtet, läuft der Aufbau so ab:
1. Library-Ordner wählen,
2. alles mergen,
3. Feeds neu laden.

### 3.2 Ordnerlayout (Beispiel)

```
torrocast-library/
├── format.json                         # quasi unveränderlich: Format-ID, Versionen
├── README.txt                          # "Nicht von Hand editieren; Aufbau siehe …"
├── subscriptions.opml                  # reiner Export für Interop, NIE gelesen (s. u.)
├── devices/
│   ├── 7f3a9c2e-laptop/                # <device-id>; schreibt NUR dieses Gerät
│   │   ├── device.json                 # Name, Plattform, App-Version, geschriebene Format-Version
│   │   ├── snapshot-000003.json        # kompaktierter EIGENER Zustand (deckt Segmente 1–3 ab)
│   │   ├── journal-000004.jsonl        # versiegelt, unveränderlich
│   │   └── journal-000005.jsonl        # aktives Segment, append-only
│   └── c41d07b8-macmini/
│       ├── device.json
│       ├── snapshot-000011.json
│       └── journal-000012.jsonl
└── exports/
    └── c41d07b8-macmini/subscriptions.opml   # Variante: OPML pro Gerät, s. u.
```

**`format.json`** wird einmal bei der Initialisierung geschrieben, danach nur bei einer bewussten Migration:

```json
{
  "format": "torrocast-library",
  "format_version": 1,
  "min_reader_version": 1,
  "min_writer_version": 1,
  "library_id": "0d9f6f0e-3c58-4a53-9a0b-0c8c1b0e7d11",
  "created": "2026-09-19T10:00:00Z"
}
```

**`subscriptions.opml`** wäre eine von mehreren Geräten geschriebene Datei und damit die einzige Konfliktquelle im Layout.
- **Empfehlung:** Jedes Gerät schreibt das OPML nur nach `exports/<device-id>/subscriptions.opml`, atomar und nur bei Abo-Änderungen.
- **Alternative:** Eine gemeinsame Datei in der Wurzel ist vertretbar, wenn drei Bedingungen gelten: Der Inhalt ist deterministisch aus dem gemergten Zustand erzeugt (sortiert, ohne Zeitstempel), sodass zwei Geräte byte-identische Dateien schreiben. Die App liest die Datei nie. Die App räumt Konfliktkopien dieser einen Datei gefahrlos weg.
- Start mit der Per-Device-Variante.

**Segmentrotation.**
- Das aktive Segment wird bei ca. 256 KiB oder 30 Tagen versiegelt.
- Grund: Jeder Append lässt den Sync-Dienst die Datei neu betrachten. Syncthing überträgt blockweise. Das Delta-Verhalten anderer Dienste ist unterschiedlich **[nicht im Einzelnen verifiziert]**.
- Kleine aktive Segmente halten den Aufwand dienstunabhängig klein.
- Zugleich bleibt die Dateizahl niedrig (OPVault-Lehre).

### 3.3 Ereignisse (JSONL)

Gemeinsame Felder:

| Feld | Bedeutung |
|---|---|
| `v` | Schema-Version der Zeile |
| `id` | `<device-id>:<seq>`, eindeutig, dient dem Dedup |
| `hlc` | HLC als lexikografisch sortierbarer String `<RFC3339-ms>-<counter hex4>-<device-id-kurz>` |
| `t` | Ereignistyp |

Beispielzeilen (jeweils genau eine Zeile, `\n`-terminiert):

```jsonl
{"v":1,"id":"7f3a9c2e:1","hlc":"2026-09-19T10:02:11.120Z-0000-7f3a9c2e","t":"DeviceRegistered","name":"Laptop (Arch)","app":"torrocast-tui 0.1.0"}
{"v":1,"id":"7f3a9c2e:2","hlc":"2026-09-19T10:03:40.551Z-0000-7f3a9c2e","t":"Subscribed","podcast":"917393e3-1b1e-5cef-ace4-edaa54e1f810","feed_url":"https://feeds.example.org/show.xml","title":"Example Show","guid_source":"podcast:guid"}
{"v":1,"id":"7f3a9c2e:3","hlc":"2026-09-19T10:04:02.009Z-0000-7f3a9c2e","t":"PodcastSettingChanged","podcast":"917393e3-1b1e-5cef-ace4-edaa54e1f810","key":"playback_speed","value":1.25}
{"v":1,"id":"7f3a9c2e:4","hlc":"2026-09-19T10:31:15.300Z-0000-7f3a9c2e","t":"PlaybackUpdated","podcast":"917393e3-…","episode":"ep1:b3k7q2…","ref":{"guid":"https://example.org/?p=812","enclosure_url":"https://cdn.example.org/ep812.mp3"},"position_ms":1612000,"duration_ms":3541000,"played":false,"reason":"pause"}
{"v":1,"id":"7f3a9c2e:5","hlc":"2026-09-19T11:05:48.870Z-0000-7f3a9c2e","t":"PlaybackUpdated","podcast":"917393e3-…","episode":"ep1:b3k7q2…","position_ms":3541000,"duration_ms":3541000,"played":true,"reason":"finished"}
{"v":1,"id":"7f3a9c2e:6","hlc":"2026-09-19T11:06:02.100Z-0000-7f3a9c2e","t":"QueueItemSet","episode":"ep1:x9p0aa…","podcast":"4be1…","sort_key":"a0V"}
{"v":1,"id":"7f3a9c2e:7","hlc":"2026-09-19T11:06:30.412Z-0000-7f3a9c2e","t":"QueueItemRemoved","episode":"ep1:b3k7q2…"}
{"v":1,"id":"7f3a9c2e:8","hlc":"2026-09-19T11:07:00.000Z-0000-7f3a9c2e","t":"FavouriteSet","episode":"ep1:x9p0aa…","value":true}
{"v":1,"id":"7f3a9c2e:9","hlc":"2026-09-19T11:08:12.555Z-0000-7f3a9c2e","t":"SettingChanged","key":"skip_forward_s","value":30}
{"v":1,"id":"7f3a9c2e:10","hlc":"2026-09-20T08:00:00.010Z-0000-7f3a9c2e","t":"FeedUrlChanged","podcast":"917393e3-…","feed_url":"https://newhost.example.com/show.xml","cause":"http-301"}
{"v":1,"id":"7f3a9c2e:11","hlc":"2026-09-21T09:12:00.000Z-0000-7f3a9c2e","t":"Unsubscribed","podcast":"917393e3-…"}
{"v":1,"id":"7f3a9c2e:12","hlc":"2026-09-21T09:13:00.000Z-0000-7f3a9c2e","t":"PodcastAliased","from":"a1c0…","to":"917393e3-…"}
```

Der Typkatalog für v1:
- `DeviceRegistered`
- `Subscribed`
- `Unsubscribed`
- `FeedUrlChanged`
- `PodcastAliased`
- `PodcastSettingChanged`
- `PlaybackUpdated` (deckt Position und gespielt/ungespielt ab)
- `FavouriteSet`
- `QueueItemSet`
- `QueueItemRemoved`
- `SettingChanged`
- später `Playlist*`

**Snapshot.** `snapshot-NNNNNN.json` enthält dieselbe Information in gefalteter Form. Pro Register steht der zuletzt von diesem Gerät geschriebene Wert samt Original-HLC und Event-ID:

```json
{"v":1,"device":"7f3a9c2e","covers_segments":[1,3],"max_seq":4711,
 "registers":{
   "sub/917393e3-…":        {"hlc":"2026-09-21T09:12:00.000Z-0000-7f3a9c2e","value":{"subscribed":false,"feed_url":"https://newhost.example.com/show.xml","title":"Example Show"}},
   "play/ep1:b3k7q2…":      {"hlc":"2026-09-19T11:05:48.870Z-0000-7f3a9c2e","value":{"position_ms":3541000,"duration_ms":3541000,"played":true}},
   "queue/ep1:x9p0aa…":     {"hlc":"…","value":{"in_queue":true,"sort_key":"a0V","podcast":"4be1…"}},
   "setting/skip_forward_s":{"hlc":"…","value":30}
 }}
```

**Kernidee.**
- Der Snapshot enthält nur die eigenen Schreibvorgänge dieses Geräts, nicht den globalen Zustand.
- Globaler Zustand = Join über alle `devices/*/snapshot-*` und alle Journalzeilen. Pro Register gewinnt der größte HLC.

Vier Folgen:
- Es braucht keine Version-Vectors und kein Pruning. Damit entfällt die Super-Productivity-Fehlerklasse.
- Ein Gerät löscht nur eigene Dateien, und nur Segmente, die der gerade atomar geschriebene eigene Snapshot abdeckt.
- Die Reihenfolge ist fest: neuen Snapshot schreiben, fsync, dann alte Segmente und den alten Snapshot löschen. Ein Absturz dazwischen hinterlässt nur Redundanz, die dank Idempotenz harmlos ist.
- Eine alte App-Version kann nichts zerstören, was eine neuere auf einem anderen Gerät geschrieben hat, weil sie fremde Dateien nie anfasst. Unbekannte Event-Typen und Felder fremder Geräte überliest sie einfach.

### 3.4 Stabile Identifikatoren

**Podcast-ID.**
- Hat der Feed ein `<podcast:guid>`, wird dieses verwendet.
- Andernfalls berechnet die App sie selbst nach exakt der Vorschrift der Podcast-Namespace-Spezifikation: UUIDv5 über die Feed-URL, "with the protocol scheme and trailing slashes stripped off", Namespace `ead4c236-bf58-58c6-a2c6-a6b28d128cb6`. https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/tags/guid.md
- So kommen zwei Offline-Geräte deterministisch auf dieselbe ID.
- Die ID bleibt danach stabil. Die Spezifikation sagt: "assigned … once in its lifetime". URL-Wechsel (301, `itunes:new-feed-url`) laufen über `FeedUrlChanged`.
- **Dubletten.** Zwei Geräte können denselben Podcast über unterschiedliche URLs abonnieren (www gegen non-www, Feedburner-Redirect), oder ein Feed bekommt nachträglich ein abweichendes `podcast:guid`. Ein Dedup-Schritt erkennt das am gleichen `podcast:guid` oder der gleichen normalisierten End-URL. Er schreibt `PodcastAliased{from,to}`. Als Ziel gewinnt deterministisch die lexikografisch kleinere ID bzw. bevorzugt die echte `podcast:guid`.
- **URL-Normalisierung für den Vergleich:** Schema weg, Host in Kleinbuchstaben, Default-Port weg, Trailing Slashes weg, Fragment weg. Query-Parameter bleiben unangetastet, da sie bei privaten oder Patreon-Feeds bedeutungstragend sind.

**Episoden-ID.** `ep1:` plus base32(sha256(podcast_id ‖ 0x00 ‖ kind ‖ 0x00 ‖ value)), auf 128 Bit gekürzt. Für `kind`/`value` gilt diese Rangfolge:
1. `guid` des Items, getrimmt. Er gilt nur innerhalb des Podcasts, denn GUIDs sind in der Praxis nicht global eindeutig, teils fehlend oder dupliziert. Siehe https://github.com/Podcastindex-org/podcast-namespace/issues/186 und https://theaudacitytopodcast.com/how-to-fix-common-podcast-rss-feed-problems-tap271/
2. die Enclosure-URL, ohne Schema;
3. `title` plus `pubDate`.

Enclosure-URLs sind als Primärschlüssel ungeeignet, weil Dynamic Ad Insertion und Tracking-Präfixe sie ändern. Deshalb hat der GUID Vorrang.

Zusätzlich führt jedes Event beim ersten Auftreten `ref:{guid, enclosure_url}` im Klartext mit. Das dient drei Zwecken:
- Menschenlesbarkeit;
- Re-Matching, falls ein Feed seine GUIDs ändert;
- die gpodder-Brücke, denn gpodder identifiziert Episoden über Feed-URL plus Media-URL. https://gpoddernet.readthedocs.io/en/latest/api/reference/events.html

Der Präfix `ep1:` versioniert das ID-Schema.

**Device-ID.** Eine zufällige UUID (im Ordnernamen gekürzt plus Klartextname). Sie wird lokal gespeichert, nie aus dem Hostnamen abgeleitet.

### 3.5 Merge-Semantik je Datentyp

Alles ist eine **LWW-Map: Register-Schlüssel → (HLC, Wert)**. Der Merge nimmt pro Schlüssel das Maximum nach HLC. Der HLC ist inklusive Device-ID total geordnet, also gibt es keine Gleichstände. Damit ist der Merge trivial kommutativ, assoziativ und idempotent.

**Abos: Register `sub/<podcast>`** mit `{subscribed: bool, feed_url, title}`.
- **OR-Set oder LWW?**
  - Ein OR-Set liefert "add wins" bei gleichzeitigem Unsubscribe und Re-Subscribe. Dafür braucht es Tag-Verwaltung und Tombstone-Mengen.
  - Bei einem einzelnen Nutzer heißt "gleichzeitig" praktisch "nacheinander auf verschiedenen Offline-Geräten". Dann entspricht LWW ("was ich zuletzt getan habe, gilt") der Nutzererwartung besser als add-wins.
  - **Empfehlung: LWW.**
- `Unsubscribed` ist ein Tombstone (`subscribed:false`) und wird nie gelöscht. Es sind wenige hundert Bytes pro Podcast. Tombstone-GC ist der klassische Weg, Gelöschtes wiederauferstehen zu lassen.
- `PodcastSettingChanged` läuft als eigene Register `subset/<podcast>/<key>`. So überschreibt eine Tempo-Änderung nicht das Abo-Flag.

**Wiedergabe: Register `play/<episode>`** mit `{position_ms, duration_ms, played}` als eine konsistente Einheit (KeePass-Argument).

Die Debatte "max position wins":

| Variante | Für | Gegen |
|---|---|---|
| Max gewinnt | robust gegen Uhrenfehler, verliert nie Fortschritt | bricht Zurückspulen, "nochmal von vorn hören" und "als ungespielt markieren"; letztere sind reale, häufige Aktionen |
| LWW nach HLC | bildet die Nutzerabsicht ab | anfällig für grobe Uhrenfehler |

**Empfehlung: LWW nach HLC**, flankiert von zwei Schutzmaßnahmen:
- (a) Drift-Klemmung beim HLC (siehe 3.7).
- (b) Beim Fortsetzen prüft die App, ob ein verdrängter Wert im Journal eines anderen Geräts mehr als 60 s weiter ist und jünger als z. B. 7 Tage. Dann bietet die UI unaufdringlich an: "Auf 'Mac mini' warst du bei 41:20 – dorthin springen?". Der Verlierer ist nicht weg, er steht noch im Journal.

`played:true` wird gesetzt bei "finished" oder bei Position ≥ Dauer − N s.

**Queue.**

| Option | Bewertung |
|---|---|
| LWW über die ganze Liste | am einfachsten, verliert aber gleichzeitige Adds zweier Geräte |
| echtes Listen-CRDT (RGA, Fugue, Loro MovableList) | korrekt, aber Overkill und mit Bibliotheks-Lock-in |
| **Empfehlung:** pro Eintrag ein LWW-Register `queue/<episode>` = `{in_queue, sort_key}` mit Fractional Index als String | siehe unten |

Details zur empfohlenen Variante:
- Die Reihenfolge ergibt sich aus der Sortierung nach `(sort_key, episode-id)`.
- Gleichzeitige Adds überleben beide.
- Ein Umsortieren ändert ein einziges Register.
- "Queue leeren" sind n `QueueItemRemoved`-Events.
- Die bekannten Schwächen von Fractional Indexing (Interleaving, wachsende Keys) sind bei einer Queue mit Dutzenden Einträgen irrelevant. Bei der Kompaktierung können die Keys re-balanciert werden, indem neue Register mit aktuellem HLC geschrieben werden.
- Die Semantik "gespielte Episode fliegt aus der Queue" ist lokal abgeleitet und erzeugt ein normales Removed-Event.

**Settings: `setting/<key>`**, LWW pro Schlüssel.
- Eine Whitelist legt fest, welche Settings synchronisiert werden: Skip-Intervalle, Standardtempo, Auto-Download-Regeln.
- Gerätelokal bleiben Pfade, Audio-Ausgabe, Theme und Keybindings.

**Favoriten: `fav/<episode>`**, LWW-bool.

**Playlists (später):**
- `pl/<id>/meta` als Metadaten-Register,
- `pl/<id>/item/<episode>` pro Eintrag, analog zur Queue.

### 3.6 Drosselung der Positions-Writes

- **Sofort schreiben bei:** Pause, Stop, Seek (entprellt auf 2 s), Episodenende, Episodenwechsel, App-Exit bzw. SIGTERM.
- **Während der Wiedergabe:** höchstens alle 60 s, konfigurierbar mit 30 s Minimum, und nur wenn sich die Position um mehr als 10 s geändert hat.
- **Größenordnung:** 1 h Hören ergibt ca. 60 Zeilen à ca. 200 Byte, also ca. 12 KB Journal. Die Kompaktierung faltet das auf ein Register.
- **Option, Standard aus:** Geräte im Akkubetrieb oder mit getakteter Verbindung schreiben nur bei Pause und Ende.
- Das lokale SQLite bekommt die Position weiterhin sekündlich. Nur der Library-Ordner wird gedrosselt.

### 3.7 Robustheit gegen Sync-Dienste

**Konfliktdateien.** Bei einem Schreiber pro Datei entstehen sie nur, wenn eine Device-ID geklont wurde: Home-Verzeichnis kopiert, VM-Snapshot oder Backup-Restore.
- Der Leser nimmt deshalb alles, was in `devices/<id>/` wie Journal oder Snapshot aussieht, inklusive der Muster `*conflicted copy*`, `*.sync-conflict-*` und `* 2.jsonl` (iCloud-Duplikate **[Muster aus Forenberichten, nicht vollständig verifiziert]**).
- Dedup über Event-`id` und LWW-Idempotenz machen das gefahrlos.
- **Klon-Erkennung:** Sieht ein Gerät in seinem eigenen Verzeichnis Events mit einer `seq`, die größer ist als sein lokal gemerkter Stand, oder findet es Konfliktkopien, erzeugt es eine neue Device-ID und einen Eintrag im lokalen Log. Die alten Dateien bleiben liegen. Die UI zeigt "verwaistes Gerät".

**Partielle oder veraltete Dateien.** Dropbox und Syncthing liefern ganze Dateien per Temp-Datei plus Rename aus. Syncthing nutzt `.syncthing.<name>.tmp` bzw. `~syncthing~…` (https://docs.syncthing.net/users/syncing.html). Eine Datei kann aber veraltet sein.
- Der Leser merkt sich pro fremdem Segment `(Größe, mtime, Offset, Hash des gelesenen Präfixes)`.
- Ist die Datei kleiner geworden oder das Präfix anders, liest er sie komplett neu. Das ist idempotent.
- Eine letzte Zeile ohne `\n` oder mit JSON-Fehler wird ignoriert und der Offset nicht vorgerückt.
- Eine kaputte Zeile mitten im Segment wird übersprungen und geloggt. Der Import bricht nie ab.

**Ignorieren:** Temp- und Metadateien sowie alles außerhalb des bekannten Schemas:
- `.syncthing.*`
- `~syncthing~*`
- `.stfolder`
- `.stversions`
- `.dropbox*`
- `desktop.ini`
- `.DS_Store`
- `*.icloud`
- `.tmp-*`

**iCloud Drive und OneDrive Files On-Demand.** Dateien können lokal "dataless" sein. Unter macOS zeigt das Flag `SF_DATALESS` in `st_flags` das an. Ein Lesezugriff löst dann den Download aus und kann blockieren oder fehlschlagen. Es gibt Berichte über wiederholte Eviction unter Speicherdruck. Quellen:
- https://eclecticlight.co/2023/11/21/icloud-drive-in-sonoma-fileprovider-and-eviction/
- https://mjtsai.com/blog/2023/10/27/icloud-drive-switches-to-dataless-files/
- https://dev.to/bokuwalily/icloud-silently-evicted-69-article-files-and-killed-4-days-of-publishing-edeadlk-and-a-1n6i

Konsequenzen für die App:
- Lesen läuft in einem Hintergrund-Thread mit Timeout und Retry. Die UI blockiert nie.
- Die Doku empfiehlt, den Ordner auf "Immer auf diesem Gerät behalten" zu stellen.
- Die wenigen kleinen Dateien machen das billig.

**Atomare Writes.**
- Snapshot, `device.json` und OPML werden als `.tmp-<rand>` im selben Verzeichnis geschrieben, dann `sync_all`, `rename`, fsync auf das Verzeichnis (Unix).
- Unter Windows kann `rename` scheitern, solange der Sync-Client die Zieldatei offen hält. Dann Retry mit Backoff.
- Journal-Append: eine komplette Zeile pro `write`. `sync_data` erfolgt bei Pause und Exit und höchstens alle paar Sekunden.
- Nach einem Crash schneidet das Gerät sein eigenes aktives Segment auf das letzte `\n` zurück.

**Uhren und HLC.**
- Lokale Regel: `hlc = max(now, last_local, max_seen_remote) + tick`.
- **Drift-Klemmung:** Liegt ein Remote-Event mehr als 24 h in der Zukunft, wird es angewendet. Die lokale Uhr wird dadurch aber nicht vorgestellt. Es gibt eine Warnung in der UI: "Gerät X hat eine falsch gehende Uhr".
- Ohne Klemmung würde ein Gerät mit der Jahreszahl 2036 alle anderen anstecken.
- Ein falsch gehendes Gerät gewinnt seine eigenen Register trotzdem bis zur Korrektur. Als Reparaturkommando dient `torrocast library doctor`: Es schreibt betroffene Register neu, mit einem HLC direkt über dem größten bisher gesehenen Wert. Da Register-Schlüssel nicht gelöscht werden können, muss der neue HLC größer sein – das ist die inhärente Grenze von LWW. Im Extremfall muss das defekte Geräteverzeichnis nach Rückfrage archiviert werden.
- James Longs Referenz-HLC verwirft Nachrichten ab einer festen Maximal-Drift. https://github.com/jlongster/crdt-example-app. Das ist für uns zu hart, weil Offline-Geräte legitimerweise alte Events liefern und nur Zukunftswerte verdächtig sind **[genauer Drift-Grenzwert dort nicht nachgeprüft]**.

**Ausgemusterte Geräte.** Ihre Verzeichnisse bleiben liegen, sie sind klein.
- Optional später: "Bibliothek konsolidieren". Ein Gerät übernimmt die Register des verwaisten Geräts mit Original-HLC in den eigenen Snapshot und löscht danach dessen Ordner.
- Das geschieht nur nach ausdrücklicher Bestätigung durch den Nutzer und ist die einzige Operation, die fremde Dateien anfasst.
- Taucht das alte Gerät wieder auf, schreibt es seine Dateien neu. Das ist idempotent und unschädlich.

**git-Repo als Ziel.**
- Weil kein Gerät fremde Dateien ändert, sind `git pull`/`merge` konfliktfrei (abgesehen von OPML in der Root-Variante).
- JSONL ergibt saubere Diffs.
- Die App ruft git in v1 nicht selbst auf.

### 3.8 Schema-Versionierung

**Additiv (Normalfall).**
- Neue Event-Typen, Felder und Register-Präfixe brauchen keinen Versionssprung.
- Leser ignorieren Unbekanntes.
- Weil niemand fremde Dateien umschreibt, geht Unbekanntes nie verloren. Das ist der große Vorteil gegenüber einer gemeinsamen Zustandsdatei.
- `v` pro Zeile erlaubt Änderungen an der Zeilenstruktur.

**Brechend.**
- `format_version` und `min_reader_version` in `format.json` werden erhöht. Das Vorbild ist Joplins `info.json`.
- Eine ältere App, die `min_reader_version > eigene` sieht, schaltet den Ordner-Sync ab und schreibt nichts mehr. Die lokale Wiedergabe funktioniert weiter. Die Meldung lautet "Bitte TorroCast aktualisieren".
- Bei `min_writer_version > eigene` liest sie nur noch.
- Die Migration ist nutzerinitiiert auf einem Gerät. Weil `format.json` dabei die einzige Multi-Writer-Datei ist, wird das neue Format in `v2/…` neben dem alten angelegt. Erst zum Schluss wird `format.json` umgestellt. Alte Daten bleiben als Backup liegen.
- `device.json` nennt pro Gerät die App- und Format-Version. Die UI kann dann warnen: "Mac mini läuft noch mit 0.3, Migration blockiert".

### 3.9 Verschlüsselung at rest – lohnt es sich?

**Dafür:** Hörgewohnheiten sind personenbezogen. Private Feed-URLs (Patreon, Supporter-Feeds) enthalten Tokens. Das ist das stärkste Argument.

**Dagegen:**
- Der Ordner liegt in der Cloud des Nutzers, in der ohnehin dessen übrige Dateien liegen.
- Verschlüsselung zerstört Menschenlesbarkeit, `git diff` und die Debugbarkeit.
- Passphrase oder Schlüssel müssen auf jedes Gerät. Das ist UX-Aufwand und bringt das Risiko eines totalen Datenverlusts.
- `age` ist ein Container-Format und nicht appendbar. Das Journal müsste auf unveränderliche Mini-Chunks oder Verschlüsselung pro Zeile umgestellt werden.

**Empfehlung:**
- v1 ohne Verschlüsselung.
- Das Format bleibt trotzdem vorbereitet: Das Feld `encryption: null` in `format.json` ist reserviert. Segmente werden als opake, versiegelte Einheiten behandelt, sodass später `journal-000004.jsonl.age` möglich ist. Das aktive Segment müsste dann in einem Chunk-Modus laufen.
- Als Sofortmaßnahme für Feed-URLs mit Credentials oder Token: Die UI warnt beim Abonnieren und bietet "nur auf diesem Gerät" an. Das Abo landet dann nicht im Library-Ordner, oder die URL wird dort durch einen Platzhalter ersetzt und das Token liegt im Keychain.
- Wer mehr will, nutzt Cryptomator, gocryptfs oder einen verschlüsselten Syncthing-Peer unterhalb der App.

### 3.10 Was NICHT in den Ordner gehört

- Audio-Downloads. Sie liegen in einem separaten, optional frei wählbaren Download-Ordner, der nicht mit dem Library-Ordner identisch sein sollte. Die App warnt, wenn er innerhalb des Library-Ordners liegt.
- Cover und Feed-XML.
- Die SQLite-DB und der Suchindex.
- Logs.
- Secrets: gpodder- und OPA-Zugangsdaten, API-Keys, HTTP-Auth für Feeds. Sie gehören in den OS-Keychain.
- Gerätelokale Settings.
- Lock-Dateien.
- Der HLC-Zustand und die Lese-Offsets.

### 3.11 Plattformpfade

| Zweck | Linux | macOS | Windows |
|---|---|---|---|
| Config (inkl. Pfad zum Library-Ordner, Device-ID) | `$XDG_CONFIG_HOME/torrocast` (`~/.config/…`) | `~/Library/Application Support/TorroCast` (für die TUI wahlweise XDG) | `%APPDATA%\TorroCast` |
| DB / Zustand | `$XDG_DATA_HOME/torrocast` bzw. `$XDG_STATE_HOME/torrocast` | `~/Library/Application Support/TorroCast` | `%LOCALAPPDATA%\TorroCast` |
| Cache (Feeds, Cover) | `$XDG_CACHE_HOME/torrocast` | `~/Library/Caches/TorroCast` | `%LOCALAPPDATA%\TorroCast\cache` |
| Downloads (Default) | `$XDG_DATA_HOME/torrocast/downloads` oder `~/Podcasts` | `~/Music/Podcasts` o. ä. | `%USERPROFILE%\Music\Podcasts` |
| Library-Ordner (Default, solange nichts gewählt ist) | `$XDG_DATA_HOME/torrocast/library` | `…/Application Support/TorroCast/library` | `%APPDATA%\TorroCast\library` |

Auch ohne Sync läuft die App über dasselbe Format. Es gibt einen Codepfad, keinen Sondermodus.

**Crates.**
- **`directories`** (`ProjectDirs`): auf Linux XDG, auf macOS alles unter `~/Library/Application Support` bzw. `Caches`, auf Windows `{RoamingAppData}\…\config|data` und `{LocalAppData}\…\cache|data`. `state_dir` existiert nur unter Linux. https://docs.rs/directories/latest/directories/struct.ProjectDirs.html (laut docs.rs aktuell 6.0.0)
- **`etcetera`** bietet wählbare Strategien (Xdg, Apple, Windows, Unix). Es empfiehlt für CLI-Tools mit editierbarer Konfiguration ausdrücklich XDG auch auf macOS und `choose_native_strategy()` für GUIs. https://docs.rs/etcetera/latest/etcetera/ (laut docs.rs 0.11.0)

Für ein TUI plus spätere native GUI ist `etcetera` flexibler.

Hinweis aus dem Schwesterprojekt: TorroMail löst Pfade in `crates/torromail-control/src/paths.rs` bewusst ohne Crate, als reine Funktion `(Platform, HOME, XDG_*) → PathBuf`. Das ist trivial testbar.
- Dasselbe Muster empfiehlt sich für TorroCast.
- Ein Crate käme höchstens für die Windows Known Folders hinzu.
- Wichtig: Native GUI und TUI müssen auf macOS **dasselbe** Verzeichnis nutzen. Die Entscheidung XDG gegen Application Support wird einmal getroffen und fixiert.

Alle Pfade sind überschreibbar: `TORROCAST_LIBRARY_DIR`, `--library`, Config-Datei.

### 3.12 File-Watching

- Geeignet ist das `notify`-Crate (docs.rs: 8.2.0) plus Debouncer (`notify-debouncer-full`). Es beobachtet `devices/` rekursiv mit 1–2 s Entprellung. Eigene Schreibvorgänge werden über den Pfadpräfix des eigenen Device-Verzeichnisses ignoriert.
- Dokumentierte Fallstricke: "Network mounted filesystems like NFS may not emit any events". Dazu kommen inotify-Watch-Limits und uneinheitliche Event-Muster (Truncate gegen Replace). `PollWatcher` dient als Fallback. https://docs.rs/notify/latest/notify/
- **Events sind nur ein Beschleuniger, nie die Wahrheit.** Zusätzlich gibt es immer einen billigen Rescan (nur `readdir` und `stat`, Vergleich mit gemerkten Größen und mtimes):
  - beim Start,
  - alle 2–5 min,
  - bei Fokus bzw. Resume,
  - vor dem Beenden.
- Bei NAS-, SMB-, NFS- oder FUSE-Mounts (rclone, Nextcloud-VFS) läuft das Watching automatisch oder per Setting `watch = "poll"`.
- mtime ist auf Netzwerk-Mounts und nach einem Sync unzuverlässig. Entscheidend sind deshalb Größe und Präfix-Hash. mtime ist nur ein Hinweis.
- Ein eingehendes Remote-Event aktualisiert SQLite und damit die UI. Läuft gerade dieselbe Episode, wird die lokale Position nicht überschrieben, weil das aktiv spielende Gerät ohnehin gleich den neueren HLC schreibt.

### 3.13 Koexistenz mit gpodder.net und Open Podcast API

Der Core kennt nur Events und den gemergten Zustand. Die Backends sitzen hinter einem Trait:

```rust
pub trait SyncBackend: Send {
    fn id(&self) -> BackendId;
    fn capabilities(&self) -> Capabilities;          // subscriptions, positions, queue, settings, favourites …
    /// Lokale, noch nicht übertragene Events hinausschieben.
    fn push(&mut self, events: &[Event]) -> Result<PushReceipt, SyncError>;
    /// Fremde Änderungen seit `cursor` holen; Backend übersetzt in Events mit Herkunfts-Tag.
    fn pull(&mut self, cursor: &Cursor) -> Result<Pulled, SyncError>;   // Pulled { events, next_cursor }
}
```

**`FolderBackend`.**
- `push` ist der Journal-Append.
- `pull` ist der Rescan.
- Der Cursor ist die Offset-Tabelle.
- Alle Capabilities sind verfügbar.

**`GpodderBackend`.**
- Abos als add/remove nach Feed-URL.
- Episode Actions `play` mit `started/position/total` in Sekunden, `timestamp` und `since`-Cursor.
- Identifikation über Feed-URL plus Media-URL. Deshalb tragen unsere Events `feed_url` und `ref.enclosure_url`. https://gpoddernet.readthedocs.io/en/latest/api/reference/events.html
- Keine Queue und keine Favoriten: `capabilities()` meldet das, die UI zeigt es an.
- Remote-Actions bekommen beim Import einen HLC aus dem Server-`timestamp`, mit Zähler 0 und der Pseudo-Device-ID `gpodder:<device>`. gpodder.net ist laut AntennaPod-Doku häufig überlastet. https://antennapod.org/documentation/general/synchronization

**`OpenPodcastApiBackend`.**
- Subscriptions mit `guid` plus `feed_url`, Tombstoning, `subscription_changed`/`guid_changed`/`deleted`-Zeitstempel und `since`-Abfragen. https://openpodcastapi.org/specs/subscriptions/
- Das Datenmodell passt fast 1:1: UUID/`podcast:guid`-basierte IDs, Tombstones.
- Die Spezifikationen gelten aber insgesamt noch als "in progress", Breaking Changes sind möglich. https://openpodcastapi.org/specs/
- Also später implementieren, das Interface aber jetzt daran ausrichten.

**Regel für v1:** Es ist genau ein aktives Backend zur Zeit. Ordner und Server parallel zu betreiben ergibt Echo-Schleifen: Gerät A lädt zu gpodder hoch, Gerät B importiert das als "neues" Event und schreibt es in den Ordner. Das ist lösbar mit Herkunfts-Tag und Dedup über `(episode, position, timestamp)`, gehört aber in eine spätere Ausbaustufe.

OPML-Import und -Export bleiben backendunabhängig.

---

## 4. Teststrategie für die Merge-Logik

Das Herzstück ist eine reine Funktion ohne I/O: `State::merge(&State) -> State` und `State::apply(&Event)`. Alles Dateibezogene liegt hinter einem kleinen `LibraryFs`-Trait, mit echtem FS und einem In-Memory-Fake.

**1. Algebraische Properties mit `proptest`** über generierte Event-Mengen. Die Generatoren arbeiten mit wenigen Podcast-, Episoden- und Device-IDs, um Kollisionen zu erzwingen, und mit absichtlich verschobenen Uhren (± Tage) und gleichen Millisekunden.
- Kommutativ: `merge(a,b) == merge(b,a)`.
- Assoziativ: `merge(merge(a,b),c) == merge(a,merge(b,c))`.
- Idempotent: `merge(a,a) == a`. Ein doppelt angewendetes Event ändert nichts.
- Reihenfolgeunabhängigkeit: Für jede Permutation π der Events gilt `fold(events) == fold(π(events))`.
- Monotonie: `merge(a,b) ≥ a` in der Halbordnung. Kein Register bekommt je einen kleineren HLC.

**2. Äquivalenz der Kompaktierung.**
- `snapshot(fold(J[..k])) ⊔ fold(J[k..]) == fold(J)` für jeden Schnittpunkt k.
- Snapshot-Roundtrip: serialisieren, deserialisieren, identischer Zustand.

**3. HLC-Properties.**
- Strikt monoton pro Gerät, auch wenn die Wanduhr rückwärts springt.
- Die String-Ordnung entspricht der Tupel-Ordnung.
- Die Drift-Klemmung verhindert eine "Ansteckung".

**4. Simulierte Multi-Device-Szenarien.** Am besten als State-Machine-Test, z. B. mit `proptest-state-machine` **[Version nicht geprüft]**.
- Modell: N Geräte mit je eigenem FS-Abbild plus ein "Sync-Netz", das beliebige Teilmengen von Dateien in beliebiger Reihenfolge zustellt.
- Das Netz kann außerdem veraltete Versionen liefern, Dateien doppelt als Konfliktkopie anlegen und abgeschnittene letzte Zeilen erzeugen.
- Aktionen: Nutzeraktion auf Gerät i, Teil-Sync i→j, Kompaktierung auf i, Crash zwischen Snapshot-Write und Segment-Löschung, Klon einer Device-ID, Uhrensprung.
- Invarianten:
  - (a) **Strong Eventual Consistency:** Nach vollständiger Zustellung haben alle Geräte identischen Zustand.
  - (b) Kein Gerät löscht je fremde Dateien.
  - (c) Kein bestätigtes Event geht verloren. Jedes Register hat mindestens den HLC des letzten darauf geschriebenen Events.
  - (d) Ein Unsubscribe ersteht nicht wieder auf.

**5. Szenario-Tests als Beispiele** (lesbare Regressionstests):
- Offline-Flug: Laptop hört weiter, Desktop markiert als gespielt.
- Re-Subscribe nach Unsubscribe.
- Gleichzeitiges Queue-Add.
- Feed-URL-Wechsel auf einem Gerät während Wiedergabe auf dem anderen.
- Alias-Merge zweier Podcast-IDs.

**6. Vorwärtskompatibilität.**
- Golden Files eines "zukünftigen" Formats mit unbekannten Event-Typen und Feldern: Die alte Codebasis liest ohne Fehler, und nach ihrer Kompaktierung sind die fremden Dateien byte-identisch.
- Ein `min_reader_version`-Gate-Test.

**7. Parser-Robustheit.**
- `cargo-fuzz` bzw. proptest mit Zufallsbytes auf dem JSONL-Leser: nie Panic, nie Abbruch des Gesamtimports.
- Dazu BOM, CRLF (von Windows-Editoren oder git autocrlf) und sehr lange Zeilen.

**8. Dateisystem-Tests.**
- Atomarer Write mit injizierten Fehlern (ENOSPC, Rename-Fehler, Crash).
- Reparatur einer unvollständigen letzten Zeile beim Start.
- Ignorierlisten für Konflikt- und Temp-Namen, als Tabellentest mit den echten Mustern von Dropbox und Syncthing.

**9. Optionaler End-to-End-Test** (nightly, nicht in jeder CI):
- zwei Syncthing-Instanzen in Containern, zwei TorroCast-Cores, Zufallsaktionen, Konvergenzprüfung.
- Für macOS und iCloud sind nur manuelle Tests realistisch.

---

## 5. Offene Entscheidungen für den Product Owner

1. **OPML im Library-Ordner:** pro Gerät unter `exports/` (konfliktfrei, das ist die Empfehlung) oder eine Datei in der Wurzel (hübscher, aber eine Multi-Writer-Datei).
2. **Position:** LWW plus "Sprung anbieten" (Empfehlung) oder max-wins.
3. **Private Feeds mit Token in der URL:** unverschlüsselt in den Ordner schreiben, "nur lokal" halten oder Platzhalter plus Keychain.
4. **macOS-Pfade für die TUI:** XDG oder Application Support. Native GUI und TUI müssen identisch sein.
5. **Verschlüsselung:** v1 ohne (Empfehlung), mit im Format reserviertem Platz.

## 6. Risiken und bewusst akzeptierte Grenzen

- LWW verwirft bei echter Gleichzeitigkeit eine Seite. Das ist für diesen Datentyp akzeptabel, und die Verlierer bleiben bis zur Kompaktierung im Journal nachvollziehbar.
- Eine grob falsche Geräteuhr ist durch HLC nur gemildert, nicht gelöst. Darum gibt es Drift-Warnung und das `doctor`-Kommando.
- Tombstones und verwaiste Geräteverzeichnisse wachsen unbegrenzt, bleiben aber winzig (KB-Bereich pro Jahr).
- Sync-Latenz ist Sache des Dienstes. Ein "Handoff in Sekunden" wie bei Pocket Casts ist mit Dropbox oder iCloud nicht garantierbar.
- iOS und Android (später?): Sandboxing macht frei wählbare Ordner umständlich. Das Format selbst ist dafür unkritisch.

## 7. Empfohlene Umsetzungsreihenfolge

1. Crate `torrocast-library` (reiner Rust-Code ohne I/O-Abhängigkeiten: Event-Typen, HLC, LWW-Map, Merge, Snapshot) plus die Property-Tests aus Abschnitt 4.
2. `LibraryFs`, atomare Writes, JSONL-Reader und -Writer, Segmentrotation, Kompaktierung.
3. Anbindung an die lokale SQLite (materialisierte Sicht mit HLC-Spalten).
4. Watcher und Rescan.
5. `SyncBackend`-Trait extrahieren. Als zweites Backend gpodder, erst danach OPA.

## 8. Nicht oder nur teilweise verifiziert

- Joplin "ein Item pro Datei" sowie die Ordner `.resource`, `.sync` und `temp` auf dem Target: aus Vorwissen. Die abgerufene Spezifikationsseite bestätigte nur `info.json` und das Lock-Schema.
- KeePass "deleted objects"-Liste: aus Vorwissen. Die abgerufene Seite bestätigte den Entry-Level-Merge per Modifikationszeit und die History.
- Actual Budget nutzt HLC und CRDT-Nachrichten: belegt über James Longs Vortrag und Beispielcode, nicht über die offizielle Actual-Doku.
- Release-Daten der Crates: Die von docs.rs abgeleiteten Versionsnummern (notify 8.2.0, directories 6.0.0, etcetera 0.11.0) stammen aus der heutigen Abfrage. Die Release-Datumsangaben des Abruf-Tools erschienen mir unzuverlässig und wurden deshalb weggelassen. Vor der Übernahme bitte auf crates.io prüfen. Für `keyring` und `proptest-state-machine` gilt dasselbe.
- cr-sqlite-Wartungsstand: nur indirekt geprüft (letztes npm-Release Januar 2024).
- Delta- und Block-Sync-Verhalten von Dropbox, iCloud und OneDrive bei Appends: nicht geprüft. Das Design hängt dank kleiner Segmente nicht davon ab.
- iCloud-Duplikat-Namensmuster (`name 2.ext`): aus Forenberichten, nicht aus Apple-Doku.
- Genauer Max-Drift-Wert in James Longs HLC: nicht nachgeprüft.
- Standard Notes, Enpass, Anytype, Organice, Buku, Obsidian-Plugins, gPodder-Desktop, newsboat und Pocket Casts: nicht im Detail recherchiert. Die Aussagen dazu sind allgemeines Vorwissen.
- Open Podcast API: Die Subscriptions-Seite nennt den Endpoint "core", die Übersichtsseite führt alle Spezifikationen als "in progress". Die Generierung der GUID bei fehlendem `podcast:guid` war auf der abgerufenen Seite nicht beschrieben.

