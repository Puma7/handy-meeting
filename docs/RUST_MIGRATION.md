# Handy-nahe Migration zu Rust und Tauri

Zielversion: **1.0.0-beta.1**, Stand 25.10.2026. Die erste Migrationsbeta zielt auf Windows. Sie ist kein Release Candidate. Die bisherigen Prototypen, Aufnahmen und Auswertungen bleiben erhalten.

## Vorhandene Handy-Bausteine verwenden

Die installierte Handy-App bleibt die Quelle für Modellkennungen, Modelllisten, Downloads und gemeldete Streamingfähigkeiten. Es gibt keinen zweiten Modellkatalog und keinen eigenen Gewichte-Downloader. Die Erkennung verwendet die offiziellen sicheren Rust-Bindings von transcribe.cpp 0.4.1. Eine eigene Erkennungsengine oder eigene FFI-Strukturdeklarationen werden nicht entwickelt.

Tauri 2.11.5, React 18.3.1, die Farbpalette und Handys festgelegter Tao-Patch folgen dem geprüften Handy-Stand `73c095857bc4eadff097f64767c40ad129cc372d`. Attribution und Lizenztexte werden mitgeliefert. Kompatible Sicherheitskorrekturen in Unterabhängigkeiten werden gesondert geprüft und gesperrt.

Meeting-Aufnahme, getrennte Spuren, Sitzungsjournale und belegte Nachbearbeitung bleiben Aufgaben der Companion App. Vorhandene Bibliotheken wie WASAPI und Hound übernehmen die zugrunde liegenden Audiofunktionen. Handy wird weder verändert noch eingebettet. Eine gemeinsame geladene Modellinstanz oder vom Maintainer zugesicherte Companion-API wird nicht vorausgesetzt.

## Verhalten bei Handy-Updates

Die Modellliste wird beim Start, beim erneuten Fokussieren mit begrenzter Abfragerate, bei ausdrücklicher Aktualisierung und vor einer neuen Aufnahme aus Handy gelesen. Neue Kennungen und zusätzliche Metadaten werden übernommen. Eine entfernte oder inkompatible bereits gewählte Kennung verlangt eine ausdrückliche neue Auswahl. Es gibt keinen stillen Modellwechsel.

Eine neue Modellarchitektur kann eine geprüfte Aktualisierung der Erkennungslaufzeit erfordern. Neue Listeneinträge allein beweisen keine Ausführbarkeit. Jeder Modell-Snapshot hält tatsächlichen Pfad, Revision, Bytezahl, SHA-256 und Runtime-Identität fest. Wiederaufnahme und jeder Ersatzprozess prüfen diese Identität. Ein Handy-Update kann neue Modelle für neue Sitzungen bereitstellen, ohne alte Textstände unbemerkt mit anderen Gewichten zu mischen.

## Aufbau

| Bereich | Verantwortung |
| --- | --- |
| `crates/handy-bridge` | Nur lesende Handy-Abfrage, Gewichtauflösung, Identitätsprüfung, offizielle ASR-Bindings |
| `crates/meeting-core` | Audiosicherung, bestätigte Schreibpunkte, Journale, Wiederherstellung, Windows-Geräteadapter |
| `crates/meeting-notes` | Getrennte Module für Datenmodell, Validierung, Texttransport und Analyseablauf |
| `apps/desktop` | Gemeinsame React-Oberfläche im Handy-Stil |
| `apps/desktop/src-tauri` | Ablaufsteuerung, begrenzte lokale Befehle und isolierte Erkennungsprozesse |

Die WebView erhält keine allgemeine Shell-, Dateisystem- oder Netzwerkfreigabe. Audio gelangt nicht in die Oberfläche oder den Textmodell-Transport. Cloud-Nachbearbeitung verlangt Textfreigabe je Lauf; Schlüssel bleiben im Arbeitsspeicher. Ergebnisse sind überprüfbare KI-Entwürfe mit Originalzitaten, keine garantierte inhaltliche Deutung.

## Daten erhalten

Alte Sitzungen lassen sich ausdrücklich schreibgeschützt öffnen und vollständig kopieren. Sie werden nicht automatisch repariert, konvertiert oder überschrieben. Ihre bisherige Nachbearbeitung und Wiederaufnahme erfolgen vorerst weiter in der bisherigen Version. Neue Rust-Sitzungen liegen im lokalen Benutzerordner `HandyMeeting/RustBeta`, getrennt vom Programm und alten Aufnahmen.

Schema 2 verwendet getrennte PCM-Dateien und append-only Audio-/Textjournale. Bestätigung erfolgt erst nach erfolgreicher dauerhafter Speicherung. Vorläufiger Erkennungstext und modellseitig bestätigter Text bleiben unterscheidbar. Beim Wiederaufbau werden frühere Hypothesen nicht über neuere gespeicherte Textstände geschrieben. Bestätigte Textpräfixe dürfen beim selben Modell nicht stillschweigend geändert werden. Eine Abweichung stoppt die betroffene Erkennung mit Hinweis; Audio bleibt erhalten. Prozessabbruch ist kein Stromausfalltest, und ein unbestätigter Audioschluss kann fehlen.

## Freigabestufen

1. **Beta 1:** Windows-Kern und Tauri-Oberfläche, Handy-Modellabfragen, lokale Streaming-Bindings, Historie und belegte Nachbearbeitung. Kurze synthetische Abbruchtests und echte lokale Modellprüfungen. Grenzen sichtbar dokumentieren.
2. **Weitere Betas:** Geräteaufnahme, Geräteverlust und voller Datenträger, Mikrofon-Stummschaltung, vollständige Agenten-/Wiederaufnahmebedienung und erneuter Zweistundenlauf mit vorab festgelegten Speichergrenzen. Mac-/Linux-Adapter schrittweise anbinden und auf echten Zielrechnern testen.
3. **Release Candidate:** Zielplattformen bestehen Geräte-, Abbruch-, Update- und Langzeittests. RAM, Private Bytes und GPU-Speicher werden je Prozess/Backend berichtet. Offene Datenverlust- und Datenschutzfehler sperren die Freigabe.
4. **1.0:** Erst nach abgeschlossener RC-Prüfung und dokumentierter Plattformfreigabe. Eine Versionsnummer ersetzt keinen Nachweis.

Die sichere Rust-Bindung erlaubt aktuell einen aktiven Stream je Modellinstanz. Zwei gleichzeitig offene Spuren verwenden getrennte Worker/Modelle; der RAM-Bedarf kann höher sein als beim früheren gemeinsamen C-API-Modell. Eine gemeinsam genutzte Instanz benötigt eine unterstützte Upstream-Schnittstelle und wird nicht durch eigene unsichere FFI erzwungen.

Details: [Handy-Kompatibilität](HANDY_COMPATIBILITY.md), [Speicherung](RUST_STORAGE.md), [Abhängigkeiten](RUST_DEPENDENCIES.md), [Prüfbericht](../TEST_REPORT.md).
