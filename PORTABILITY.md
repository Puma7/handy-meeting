# Plattformstand und Testpakete

Stand: 26.10.2026. Rust/Tauri 1.0.0 wird mit Windows-Setup/ZIP, Ubuntu-/Debian-DEB sowie getrennten Apple-Silicon-/Intel-DMGs vorbereitet. [Aktueller Buildstand](BUILD_STATUS.md), [Paketierung](docs/RUST_PORTABLE.md), [Veröffentlichung](docs/RELEASING.md). Erfolgreiche Paketbauten und echte Geräte-/Langzeitprüfungen bleiben getrennte Nachweise. Keine Installation beendet fremde Handy-Prozesse oder laufende Nutzeraufnahmen.

## Historische Vorschauen

**Rust/Tauri 1.0.0-beta.2 ist eine zusätzliche Migrationsbeta.** Linux-/Mac-Aufnahmeadapter und native DEB-/DMG-Builds sind angelegt; erreichte CI-Ergebnisse sind im [Prüfbericht](TEST_REPORT.md) getrennt von echten Geräte- und Langzeitnachweisen dokumentiert. Zielversionen: Ubuntu 22.04/24.04, Debian 12/13 x64 und macOS 13+ auf Apple Silicon/Intel. [Details und Grenzen](docs/RUST_PORTABLE.md). Die nachfolgenden älteren Download-/Hardwareangaben gelten für WPF/Avalonia 0.4.1 beziehungsweise 0.5.0-preview.1.

| Ziel | Stand | Nächste Freigabeprüfung |
| --- | --- | --- |
| Windows x64 | Bestehende WPF-App; 0.4.1 mit verständlicherem Editor, Logo und lokaler Agentenschnittstelle | `verify.ps1` und gezielte native Regression |
| Ubuntu/Debian x64 | Selbstständiger Avalonia-Build und `.deb` als technische Vorschau | Echte Desktop-, PulseAudio-/PipeWire-, Gerätewechsel- und Langzeittests auf beiden Distributionen |
| Mac Apple Silicon | .NET-Programm, Swift-Helfer und DMG auf einem echten Mac-Runner gebaut; Signaturen und synthetische Kernprüfungen bestanden | Berechtigungen, echte Audiogeräte, native GUI und lange Sessions auf Hardware testen |
| Mac Intel | Dasselbe auf einem Intel-Mac-Runner bestanden | Dieselben Hardwareprüfungen auf Intel-Mac |

**Mac-DMGs wurden inzwischen auf GitHub gebaut und geprüft.** Der erfolgreiche Workflow lief auf echten Mac-Runnern beider Architekturen. Das belegt den Swift-Build, Paket-/Signaturprüfungen und synthetische Kernprüfungen, aber keine funktionierende Aufnahme realer Meetings. Der aktuelle Prüfumfang steht in [BUILD_STATUS.md](BUILD_STATUS.md).

Die [portable Vorschau 0.5.0-preview.1](https://github.com/Puma7/handy-meeting/releases/tag/v0.5.0-preview.1) enthält jetzt tatsächlich veröffentlichte DMGs für Apple Silicon/Intel und das Ubuntu-/Debian-DEB, ergänzt um `SHA256SUMS.txt`. Diese Dateien stammen aus dem erfolgreichen Build von Release-Commit `cbc0f1e358b51ee3c568ad3f319d8fc02281ca07`. Die Veröffentlichung ersetzt die offenen Hardwareprüfungen nicht.

## Architektur und Größe

`HandyMeeting.Core` verwendet dieselben Quelldateien für PCM-Sicherung, Transkriptjournal, Streaming, Quellenprüfung und Nachbearbeitung wie die Windows-App. `HandyMeeting.Portable` ergänzt eine Avalonia-Oberfläche; die bestehende Windows-App wird während der Portierung erhalten. Es wird keine zweite Erkennungsengine entwickelt.

Handy wird über typische Installationsorte beziehungsweise `PATH` erkannt. Die App fragt seine installierten Streaming-Modelle ab und liest vorhandene Modellgewichte. Native Bibliotheken werden passend zum Betriebssystem mitgeliefert, über festgelegte SHA-256-Werte geprüft und aus dem offiziellen transcribe.cpp-Release 0.4.1 bezogen. Ein gemeinsam mit Handy geladener Erkennungsprozess wird noch nicht verwendet. Das Textmodell läuft weiterhin separat in einem lokalen Dienst.

Die Vorschau enthält die .NET-Laufzeit, damit Tester kein SDK installieren müssen. Das erhöht die Programmgröße gegenüber dem Windows-Ordner ohne Runtime. Große Sprach- oder Textmodellgewichte gehören nicht zum Paket. API-Schlüssel werden in der portablen Vorschau nur für die Sitzung im Arbeitsspeicher gehalten; es gibt keinen Klartext-Fallback. Native Schlüsselbundintegration steht noch aus.

## Linux bauen und testen

```powershell
./scripts/build-portable.ps1 -Runtime linux-x64
```

Anschließend auf Linux:

```sh
bash scripts/package-linux.sh
```

Das `.deb` liegt unter `packages/HandyMeeting-0.5.0-preview.1-linux-amd64.deb`. Es verlangt die üblichen Grafik-/Sprachbibliotheken und `pulseaudio-utils`. Der bereits laufende lokale Audiodienst muss PulseAudio-kompatibel sein, etwa PipeWire-Pulse. Die App startet oder ersetzt keinen System-Audiodienst. Systemausgabe und Mikrofon werden getrennt über `parec` gelesen; der Ausgabemonitor wird aus dem ausgewählten Sink abgeleitet. Genaue Gerätezeitmarken liegen auf diesem Pfad noch nicht vor; Zeitangaben werden als angenähert markiert.

Die vorhandene Ubuntu-26.04-WSL-Umgebung ist minimal und besitzt kein ICU-Paket. Für die dortigen synthetischen Kernprüfungen wurde ausschließlich im Testprozess `DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1` gesetzt. Das Installationspaket verlangt dagegen ICU. Weder Desktopdarstellung noch tatsächliche PulseAudio-/PipeWire-Geräteaufnahme wurden in dieser Umgebung geprüft. Eine gebaute `.deb` ist noch keine Freigabe für wichtige echte Meetings.

Der synthetische Abbruchtest verwendet eigene Fake-Pulse-Programme in seinem Testordner. Er führt den tatsächlichen Prozesslese- und PCM-Sicherungspfad mit zwei verschiedenen Signalen aus, beendet den eigenen Aufnahmehost hart und prüft anschließend bestätigte Audiopräfixe per SHA-256 sowie beide Textjournalstände. Es werden keine echten Geräte geöffnet. Der Test dauert nur wenige Sekunden und ist kein weiterer Zweistundentest. Unbestätigter Audioschluss kann bei einem Prozessabbruch fehlen; Stromausfall wurde nicht geprüft.

## Mac bauen

Auf einem Mac mit .NET SDK 10, Python 3 und Xcode Command Line Tools:

```sh
bash scripts/package-macos.sh arm64
bash scripts/package-macos.sh x64
```

Das Skript erstellt die App-Struktur, kompiliert den ScreenCaptureKit-/AVAudioEngine-Helfer, prüft die Struktur, signiert Testpakete ad hoc und erzeugt/prüft die DMG mit `hdiutil`. Das Paket verlangt macOS 15 oder neuer; ältere Zielangaben des frühen Helferentwurfs waren keine Runtime-Freigabe. Die Mindestversion richtet sich nach den [aktuell unterstützten .NET-10-Plattformen](https://github.com/dotnet/core/blob/main/release-notes/10.0/supported-os.md). Für einfache öffentliche Installation fehlen weiterhin ein geprüfter Developer-ID-/Hardened-Runtime-/Notarisierungsablauf und reale Tests. Ad-hoc-Signierung ist keine Apple-Notarisierung. Keine Anleitung verlangt das pauschale Abschalten von Gatekeeper.

## Daten und Upgrade

Portable Sitzungen liegen im benutzerspezifischen lokalen Anwendungsdatenordner `HandyMeeting`, außerhalb einer installierten signierten App. Die Windows-WPF-App verwendet weiterhin die vorhandenen Projektordner `Sessions` und `data`. Bestehende Dateien werden nicht automatisch verschoben oder gelöscht. Die portable Vorschau erreicht noch nicht alle Komfortfunktionen der Windows-Oberfläche; insbesondere Geräte- und Fehlerzustände, Lokalisierung, Vorlagenverwaltung und Schlüsselbundanbindung benötigen weitere Angleichung und Tests.
