# Prüfung von HandyMeeting

Stand: 25. Oktober 2026. Native Erkennung: transcribe.cpp 0.4.1, vorhandenes Nemotron Streaming 3.5. Windows x64, .NET 10.

## Tatsächliche GitHub-Prüfungen vom 25.10.2026

Windows mit 86 benannten Prüffällen und zusätzlichen Monitor-Dateiprüfungen: [erfolgreicher Lauf](https://github.com/Puma7/handy-meeting/actions/runs/40210563368). Die portablen Ziele wurden auf Ubuntu 24.04, macOS 15 Apple Silicon und macOS 15 Intel gebaut; die jeweiligen synthetischen Kern-/Agentenprüfungen bestanden ebenfalls. Auf beiden Macs wurden Swift-Helfer, App-Signatur und DMG-Integrität geprüft: [erfolgreicher portabler Lauf](https://github.com/Puma7/handy-meeting/actions/runs/40210637828). Nachweisstand ist Commit `60335cd0c26aca341a4013dac21bcf5423f95ffe`; nachfolgende Paketverfeinerungen werden vor Veröffentlichung erneut gebaut. Aktueller Überblick: [BUILD_STATUS.md](BUILD_STATUS.md).

Der erste private Lauf wurde wegen des Actions-Budgets gar nicht gestartet. Der erste Mac-Paketlauf erkannte eine unsignierte verwaltete .NET-Assembly; nach der gezielten Korrektur und einer expliziten Signatur-Regressionsprüfung bestanden beide Architekturen. Diese CI-Läufe bestätigen keine echte Geräteaufnahme, native Fensterbedienung oder portablen Zweistundenlauf. Frühere Abschnitte unten bleiben historische lokale Messprotokolle und beschreiben den Stand vor diesen GitHub-Läufen.

## Windows 0.4.1 und portable Vorschau 0.5.0-preview.1

**Windows: 86 Prüffälle bestanden** (18 PCM/Grundlagen, 35 Nachbearbeitung, 25 Vorlagen, acht Agentenfälle), zusätzlich die Statusdateiprüfung des Testwächters. Letzter vollständiger Lauf: `.build/verification-20261025-161437-0231f5`. Die sichtbare Bearbeiten-Aktion wurde in Deutsch und Englisch aus echten WPF-Steuerelementen gerendert; der Editor erklärt Arbeitsauftrag und Ausgabeform. Logo und App-Icon sind eingebunden. Die neue Agentenschnittstelle wurde über eigene echte Kindprozesse und stdin/stdout geprüft: ausdrückliche Textfreigabe, wörtlicher Quelltext, begrenzte Seiten, ungültige Sitzungskennungen, doppelte/zu große Anfragen mit anschließender Wiederaufnahme des Protokolls, abgelehnte Cloud- und Aufnahmebefehle. Ein vorbereiteter GitHub-Workflow ist weiterhin kein ausgeführter CI-Nachweis.

Ein zusätzlicher Windows-CPU-Probelauf mit dem vorhandenen Nemotron-Streaming-Modell und einem synthetischen Systemspur-Ausschnitt lieferte erneut den erwarteten Projektplan-Text. Keine echten Geräte wurden geöffnet. Befund: `.build/windows-native-0.4.1/stream-probe.json`. Die modellseitigen Prompts wurden bei dieser UI-/Portierungserweiterung nicht geändert; die echten Gemma-Referenzfälle von 0.4 weiter unten bleiben als frühere Nachweise erhalten.

**Linux: Zehn Kernprüfungen bestanden**, einschließlich beider PCM-Spuren mit identischen Wiederherstellungs-Hashes, Transkriptjournal, Originalzitatprüfung, lokaler Zielbindung, fehlender Cloud-Freigabe und Agentenschnittstelle. Die native CPU-Streaming-Anbindung transkribierte einen vorhandenen synthetischen WAV-Ausschnitt mit denselben vorhandenen Modellgewichten erfolgreich. Ein separater Test nutzte zwei unterschiedliche synthetische Signale im tatsächlichen Capture-Prozesspfad, beendete den eigenen Aufnahmehost hart und bestätigte danach bytegleiche gesicherte Audio-Präfixe und beide Textjournalstände. Rohdaten unter `.build/portable-linux-final`, `.build/portable-linux-native` und `.build/portable-linux-capture-final`.

Diese Linux-Prüfungen liefen in Ubuntu 26.04 unter WSL auf demselben Windows-Rechner (i7-13700, rund 64 GiB RAM; native Probe ausdrücklich CPU). Die minimal installierte Umgebung besitzt kein ICU. Ausschließlich für diese Testprozesse wurde der Globalisierungs-Invariantmodus gesetzt; das Installationspaket verlangt ICU. Echte PulseAudio-/PipeWire-Geräte, Linux-Desktopdarstellung, Geräteverlust und ein Linux-Zweistundentest sind nicht geprüft. Der synthetische Aufnahme-Abbruchtest dauerte nur wenige Sekunden. Die bestätigten Präfixe sind nachgewiesen; ein unbestätigter Schluss kann fehlen. Ein Prozessabbruch ist kein Stromausfall. Neue Working-Set-/Private-Bytes-/VRAM-Langzeittrends werden daraus nicht abgeleitet; die native Probe enthält nur Momentaufnahmen des steuernden Prozesses.

**Paket:** Ein `.deb` für Ubuntu/Debian x64 wurde mit `dpkg-deb` gebaut und als technische Vorschau gekennzeichnet. Die Abhängigkeiten umfassen ICU, Grafikbibliotheken und PulseAudio-Werkzeuge. Das Paket wurde nicht auf einem Ubuntu-/Debian-Desktop installiert und ist nicht für wichtige reale Meetings freigegeben. Die Avalonia-Oberfläche wurde separat ohne sichtbares Fenster auf Windows gerendert und visuell kontrolliert (`.build/portable-ui-final`); dies belegt keine native Linux- oder Mac-Darstellung.

**Mac:** Die .NET-Ziele für Apple Silicon und Intel ließen sich ohne Compilerwarnungen von Windows aus veröffentlichen; passende native Bibliotheken wurden mit festgelegten SHA-256-Werten bezogen. Der Swift-Audiohelfer, die Mac-Berechtigungen, ad-hoc Signierung und DMG-Erzeugung sind als Quellen/Skripte vorbereitet, wurden aber mangels Mac-Buildhost nicht ausgeführt. Es liegt keine erfolgreich auf macOS gebaute DMG und kein Mac-Audionachweis vor. Schlüssel bleiben in der portablen Vorschau nur im Arbeitsspeicher; eine verifizierte native Schlüsselbundanbindung fehlt. Weitere Freigabeschritte: [PORTABILITY.md](PORTABILITY.md).

Die gesperrten Portable-NuGet-Abhängigkeiten wurden gegen NuGets bekannte Sicherheitsmeldungen abgefragt; die Abfrage lieferte keine gemeldeten betroffenen Pakete (`.build/portable-dependency-vulnerabilities.json`). Dies ist keine Aussage über die Abwesenheit unbekannter Schwachstellen. Lizenzmetadaten und mitgelieferte Hinweise von 27 Paket-/Versionskombinationen sowie die Inter-OFL und Avalonia-MIT stehen unter `licenses/`; native Archive erhalten ihre eigenen Lizenzdateien. Aufnahmen, Modelle, Einstellungen und Schlüssel sind aus dem vorbereiteten GitHub-Quelltext-Export ausgeschlossen.

Zusätzlich wurde der tatsächlich im Linux-Paket enthaltene plattformneutrale Kern über einen eigenen Windows-Referenzhost mit dem echten lokalen Gemma-Textmodell geprüft. Ein synthetischer positiver Auftrag samt englischem Zusatzbereich und ausdrücklich unverbindliche deutsch-englische Aussagen bestanden die Quellen- und Inhaltsreferenzkriterien; keine Cloud- oder echten Nutzertexte. Der Referenzhost war Windows, kein Linux-Desktop. Befund: `.build/portable-text-reference/results/reference.json`. Das entpackte Linux-Paket enthält bytegleich dieselbe `HandyMeeting.Core.dll` wie der geprüfte Portable-Build. Für Windows sind EXE und Assembly bytegleich zum vollständigen Prüfbuild: `.build/release-0.4.1-verification.json`.

## Frei anpassbare Vorlagen und Oberfläche in Version 0.4.0

**verify.ps1 bestanden: 18 Grundprüfungen, 35 bisherige Nachbearbeitungsprüfungen und 25 neue Vorlagenprüffälle**, zusätzlich die Statusdatei-Sperrprüfung des Testwächters. Build mit gesperrten Paketversionen, null Compilerwarnungen und null Fehlern. Letzter Prüflauf: [verification-20261025-150728-0a9093](.build/verification-20261025-150728-0a9093/templates/template-checks.json). Der vorbereitete GitHub-Workflow sammelt nun auch diese Berichte; er wurde weiterhin nicht auf GitHub ausgeführt.

Die neuen Prüfungen erkennen insbesondere falsche Originalzitate, doppelte Antwortfelder, veränderte historische Prompts, unzulässige Wiederaufnahme mit verändertem Modell, Prompt oder gleicher Schrittzahl bei anderer Reihenfolge, fehlende Cloud-Freigabe und Schlüssel in gespeicherten Arbeitsständen. 20 unabhängig konfigurierte Tabs durchliefen denselben Textvertrag. Speichern/Laden erhielt ihre Kennungen und Reihenfolge; der Klartext zum Kopieren enthielt alle Tabs mit Quellenkennungen und wörtlichen Zitaten ohne Markdown-Escapezeichen. Eine lange synthetische Zusammenfassung wurde über begrenzte Paaranfragen verdichtet. Benutzerdefinierte Tabnamen mit aktivem HTML und Markdown wurden in Dateiexporten entschärft.

Nach einem Dienstfehler wurde nur der Rest verarbeitet; bestätigte Schrittdateien blieben bytegleich. Ein eigener Prozess wurde nach dem ersten bestätigten Schritt hart beendet. Die Fortsetzung erhielt dessen Datei bytegleich und führte die verbleibenden zwei Schritte aus. Fertige Auswertungen wurden ohne neue Modellanfrage und ohne Umschreiben des Ergebnisses wieder geladen. Die bestehenden Prüfungen für 0.3-Ergebnisse bleiben bestanden. Diese Nachweise betreffen Prozessabbrüche und gesicherte Schritte, keinen Stromausfall.

Die vollständige Transkriptkopie wurde an 24 Abschnitten beider Spuren geprüft, einschließlich des laufenden Texts und älterer Abschnitte außerhalb des auf 16 Abschnitte begrenzten Live-Fensters. Die produktive Aufnahme und der Streaming-Prozess wurden für diese Erweiterung nicht verändert. Für das Kopieren kam ausschließlich ein synchronisierter Lesezugriff hinzu. Der native Zwischenablageinhalt des Nutzers wurde für den automatisierten Test nicht überschrieben.

Hauptfenster und Nachbearbeitung wurden in Deutsch/Hell und Englisch/Dunkel aus echten WPF-Steuerelementen gerendert und visuell kontrolliert: [Hauptfenster](.build/verification-20261025-150728-0a9093/templates/main-en-dark.png), [20 Tabs mit dauerhaft erreichbarem Plus](.build/verification-20261025-150728-0a9093/templates/templates-en-dark.png). Quellenwechsel markierten das exakte Originalzitat. Auch das Einfügen eines anderen Modellnamens im bearbeitbaren Auswahlfeld setzte eine frühere Cloud-Freigabe zurück. Ein Fenster in der Geräteprüfung konnte regulär schließen. Diese Prüfung bediente kein sichtbares Nutzerfenster und ersetzt keinen vollständigen manuellen Durchlauf sämtlicher Dialoge, Tastaturpfade und Anzeige-Skalierungen. System- und Treiberdiagnosen sowie gespeicherte Benutzernamen/Prompts behalten gegebenenfalls ihre Originalsprache.

**Echter lokaler Referenztest des endgültigen Programmstands bestanden:** google/gemma-4-e4b, GGUF Q4_K_M, LM Studio mit 8.192 Kontexttokens; synthetische deutsche Positivaussagen und deutsch-englische unverbindliche Aussagen. Ergebnis: zwei Zusammenfassungspunkte, genau eine ausdrückliche Entscheidung, genau eine Aufgabe mit Anna und Freitag sowie zwei englische Punkte in einer frei definierten Audioanalyse. Der Negativfall lieferte keine Entscheidungen oder Aufgaben. Sechs Modellanfragen benötigten insgesamt **37,37 Sekunden** einschließlich Validierung und Speicherung. Rohdaten: [template-local.json](.build/templates-local-final/template-local.json). Rechner: Intel Core i7-13700, NVIDIA GeForce RTX 4070, Intel UHD 770, 63,74 GiB vom Betriebssystem gemeldeter physischer RAM. Verwendet wurde der bereits geladene lokale Dienst; seine konkrete CPU-/GPU-Aufteilung wurde für diesen Test nicht separat gemessen. Die Laufzeit ist kein Geschwindigkeitsnachweis für lange reale Meetings; Working Set, Private Bytes und Grafikspeicher wurden hier nicht neu profiliert.

Ein erster Modellversuch hatte im Entscheidungsbereich zusätzlich eine Aufgabenübernahme und eine Zustandsbeschreibung aufgeführt. Ein strengerer Referenztest erkannte dies. Systemanweisung und Standardprompt wurden daraufhin gegen diese Verwechslung präzisiert; derselbe Referenztext bestand danach mit genau einer Entscheidung. Die früheren Versuche unter `.build/templates-local-01`, `-02` und `-03` bleiben erhalten. Dieser kleine Referenzsatz belegt keine allgemeine inhaltliche Fehlerfreiheit. Quellenprüfung bestätigt wörtliche Belege, nicht automatisch deren richtige Deutung. Personenanzahl und Wörter je Person werden ohne Sprecherzuordnung nicht als verlässliche Statistik angeboten; freie Analysen erhalten nur begrenzte Originalabschnitte.

Keine realen Nutzertranskripte gingen in diese Modelltests ein. Echte Cloud-Anbieter wurden nicht aufgerufen; Cloud-Anfragen wurden ausschließlich mit simuliertem HTTP-Transport geprüft. Aufnahmen, Journale, ältere Auswertungen und laufende Nutzerprogramme blieben erhalten. Der Zweistundennachweis weiter unten gilt unverändert für die damalige 0.2.1-Testlaufzeit einschließlich ihrer offenen GPU-Speicherauffälligkeit; er ist kein erneuter Dauertest von 0.4.

Bereitgestellt in `dist-0.4.0`. Programmdatei und .NET-Assembly stimmen per SHA-256 bytegleich mit dem endgültigen Prüfbuild überein: [Freigabenachweis](.build/release-0.4.0-verification.json). Die Startdatei verweist auf diese Version; laufende ältere Versionen wurden nicht beendet oder ersetzt.

## Nachbearbeitung in Version 0.3.0

**35 Nachbearbeitungsprüfungen bestanden**, einschließlich unbekannter Quellen, falscher Zitate, erfundener Zuständigkeiten und Termine, doppelter JSON-Felder, Fremdhosts im lokalen Modus, fehlender Cloud-Freigabe, HTTPS-Pflicht, blockierter echter HTTP-Weiterleitung und kontogebundener Verschlüsselung für API-Schlüssel.

Ein eigener Nachbearbeitungsprozess wurde nach dem ersten bestätigten Abschnitt hart beendet. Die Fortsetzung erhielt dessen Datei bytegleich und verarbeitete nur die übrigen Anfragen. Dienstfehler und Abbruch erhielten Arbeitsstände und frühere vollständige Fassungen. Eine bereits fertige Fassung erzeugte beim erneuten Aufruf keine Anfrage und änderte ihre Metadaten nicht. Große synthetische Texte durchliefen eine begrenzte, mehrstufige Zusammenfassung. Veränderungen des Originaltranskriptstands wurden über die Prüfsumme erkannt. Exporte wurden gegen aktive HTML- und Markdown-Inhalte geprüft.

Die WPF-Ergebnisansicht wurde aus ihrem Inhalt gerendert und visuell kontrolliert. Der Quellensprung markierte im automatisierten Test genau das Originalzitat. Ein sichtbares Fenster wurde für diese Prüfung nicht auf dem Nutzerdesktop bedient.

**Zusätzliche lokale Modellprüfung bestanden:** vorhandenes google/gemma-4-e4b in LM Studio, CPU-Betrieb, 8.192 Kontexttokens. Ein synthetisches deutsches Referenzmeeting lieferte drei Zusammenfassungspunkte, eine Entscheidung und zwei To-dos mit gültigen Belegen. Annas Auftrag bis Freitag blieb korrekt zugeordnet. Ein zweiter deutsch-englischer Text mit ausdrücklich unverbindlichen Vorschlägen lieferte weder Entscheidungen noch To-dos. Rohdaten unter `.build/notes-local-02`; technische Prüfungen unter `.build/verification-20261025-135808-4305e3`.

Die zwei Referenzfälle belegen keine allgemeine Zusammenfassungsgenauigkeit. Quellenprüfung bestätigt vorhandene Zitate, nicht die richtige inhaltliche Deutung. Echte Cloud-Provider wurden ohne Zugangsdaten nicht aufgerufen; die Cloud-Verbindung wurde mit demselben Textvertrag simuliert geprüft. Reale Nutzeraufnahmen waren nicht Teil dieser Tests.

Die **18 Grundprüfungen bestanden auch mit 0.3.0 erneut**. Compilerwarnungen gelten als Fehler. Der Ablauf verify.ps1 führt zusätzlich die gezielten Dateisperrtests des Testwächters aus. Der vorbereitete Windows-GitHub-Workflow ist noch kein ausgeführter CI-Nachweis. Inhalt und Gliederung des erweiterten Masterplans wurden zurückgelesen; die Pages-Seitenvorschau wurde nicht visuell geprüft.

Der abgeschlossene Zweistundentest verwendete die unveränderte 0.2.1-Testlaufzeit. Die parallel durchgeführten CPU-Tests des lokalen Textmodells sind im Testordner unter external-activity.jsonl dokumentiert.

## Automatisierte Sicherungsprüfungen

**18 PCM- und Grundprüfungen mit Version 0.2.1 erneut bestanden.** Geprüft wurden unter anderem stille Zeitbereiche, sehr leise Signale, Abschnittsgrenzen, zwei getrennte Spuren, Paketfolgen ohne präzise Treiberzeitmarken, Wiederherstellung von WAV-Dateiköpfen sowie Erhalt der Daten bei Fehlern und Abbruch.

**10 zusätzliche Journal- und Langzeitdatei-Prüfungen mit Version 0.2 bestanden.** Die folgenden Ergebnisse stammen aus der vorherigen Prüfung; der Zwei-Stunden-Dateitest wurde für die Speicheroptimierung nicht erneut erzeugt.

- Ein beschädigtes Sitzungsmanifest wird aus Sicherung, vorhandenen Audiodateien und Textjournal wiederhergestellt.
- Bestätigter und vorläufiger Text bleiben wieder auffindbar.
- Eine abgerissene letzte Journalzeile blockiert spätere Einträge nicht.
- Ältere Textstände bleiben im fortlaufenden Journal erhalten.
- Ein Modellwechsel archiviert die vorherige Textfassung.
- Aktive Sitzungen sind gegen parallele Reparatur gesperrt.
- Die Historie findet eine wiederhergestellte Sitzung.
- Eine erzeugte Zwei-Stunden-Zeitachse mit zwei Spuren ergibt 480 Audiodateien.
- Die PCM-Datenmenge dieser Zeitachse beträgt exakt 460.800.000 Bytes.
- Alle 480 Dateien sind bei der Wiederherstellung auffindbar.

Die Zwei-Stunden-Prüfung erzeugt Dateien mit zwei Stunden Zeitumfang. Sie ist kein zweistündiger Echtzeit-Praxistest.

## Echte lokale Streaming-Erkennung

Eine lokal erzeugte deutsche Teststimme wurde am Fractal-Scape-Kopfhörerausgang abgespielt und von der App per WASAPI-Loopback aufgenommen.

- Live-Text wurde **vor dem Beenden der Aufnahme** gespeichert.
- Das Nemotron-Modell wurde für die normale Sitzung **genau einmal** geladen.
- Verwendetes Gerät für die Erkennung: Vulkan-GPU.
- Der Testsatz wurde inhaltlich korrekt erkannt; die Zeichensetzung wich teilweise von der Vorlage ab.
- Der automatische Test öffnete kein Mikrofon.

Als zusätzlicher Test wurde dieselbe Aufnahme über die lokale CPU-Streaming-Engine verarbeitet. Für rund zwölf Sekunden Testaudio wurden etwa 2,7 Sekunden inklusive etwa 0,9 Sekunden Modellladen gemessen. Das ist ein einzelner Dateitest mit paketweiser Streaming-Verarbeitung und kein allgemeines Leistungsversprechen.

Testtext:

> Dies ist ein lokaler Test. Am Montag besprechen wir den Projektplan. Die Aufnahme bleibt auf diesem Computer.

## Gezielte Prozessabbrüche

**Gesamten eigenen Testprozess abrupt beendet:** Bereits bestätigtes Audio blieb vorhanden. Der Live-Text wurde aus dem Journal wiederhergestellt. Anschließend wurde das offene Audio vollständig nachverarbeitet. Der vor dem Abbruch vorhandene Journalanfang blieb bytegleich erhalten.

**ASR-Prozess bei zwei unterschiedlichen Testspuren beendet:** Der Erkennungsprozess wurde neu gestartet und beide Spuren wurden fertig verarbeitet. Die Systemspur sprach vom Projektplan; die synthetische Mikrofonspur sprach von Bananen. Die Ergebnisse blieben den richtigen Spuren zugeordnet. Es wurde kein echtes Mikrofon für diesen automatisierten Test aufgenommen.

**ASR-Prozess während einer echten laufenden Systemaudioaufnahme beendet:** Die Audioaufnahme lief weiter. Der Erkennungsprozess startete einmal neu. Beide nacheinander abgespielten Testsätze erschienen anschließend im Transkript. Auch hier gab es bereits vor dem Aufnahmeende gespeicherten Live-Text.

Der erste zusätzliche Hardwaretest zeigte eine falsche Unterbrechungsmeldung beim ersten Treiberpaket nach dem vorsorglichen Schreiben von Stille. Die Erkennung verwendet jetzt das erste tatsächlich empfangene Paket als Ausgangspunkt. Spätere gemeldete Unterbrechungen werden weiterhin sichtbar gekennzeichnet.

## Speicherprüfung und Änderungen in 0.2.1

Die Statusabfragen der Version 0.2 kopierten bei jedem Durchlauf die gesamte Liste der Audioabschnitte mehrfach. Auch der Live-Text sortierte Kopien aller Abschnitte, obwohl nur die jüngsten 16 angezeigt werden. Die Verarbeitungsschleife löste zusätzlich zum Oberflächentakt eigene Aktualisierungen aus.

0.2.1 fragt offene Abschnitte gezielt ab, berechnet Zähler ohne Listenkopien und begrenzt die temporäre Liste der Live-Ansicht auf 16 Einträge. Die Oberfläche liest den Fortschritt über ihren vorhandenen 250-ms-Takt; Fehler und Zustandswechsel werden weiterhin gemeldet. Der Erkennungsprozess verwendet feste PCM- und Float-Puffer wieder. Audiosicherung, Textjournal und Abschnittsgröße wurden dafür nicht verändert. Es werden keine erzwungenen Speicherbereinigungen oder künstlichen Begrenzungen der Windows-Speicheranzeige verwendet.

Vergleich von **1.000 identischen Abfragezyklen**, einschließlich regelmäßiger Darstellung des Live-Texts. Erzeugte Metadaten, zwei Spuren, absichtlich unsortierte Abschnitte; keine echten Meetingtexte. Angegeben ist die Summe temporärer Speicherzuweisungen in dezimalen MB, **keine dauerhaft belegte RAM-Menge**:

| Umfang der Metadaten | Vorher | 0.2.1 |
| --- | ---: | ---: |
| 4 Abschnitte | 5,34 MB | 0,96 MB |
| 2 Stunden / 480 Abschnitte | 310,11 MB | 2,79 MB |
| 12 Stunden / 2.880 Abschnitte | 1.838,88 MB | 2,79 MB |

Die Ergebnisprüfsummen waren im Vorher-/Nachhervergleich identisch. Die begrenzte Live-Auswahl und die Auswahl offener Abschnitte wurden zusätzlich auf Reihenfolge und unabhängige Kopien geprüft. Der Test setzt ein Budget von 12 MB temporären Zuweisungen pro 1.000 Zyklen voraus. Die Messung erfasst diese Abfragepfade, nicht den gesamten Verbrauch der WPF-Oberfläche.

Bei **sechs Minuten synthetischem Testaudio je Spur** sank die Summe verwalteter Speicherzuweisungen des Erkennungsprozesses von 72,97 auf 3,80 MB, ungefähr **95 %**. Modellladen ist aus dieser Differenz herausgerechnet. Alle 24 Abschnitte enthielten vor und nach der Änderung den Referenzbegriff. Die Verarbeitung dauerte jeweils ungefähr 20,5 Sekunden. Daraus folgt keine gleich große Verringerung des tatsächlichen Arbeitsspeichers.

Zusätzlich wurden mit der ausgelieferten Version **30 Minuten je Spur** in 60 Fenstern verarbeitet, insgesamt 120 getrennte Erkennungsabschnitte. Alle enthielten den Referenzbegriff; Laufzeit rund 101 Sekunden, Vulkan auf diesem Rechner. Am letzten Messpunkt hatte der Erkennungsprozess etwa **320 MiB Working Set** und **1.915 MiB Private Bytes**. Die Private Bytes schwankten nach der Anlaufphase zwischen ungefähr 1.879 und 1.964 MiB. Ein unbegrenzter Anstieg dieser Größe zeigte sich in diesem Test nicht. Der Working Set wuchs während des Tests weiter moderat; eine vollständige Speicherleckfreiheit ist damit nicht bewiesen. Die Windows-Werte sind getrennte Messgrößen und werden nicht addiert; Grafikspeicher wurde nicht separat gemessen. Hauptfenster und gleichzeitig laufendes Handy sind in diesen Worker-Zahlen nicht enthalten.

Der native Modell- und Rechenbedarf bleibt für schwächere Rechner relevant. Ein festes Gesamtbudget von beispielsweise 300 MB lässt sich aus diesen Ergebnissen nicht zusagen.

**Mit 0.2.1 erneut bestandener Abbruchtest:** Eigenen synthetischen Aufnahmeprozess beendet, gesichertes Audio wiederhergestellt, zuvor geschriebenen Journalanfang bytegleich erhalten und Erkennung abgeschlossen. Anschließend den Erkennungsprozess während zweier unterschiedlicher Testspuren beendet: ein Neustart, beide Spuren vollständig und korrekt getrennt erkannt. Die 18 PCM-Grundprüfungen bestanden ebenfalls erneut. Die realen Nutzeraufnahmen unter Sessions wurden für diese Prüfungen weder verändert noch verwendet.

Rohdaten unter `.build/memory-before`, `.build/memory-after`, `.build/memory-long`, `.build/memory-regression` und `.build/memory-recovery`. Die Vorher-Fassung liegt ausschließlich für den Vergleich unter `.build/memory-baseline`.

Die Mikrofonwarnung der vorhandenen 56-Sekunden-Nutzeraufnahme lässt sich aus deren Metadaten nicht zeitlich eingrenzen: gespeichert wurde ein allgemeines Treibersignal ohne einzelne Ereigniszeitpunkte. Sie wurde nicht als bewiesener Speicherfehler eingestuft. Vorhandene Warnungen bleiben erhalten.

## Zweistündiger Echtzeittest mit abgeschlossener Wiederherstellung

Der erste Echtzeitlauf am 25.10.2026 wurde nach etwa 82 Minuten durch einen Fehler beim Statuslesen im Testwächter beendet. Die Windows-Statusdatei war während eines gleichzeitigen Ersetzens kurz gesperrt. Die beiden vorher geplanten Erkennungsabbrüche waren erfolgreich aufgeholt worden. Dieser abgebrochene Lauf gilt ausdrücklich nicht als bestandener Zweistundentest.

Die Wiederherstellungsprüfung der vorhandenen Daten bestand: sämtliche gespeicherten PCM-Samples stimmen mit ihren Referenzspuren überein, die vor beiden Erkennungsabbrüchen bestätigten Audio- und Textjournal-Präfixe blieben hashgleich, und alle vorhandenen Abschnitte wurden vollständig und getrennt erkannt. Der Zeitumfang beträgt rund 82 Minuten je Spur. Nachweise: [.build/soak-2h-20261025/REPORT-INTERRUPTED.md](.build/soak-2h-20261025/REPORT-INTERRUPTED.md) und [interrupted-recovery.json](.build/soak-2h-20261025/interrupted-recovery.json).

Der korrigierte Testwächter bestand eine gezielte Prüfung mit exklusiver Dateisperre und 1.000 gleichzeitigen Statuswechseln. Der vollständige Wiederholungslauf dauerte am 25.10.2026 von 12:12:33 bis 14:12:52 Uhr einschließlich Wiederherstellung. Aufnahmelaufzeit: 7.207,35 Sekunden in Echtzeit. Beide Spuren enthalten jeweils exakt 7.207 Sekunden, zusammen 482 Dateien und 461.248.000 PCM-Bytes. Nach zwei Erkennungsabbrüchen und einem harten Abbruch des Aufnahmeprozesses blieben sämtliche bestätigten Audio- und Textjournal-Präfixe erhalten. In diesem Lauf fehlte auch am unbestätigten Schluss kein Sample. Die offenen Abschnitte waren nach rund 9,4 Sekunden nachverarbeitet.

Eine unabhängige Python-Prüfung bestätigte alle gespeicherten Samples gegen die beiden Referenzspuren, WAV-Dateilängen, lückenlose Zeitabdeckung, 886 geschützte Audiopräfixe, drei Journalpräfixe und die unveränderte Testlaufzeit. Alle 482 signalführenden Abschnitte enthalten Text. In allen 480 vollständigen 30-Sekunden-Abschnitten blieb der Referenzbegriff der richtigen Spur zugeordnet. Dies ist kein Wortfehlerraten-Nachweis für natürliche Meetings.

**Speicherteil nicht vollständig freigegeben:** Beim Aufnahmeprozess stieg der Working-Set-Median von 109,5 auf 121,8 MiB, Private Bytes von 59,4 auf 79,0 MiB. Die CPU-Phase blieb mit 1.257,5 auf 1.268,3 MiB Working Set und 877,7 auf 887,6 MiB Private Bytes unter den Prüfgrenzen. Die GPU-Prozesse lagen im System-RAM ungefähr zwischen 236 und 297 MiB; Grafikspeicher wurde nicht separat gemessen. Für die Private Bytes des ersten GPU-Prozesses überschritt der festgelegte lineare Trend mit 276,4 MiB/h die Grenze von 128 MiB/h. Dessen Anfangs-/Endmedian stieg lediglich um 12,3 MiB; die Rohkurve zeigt frühe Einbrüche und periodische Schwankungen. Der Befund belegt noch kein Speicherleck, bleibt aber eine offene Auffälligkeit. Prüfgrenzen und Success=false des Rohbefunds wurden nicht geändert.

Die drei Erkennungsprozesse liefen etwa 20 Minuten auf GPU, 60 Minuten auf GPU und 40 Minuten auf CPU. Das ist kein zweistündiger ununterbrochener GPU-Prozess. Parallel liefen Softwareentwicklung und protokollierte Gemma-CPU-Tests; eine Ursache für einzelne Schwankungen ist damit nicht nachgewiesen. Ein gezielter weiterer GPU-Test mit dichterer Abtastung und Prüfung der nativen Pufferlebensdauer ist sinnvoll.

Vollständige Auswertung und Grenzen: [DAUERTEST.md](DAUERTEST.md). [Speicherdiagramm](.build/soak-2h-20261025-r2/memory-overview.png), [automatischer Bericht](.build/soak-2h-20261025-r2/REPORT.md), [unveränderter Rohbefund](.build/soak-2h-20261025-r2/result.json), [unabhängige Prüfung](.build/soak-2h-20261025-r2/independent-verification.json). Alle Testaufnahmen und Protokolle bleiben erhalten.

## Grenzen der Prüfung

Nicht praktisch geprüft wurden ein zweistündiges reales Meeting mit echten WASAPI-Geräten, die Darstellung eines sichtbaren Fensters während eines solchen Meetings, Stromausfall, physischer Datenträgerdefekt und echter USB-Geräteverlust während eines langen Meetings. Der bestandene synthetische Zweistundenlauf ersetzt diese Hardware- und Meetingprüfungen nicht. Ein Prozessabbruch ist kein Stromausfall.

Durchschreiben, Sicherungskopien, Textjournal und unabhängige Aufnahme begrenzen die Auswirkungen typischer Softwarefehler. Sie können keine absolute Zusage gegen jeden Hardware- oder Datenträgerfehler geben. Die App löscht Originalaufnahmen und das Textjournal nicht automatisch.
