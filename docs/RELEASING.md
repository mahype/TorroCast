# Ein Release ausliefern

Ein Tag der Form `v0.1.0` baut TorroCast für Linux, macOS und Windows und hängt die drei
Archive samt `SHA256SUMS` an ein GitHub-Release. Das macht `.github/workflows/release.yml`.

## Kurzfassung

```
# 1. Version in Cargo.toml ([workspace.package]) setzen, Cargo.lock folgt mit:
cargo build
# 2. docs/release-notes/vX.Y.Z.md schreiben — das wird der Text des Releases
git commit -am "release: vX.Y.Z"
git tag vX.Y.Z
git push origin main vX.Y.Z
```

Nach etwa einer Viertelstunde steht das Release unter
<https://github.com/mahype/TorroCast/releases>. Stimmen Tag und Version in `Cargo.toml` nicht
überein, bricht der Lauf ab, bevor etwas gebaut wird. Fehlt die Datei mit den Release-Notizen,
scheitert erst der letzte Schritt.

## Probelauf

Unter *Actions → Release → Run workflow* baut derselbe Ablauf die drei Archive, ohne etwas zu
veröffentlichen; sie hängen dann als Artefakte am Lauf. Von der Kommandozeile:

```
gh workflow run release.yml
gh run watch
gh run download <id>
```

## Was gebaut wird

| Datei | Gebaut auf | Bemerkung |
| --- | --- | --- |
| `torrocast-X-linux-x86_64.tar.gz` | Ubuntu 22.04 | alte glibc (2.35), läuft damit auch auf neueren Systemen; braucht `libasound.so.2` |
| `torrocast-X-macos-universal.tar.gz` | macOS 14 | arm64 und x86_64 in einer Datei (`lipo`) |
| `torrocast-X-windows-x86_64.zip` | Windows | C-Laufzeit eingebunden, braucht kein VC++-Paket |

Alle drei enthalten Opus: libopus wird mitgebaut (`--features opus-bundled`). Jeder Lauf startet
die fertige Datei einmal mit `--version` und vergleicht die Antwort mit der erwarteten Version.

## macOS: signieren und notarisieren

Ohne Apple-Secrets wird die Datei nur ad hoc signiert. Sie läuft, aber Gatekeeper hält einen
Browser-Download zurück: „Apple konnte nicht überprüfen, ob ‚torrocast‘ frei von Schadsoftware
ist“ – mit den Knöpfen *In den Papierkorb legen* und *Fertig*, keinem zum Öffnen. Drei Wege daran
vorbei, solange nicht notarisiert wird:

```
# 1. Die Sperre von der entpackten Datei nehmen
xattr -d com.apple.quarantine ./torrocast

# 2. Gar nicht erst mit dem Browser laden – gh und curl setzen die Sperre nicht
gh release download v0.1.0 -R mahype/TorroCast -p '*macos*' && tar -xzf torrocast-*-macos-universal.tar.gz
```

3\. Nach dem ersten, abgewiesenen Start: *Systemeinstellungen → Datenschutz & Sicherheit*, ganz
unten „torrocast wurde blockiert“ → *Dennoch öffnen*.

### Dauerhaft: die fünf Secrets

Sind im Repository dieselben Secrets hinterlegt wie bei TorroMail, signiert der Ablauf mit der
Developer ID, lässt die Datei notarisieren und bricht ab, wenn Apple nicht „Accepted“ sagt. Am
Workflow ändert sich dafür nichts. GitHub gibt Secrets nicht wieder heraus – sie lassen sich also
nicht von TorroMail herüberkopieren, sondern müssen noch einmal gesetzt werden, am einfachsten
auf dem Mac, in dessen Schlüsselbund das Zertifikat liegt:

```
# Schlüsselbundverwaltung → „Developer ID Application: …“ → Exportieren als cert.p12 (Passwort vergeben)
base64 -i cert.p12 | gh secret set MACOS_CERTIFICATE_P12 -R mahype/TorroCast
gh secret set MACOS_CERTIFICATE_PASSWORD  -R mahype/TorroCast   # fragt nach dem Wert
gh secret set APPLE_ID                    -R mahype/TorroCast   # die Apple-ID-Mailadresse
gh secret set APPLE_TEAM_ID               -R mahype/TorroCast   # 10 Zeichen, developer.apple.com → Membership
gh secret set APPLE_APP_SPECIFIC_PASSWORD -R mahype/TorroCast   # account.apple.com → App-spezifische Passwörter
rm cert.p12
```

Danach zeigt ein Probelauf (`gh workflow run release.yml`), ob Signatur und Notarisierung
durchgehen, ohne dass etwas veröffentlicht wird. Eine einzelne Programmdatei lässt sich nicht
„stapeln“: Der Mac fragt das Ergebnis der Notarisierung beim ersten Start online ab.

## Solange das Repository privat ist

Releases eines privaten Repositorys sieht und lädt nur, wer Zugriff hat — angemeldet im Browser
oder mit `gh release download v0.1.0 -R mahype/TorroCast`.
