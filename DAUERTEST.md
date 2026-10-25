# Ergebnis des zweistündigen Echtzeittests

Am **25.10.2026 von 12:12:33 bis 14:12:52 Uhr (Europe/Berlin)** wurde der vollständige Wiederholungslauf einschließlich Wiederherstellung abgeschlossen. Bis zur Absturzquittung vergingen **zwei Stunden und 7,35 Sekunden in Echtzeit**; je Audiospur wurden exakt **zwei Stunden und 7,000 Sekunden** wiederhergestellt.

**Ergebnis: Datenrettung bestanden. Speicherprüfung mit einer offenen GPU-Auffälligkeit.** Der Rohbefund bleibt unverändert bei Success=false und DataRecoveryPassed=true. Die vorab gesetzten Grenzen wurden nicht angepasst.

## Aufnahme und Wiederherstellung

| Prüfung | Ergebnis |
| --- | --- |
| Zwei getrennte Spuren | Je 241 Dateien und 115.312.000 Samples |
| Gesamtmenge | 482 Dateien, 461.248.000 PCM-Bytes |
| Vergleich mit den beiden Referenzsignalen | Alle gespeicherten Samples bytegleich, keine Lücken oder Überlappungen |
| Zwei harte Erkennungsabbrüche | Bei weiterlaufender Aufnahme automatisch aufgeholt |
| Harter Abbruch des gesamten Aufnahmeprozesses | Gesicherte Aufnahme und Textjournal wiederhergestellt |
| Vorher bestätigte Daten | 886 Audiopräfixvergleiche über drei Abbruchquittungen und drei Journalpräfixe unabhängig per SHA-256 nachgeprüft |
| Fertigstellung des Transkripts | Alle 482 Abschnitte abgeschlossen, alle signalführenden Abschnitte mit Text |
| Getrennte Referenztexte | Richtiger Referenzbegriff in allen 480 vollständigen 30-Sekunden-Abschnitten |
| Fehlender unbestätigter Schluss | In diesem Lauf 0 Samples auf beiden Spuren |
| Nachverarbeitung nach dem Gesamtabbruch | Rund 9,4 Sekunden |

Zusätzlich zum ursprünglichen Testprogramm prüfte ein unabhängiger Python-Auswerter WAV-Format, Dateilängen, Zeitabdeckung, alle Samples, Quellenbegriffe und geschützte Hash-Präfixe. Der Prüfsummenabgleich bestätigte die unveränderte Testlaufzeit. Nachweis: [independent-verification.json](.build/soak-2h-20261025-r2/independent-verification.json).

Null verlorene Samples in diesem Lauf sind keine allgemeine Verlustgarantie. In der kurzen Vorprüfung fehlten 44 Millisekunden je Spur am noch nicht bestätigten Ende. Ein Prozessabbruch lässt Betriebssystem- und Datenträgercaches bestehen; ein Stromausfall oder Datenträgerdefekt wurde nicht nachgestellt.

## Arbeitsspeicher

Mediane der Anfangs- und Endbereiche nach der jeweiligen Anlaufphase; MiB. Die vollständigen Rohwerte und Spitzen stehen im Detailbericht. Working Set, Private Bytes und verwalteter Speicher sind getrennte Größen und werden nicht addiert. Grafikspeicher wurde nicht separat gemessen.

| Messgröße | Anfang | Ende | Änderung |
| --- | ---: | ---: | ---: |
| Aufnahmeprozess Working Set | 109,5 | 121,8 | +12,3 |
| Aufnahmeprozess Private Bytes | 59,4 | 79,0 | +19,6 |
| Aufnahmeprozess verwalteter Speicher | 11,7 | 22,5 | +10,8 |
| Erster GPU-Prozess Working Set | 264,3 | 276,8 | +12,5 |
| Erster GPU-Prozess Private Bytes | 1.832,8 | 1.845,2 | +12,3 |
| Zweiter GPU-Prozess Working Set | 266,4 | 236,3 | −30,1 |
| Zweiter GPU-Prozess Private Bytes | 1.787,0 | 1.815,8 | +28,7 |
| CPU-Ersatzbetrieb Working Set | 1.257,5 | 1.268,3 | +10,8 |
| CPU-Ersatzbetrieb Private Bytes | 877,7 | 887,6 | +9,9 |

Aufnahmeprozess und CPU-Phase blieben unter den vorab gesetzten Wachstumsgrenzen. Die CPU-Erkennung benötigt deutlich mehr System-RAM als die GPU-Erkennung in diesem Versuch. Der unterschiedliche Modus ist kein Wachstum desselben Prozesses.

**Offener Befund:** Für die Private Bytes des ersten GPU-Prozesses ergibt die festgelegte lineare Auswertung **276,4 MiB pro Stunde**, oberhalb der Grenze von **128 MiB pro Stunde**. In derselben ungefähr 18-minütigen Auswertungsphase steigt der Anfangs-/Endmedian nur um rund 12,3 MiB. Die Rohkurve enthält frühe Einbrüche auf etwa 1.480 MiB, die später in dieser Phase nicht mehr erfasst werden. Das beeinflusst den linearen Fit. Ein Speicherleck, ein Wechsel nativer Puffer oder ein Effekt des Abtastzeitpunkts ist damit nicht abschließend unterschieden. Die Warnung bleibt bestehen.

Als nächster gezielter Nachweis bietet sich eine längere ununterbrochene GPU-Messung mit dichterer Abtastung und Untersuchung der nativen Pufferlebensdauer an. Der aktuelle Lauf enthält absichtlich drei Erkennungsprozesse: ungefähr 20 Minuten GPU, 60 Minuten GPU und 40 Minuten CPU.

[Speicherverlauf als PNG](.build/soak-2h-20261025-r2/memory-overview.png) · [SVG](.build/soak-2h-20261025-r2/memory-overview.svg) · [Messwerte als CSV](.build/soak-2h-20261025-r2/memory.csv)

## Prüfaufbau und Grenzen

Zwei unterschiedliche synthetische 16-kHz-PCM-Spuren wurden in Echtzeit in die produktive PCM-Sicherung und Streaming-Koordination eingespeist. Die Systemspur handelt vom Projektplan, die Mikrofonspur von Bananen. Es wurden keine echten WASAPI-Geräte aufgenommen. WPF-Steuerelemente wurden unsichtbar aktualisiert; die Leistung eines sichtbaren Fensters und seiner Grafikdarstellung wurde nicht abgenommen.

Der Test verwendete die unveränderte 0.2.1-Testlaufzeit, transcribe.cpp 0.4.1 und das vorhandene lokale Nemotron Streaming 3.5. Parallel wurde auf Nutzerwunsch Nachbearbeitung 0.3.0 entwickelt und mit einem separaten lokalen Gemma-Modell im CPU-Betrieb geprüft. Die protokollierte Last von etwa 13:29 bis 13:47 Uhr ist im Diagramm markiert. Weitere normale Rechneraktivität wurde nicht isoliert. Daraus wird keine Ursache für einzelne Speicherschwankungen abgeleitet.

Der erste Versuch ab 10:34 Uhr brach nach rund 82 Minuten durch eine kurzzeitige Statusdateisperre im Testwächter ab. Dessen anschließende Datenrettung bestand; er zählt nicht als Zweistundentest. Der korrigierte Wächter bestand absichtliche Sperren und 1.000 gleichzeitige Statuswechsel. Alle alten Nachweise bleiben unter [.build/soak-2h-20261025/REPORT-INTERRUPTED.md](.build/soak-2h-20261025/REPORT-INTERRUPTED.md) erhalten.

## Berichte und Rohdaten

- [Vollständiger automatisch erzeugter Bericht](.build/soak-2h-20261025-r2/REPORT.md)
- [Unverändertes Testergebnis](.build/soak-2h-20261025-r2/result.json)
- [Vorab gesetzte Prüfgrenzen](.build/soak-2h-20261025-r2/plan.json)
- [Speicher-Rohdaten](.build/soak-2h-20261025-r2/memory.jsonl)
- [Parallel dokumentierte Aktivität](.build/soak-2h-20261025-r2/external-activity.jsonl)
- [Gesamter Prüfbericht einschließlich Nachbearbeitung 0.3.0](TEST_REPORT.md)

Die ausschließlich für den Test eingerichtete automatische Überwachung endet nach dieser Auswertung. Alle Aufnahmen und Protokolle bleiben erhalten. Bestehende Nutzeraufnahmen und laufende HandyMeeting- beziehungsweise LM-Studio-Prozesse wurden nicht beendet.
