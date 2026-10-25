# Plattformstand und Testpakete

Stand: 25.10.2026. Windows und portable Vorschau werden getrennt bereitgestellt. Keine Installation beendet fremde Handy-Prozesse oder laufende Nutzeraufnahmen.

| Ziel | Stand | Nächste Freigabeprüfung |
| --- | --- | --- |
| Windows x64 | Bestehende WPF-App; 0.4.1 mit verständlicherem Editor, Logo und lokaler Agentenschnittstelle | `verify.ps1` und gezielte native Regression |
| Ubuntu/Debian x64 | Selbstständiger Avalonia-Build und `.deb` als technische Vorschau | Echte Desktop-, PulseAudio-/PipeWire-, Gerätewechsel- und Langzeittests auf beiden Distributionen |
| Mac Apple Silicon | .NET-Programm und passende Metal-Bibliotheken von Windows aus kompiliert; nativer Aufnahmehelfer als Swift-Quelle vorhanden | Helfer auf einem echten Mac bauen, Berechtigungen und Audio testen, DMG prüfen |
| Mac Intel | .NET-Programm und CPU-Bibliotheken von Windows aus kompiliert | Dieselben Prüfungen auf Intel-Mac |

**Es gibt noch keine auf macOS gebaute und geprüfte DMG.** Die Skripte dafür sind vorbereitet. Ein plattformübergreifender .NET-Build allein belegt weder einen erfolgreichen Swift-Build noch funktionierende macOS-Aufnahme. Hier steht kein Mac-Buildhost zur Verfügung. Der manuell gestartete Workflow `.github/workflows/portable.yml` sieht echte Mac-Runner beider Architekturen vor; er wurde noch nicht auf GitHub ausgeführt.

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

Das Skript erstellt die App-Struktur, kompiliert den ScreenCaptureKit-/AVAudioEngine-Helfer, prüft die Struktur, signiert Testpakete ad hoc und erzeugt/prüft die DMG mit `hdiutil`. Ziel ist macOS 13 oder neuer; die tatsächliche Plattformfreigabe steht aus. Für einfache öffentliche Installation fehlen zusätzlich ein geprüfter Developer-ID-/Hardened-Runtime-/Notarisierungsablauf und reale Tests. Ad-hoc-Signierung ist keine Apple-Notarisierung. Keine Anleitung verlangt das pauschale Abschalten von Gatekeeper.

## Daten und Upgrade

Portable Sitzungen liegen im benutzerspezifischen lokalen Anwendungsdatenordner `HandyMeeting`, außerhalb einer installierten signierten App. Die Windows-WPF-App verwendet weiterhin die vorhandenen Projektordner `Sessions` und `data`. Bestehende Dateien werden nicht automatisch verschoben oder gelöscht. Die portable Vorschau erreicht noch nicht alle Komfortfunktionen der Windows-Oberfläche; insbesondere Geräte- und Fehlerzustände, Lokalisierung, Vorlagenverwaltung und Schlüsselbundanbindung benötigen weitere Angleichung und Tests.
