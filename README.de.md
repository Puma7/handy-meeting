# HandyMeeting · lokale Meetings im Handy-Ökosystem

**Nächste Hauptversion:** Die schrittweise Migration zu Rust und Tauri läuft als `1.0.0-beta.1`, zunächst für Windows. Handys aktuelle Modellliste und offizielle Erkennungsbibliotheken bleiben die Basis. Bisherige Prototypen und Aufnahmen bleiben erhalten. [Umfang und Freigabeschritte](docs/RUST_MIGRATION.md).

HandyMeeting nimmt Computerton und optional Mikrofon getrennt auf und transkribiert **während des Meetings**. Die Erkennung verwendet eine lokale transcribe.cpp-Engine und ein bereits in Handy installiertes Streaming-Modell. Das Modell bleibt für die Sitzung geladen.

**Plattformstand:** Windows 0.4.1; technische Linux-Vorschau 0.5.0-preview.1 für Ubuntu/Debian x64; Mac-DMGs für Apple Silicon und Intel wurden auf echten Mac-Runnern gebaut und geprüft. Echte Mac-/Linux-Audiogeräte und lange Meetings sind damit noch nicht validiert. Die Mac-Vorschau zielt auf macOS 15 oder neuer und ist ad hoc signiert, nicht Apple-notarisiert. Downloads und aktuelle Build-Nachweise: [BUILD_STATUS.md](BUILD_STATUS.md). Weitere Grenzen: [PORTABILITY.md](PORTABILITY.md). Eigenes Logo, App-Icon und Favicon liegen unter `assets/`. Dies ist eine eigenständige Companion App und kein offizielles Handy-Produkt.

## Starten

Downloads: [Windows-ZIP 0.4.1](https://github.com/Puma7/handy-meeting/releases/tag/v0.4.1), [Mac-DMGs und Linux-DEB 0.5.0-preview.1](https://github.com/Puma7/handy-meeting/releases/tag/v0.5.0-preview.1). Beide Vorschau-Releases enthalten Prüfsummen. Mac und Linux benötigen noch reale Geräte- und Langzeitprüfungen.

Das Windows-ZIP vollständig entpacken und **Start HandyMeeting.cmd** doppelklicken. Im Quellprojekt heißt diese Datei **HandyMeeting starten.cmd**; alternativ **dist-0.4.1\HandyMeeting.exe** öffnen. Eine noch laufende ältere Fassung vorher regulär schließen. Windows x64, .NET Desktop Runtime 10 und Handy mit einem installierten Streaming-Modell werden benötigt.

Neue Versionen werden in einem eigenen Programmordner bereitgestellt. Die Startdatei verweist nach einem erfolgreichen Build auf die neue Fassung. **Sessions** und **data** bleiben gemeinsam im Projektordner; vorhandene Aufnahmen, Textstände und Einstellungen werden weiterverwendet.

1. Dieselbe Audioausgabe auswählen, die Teams, Zoom oder der Browser verwendet.
2. Bei Bedarf das Mikrofon zusätzlich aktivieren und das gewünschte Gerät wählen.
3. Ein installiertes Streaming-Modell auswählen; geprüft ist **Nemotron Streaming 3.5**.
4. **Aufnahme starten** wählen. Audio wird sofort lokal geschrieben; während das Modell lädt, wächst die gesicherte Aufnahme bereits weiter.
5. Den Live-Text und den Status „Audio gesichert bis …“ beobachten.
6. **Meeting beenden** schließt die Audiospuren ab und verarbeitet noch offene Pakete mit dem bereits geladenen Modell.

Es werden nur Modelle angeboten, die Handy als streamingfähig meldet. Die Engine prüft diese Fähigkeit zusätzlich beim Laden. Es gibt keine automatische Umstellung auf eine große nachträgliche Dateitranskription.

**Transkript kopieren** kopiert den vollständigen bisherigen Verlauf beider Spuren auf einen Schlag, einschließlich des aktuellen Live-Textes. Die Anzeige selbst bleibt auf die jüngsten 16 Abschnitte begrenzt. Noch laufender Text kann sich verändern; unvollständige Sitzungen werden im kopierten Text gekennzeichnet.

Oben lassen sich **Deutsch / English** und **Hell / Dunkel** einstellen. Die Auswahl wird lokal gespeichert und gilt auch für die Nachbearbeitung. Die Sprache eigener Vorlagen, Prompts und Originaltranskripte wird dadurch nicht geändert. System- und Treibermeldungen können in ihrer ursprünglichen Sprache erscheinen. Buttons und Statusmeldungen bieten nach kurzem Verweilen zusätzliche Hinweise; insbesondere pausiert **Streaming pausieren** nur die Erkennung und lässt die Audioaufnahme weiterlaufen.

Die Mikrofon-Stummschaltung in Teams/Zoom gilt nicht für HandyMeeting. Die App hat eine eigene Mikrofon-Stummschaltung. Mit Kopfhörern vermeidest du doppelte Stimmen durch Lautsprecher-Echo. Computerton umfasst alle Apps auf der gewählten Ausgabe, einschließlich Benachrichtigungen.

## Wie Audio und Text gesichert werden

Die Audioaufnahme und die Spracherkennung laufen unabhängig voneinander:

- PCM-Audio wird fortlaufend in **30-Sekunden-Dateien** geschrieben. Auch die gerade offene Datei wird etwa alle 0,5 Sekunden mit Dateikopf und Daten auf den Datenträger durchgeschrieben.
- Die Erkennung liest ausschließlich bereits bestätigte Audiobereiche von der Platte und verarbeitet kleine Pakete. Das Meeting muss nicht vollständig im Arbeitsspeicher liegen.
- Neue Textstände werden vor der Anzeige in ein **fortlaufendes Textjournal** geschrieben und durchgeschrieben. Bestätigter Text und noch vorläufiger Text werden darin getrennt erhalten.
- Das Modell bleibt geladen. Die kurzen Erkennungsfenster werden abgeschlossen und zurückgesetzt, sodass kein zwei Stunden langer Erkennungskontext aufgebaut werden muss.
- Sitzungsübersichten und Exporte werden über temporäre Dateien ersetzt; die vorherige gültige Datei bleibt zusätzlich als .bak erhalten.
- Originalaudio und Textjournal werden von der App **nicht automatisch gelöscht**.

Ein abgestürzter oder langsamer Erkennungsprozess beendet die Audioaufnahme nicht. Die App zeigt Rückstand und Fehler an, startet den Erkennungsprozess erneut und liest offene Abschnitte aus dem gesicherten Audio. Nach wiederholten Fehlern wird Streaming pausiert; die Aufnahme läuft weiter. Der Prozessor kann als Ersatz für einen fehlerhaften GPU-Betrieb verwendet werden.

**Streaming pausieren** hält nur die Erkennung an. Die Audioaufnahme läuft weiter. **Streaming fortsetzen** holt die gespeicherten Abschnitte nach. Ein pausierter Nachlauf lässt sich später über die Historie fortsetzen.

Eine absolute Zusage gegen jeden Datenverlust ist technisch nicht möglich: Stromausfall, defekte Datenträger oder ein voller Datenträger können auch lokale Dateien betreffen. Der noch nicht durchgeschriebene Schluss hat ein kleines Zeitfenster. Ein Fehler beim Schreiben von Audio hält die Aufnahme sichtbar an. Der Sicherungsstatus zeigt erfolgreich gespeicherte Audiobereiche und keine pauschale Garantie für die Hardware.

## Historie und Wiederaufnahme

Unter **Historie** stehen die Sitzungen mit Datum, Titel, Dauer und Status. **Anzeigen** öffnet das gespeicherte Transkript. **Offene Abschnitte nachholen** verarbeitet noch ausstehendes Audio mit einem Streaming-Modell.

Nach einem Prozessabbruch prüft das Öffnen der Sitzung:

1. Sitzungsübersicht und gegebenenfalls deren Sicherung.
2. Vorhandene Audiodateien einschließlich nicht fertig geschriebener WAV-Dateiköpfe.
3. Das Textjournal mit den schon gespeicherten Textständen.
4. Welche Abschnitte vollständig erkannt wurden und welche erneut verarbeitet werden müssen.

Eine am Ende abgerissene Journalzeile verhindert die Wiederherstellung älterer Einträge nicht. Ein späterer Modellwechsel archiviert die vorherige Textfassung zusätzlich. Eine noch aktive Sitzung wird gegen parallele Reparatur beziehungsweise Verarbeitung gesperrt.

Sitzungen aus Version 0.1 bleiben in derselben Historie sichtbar. Die bisherigen Aufnahmen und Transkripte werden übernommen.

## Dateien und Speicherplatz

Jede Sitzung hat einen eigenen Ordner unter **Sessions** neben diesem Dokument:

| Datei | Inhalt |
| --- | --- |
| audio/system-….wav | Getrennter Computerton, 16 kHz mono, 16-Bit-PCM |
| audio/microphone-….wav | Optionales Mikrofon im selben Format |
| transcript-journal.jsonl | Fortlaufende Textstände mit bestätigtem und vorläufigem Text |
| session.json und session.json.bak | Sitzungsübersicht, Audioabschnitte, Modell und Status |
| transcript.md, transcript.txt, transcript.json | Lesbare beziehungsweise strukturierte Exporte |
| text-history | Gesicherte frühere Textfassungen bei Modellwechseln |

Die Exporte werden beim Abschluss von Erkennungsfenstern und beim Beenden aktualisiert. Das Journal wird bereits bei neuen Live-Textständen geschrieben. Beim Öffnen einer wiederhergestellten Sitzung werden die Exporte aus den gespeicherten Daten neu erstellt.

Rechne mit ungefähr **115 MB pro Stunde und Spur**, also rund **461 MB für zwei Stunden mit Computerton und Mikrofon** zuzüglich Text und Metadaten. Die Anzeige des Live-Texts begrenzt sich aus Leistungsgründen auf die jüngsten Abschnitte. Der gesamte Verlauf bleibt gespeichert und ist nach dem Öffnen der Sitzung beziehungsweise im Export verfügbar.

## Arbeitsspeicher

Audio wird fortlaufend auf den Datenträger geschrieben. Der Erkennungsprozess liest jeweils höchstens eine Sekunde PCM in wiederverwendete Puffer ein; gewöhnlich verarbeitet er eine halbe Sekunde. Das Modell bleibt geladen. Statusabfragen kopieren seit 0.2.1 nicht mehr alle Aufnahmeabschnitte. Die Live-Ansicht kopiert höchstens die jüngsten 16 Textabschnitte und wird über einen gemeinsamen Takt aktualisiert.

Im synthetischen Vergleich mit zwei Stunden Sitzungsmetadaten erzeugten 1.000 Abfragezyklen ungefähr **2,8 statt 310 MB temporäre Speicherzuweisungen**. Diese Zahlen sind keine Messung des dauerhaft belegten Arbeitsspeichers. Den größten Speicherbedarf bestimmt weiterhin die native Modellausführung. Die tatsächlichen Messwerte und Grenzen stehen im [Prüfbericht](TEST_REPORT.md).

## Zusammenfassung Entscheidungen und To-dos

Nach einer vollständig transkribierten Aufnahme **Nachbearbeitung** wählen. Bei älteren Meetings zuerst in der Historie **Anzeigen** wählen. Jeder Ergebnisbereich enthält Quellenverweise. Ein Klick öffnet die unveränderte Originalstelle und markiert das wörtliche Zitat. Zeitangaben bezeichnen Aufnahmeabschnitte.

Unter **Vorlage** lassen sich benannte Vorlagen erstellen, duplizieren, umbenennen und entfernen, beispielsweise „Englisches Arbeitsmeeting“. Jede enthält ihre eigenen Tabs und Prompts. Das **Plus neben den Tabs** fügt einen Verarbeitungsschritt hinzu. **Rechtsklick → Bearbeiten** öffnet Name, Prompt und Ergebnisart. Derselbe Kontext erlaubt das Verschieben und Entfernen von Tabs. Es gibt keine Begrenzung auf drei oder vier Tabs; 20 wurden geprüft. Mindestens ein Schritt und eine Vorlage bleiben erhalten.

Der sichtbare Button **Auftrag bearbeiten** öffnet denselben Editor. **Was soll die KI aus dem Gespräch herausarbeiten?** ist der Arbeitsauftrag, oft auch Prompt genannt. **Wie soll das Ergebnis aussehen?** wählt zwischen eigener Auswertung, kurzem Überblick, beschlossenen Entscheidungen und Aufgaben. Ein erklärender Text nennt bei jeder Auswahl Bedeutung und Grenzen.

Die Ergebnisarten sind freie Analyse, Zusammenfassung, Entscheidungen und Aufgaben mit Zuständigen/Terminen. Der Prompt legt Thema und Ausgabesprache fest. Beispielsweise: „Schreibe auf Englisch. Arbeite ausdrücklich geäußerte Risiken und offene Fragen heraus.“ Originalbelege bleiben in jeder Art Pflicht. Jeder Schritt liest begrenzte Teile des Originaltranskripts unabhängig von den anderen Schritten; eigene Analysen sind keine Kette, die automatisch Ergebnisse anderer Tabs verwendet. Zusammenfassungen werden bei Bedarf weiter verdichtet. Entscheidungen, Aufgaben und freie Analysen behalten alle geprüften Einträge; exakt gleiche Einträge werden zusammengeführt.

Vor dem Start zeigt die App die Anzahl der ersten Modellanfragen an: Schritte × Textabschnitte. Viele Tabs benötigen entsprechend mehr Rechenzeit beziehungsweise optionale Cloud-Anfragen. **Ergebnisse kopieren** kopiert sämtliche Tabs der angezeigten Auswertungsfassung in ihrer Reihenfolge als lesbaren Klartext mit Quellenkennungen und Originalzitaten. Für Dateien bleiben sichere Markdown- und HTML-Exporte verfügbar.

Vorlagen stehen getrennt von **gespeicherten Auswertungen**. Jede Auswertung sichert ihre vollständige Vorlage und deren Prüfsumme. Änderungen an einer historischen Vorlage erzeugen eine eigene Vorlagenkopie; die historische Auswertung bleibt unverändert. Schon geprüfte Arbeitsschritte lassen sich nur mit identischem Transkript, Anbieter, Modell, Prompts und gleicher Schrittfolge fortsetzen. Fertige Auswertungen aus 0.3 bleiben lesbar.

**Sprecherstatistiken:** Computerton und Mikrofon sind Audiospuren, keine verlässlichen Personenkennungen. Ohne vorhandene Sprecherzuordnung kann ein eigener Prompt weder die genaue Personenzahl noch Wörter pro Person zuverlässig bestimmen. Die App erklärt diese Grenze im Prompteditor und weist das Modell an, solche Zahlen nicht zu erfinden. Exakte Statistik über das gesamte Meeting wird durch die abschnittsweise freie KI-Analyse nicht bereitgestellt.

**Lokal ist bei jedem Öffnen der Standard.** Vorgesehen ist LM Studio auf `http://127.0.0.1:1234/v1/`; auch Ollama oder ein anderer kompatibler lokaler Dienst kann verwendet werden. Server starten, **Modelle prüfen**, ein Instruct-Textmodell mit JSON-Schema-Unterstützung und ausreichend Kontext wählen und **Neue Auswertung erstellen** drücken. Für längere Texte mindestens 8.192 Kontexttokens vorsehen. Das Textmodell ist zusätzlich zum Handy-Spracherkennungsmodell nötig. Die App lädt keine Textmodellgewichte herunter und bringt keinen eigenen großen LLM-Stack mit.

Geprüft wurden das vorhandene **google/gemma-4-e4b** und LM Studio mit 8.192 Kontexttokens. Die Vorlagenprüfung in 0.4 umfasst ein deutsches Referenzmeeting, eine eigene englische Audioanalyse und unverbindliche deutsch-englische Vorschläge ohne Entscheidungen oder Aufgaben. Die frühere 0.3-Prüfung lief ausdrücklich auf CPU; der 0.4-Test nutzt das bereits im Dienst geladene Modell. Laden, Beschleunigung und Speicherbedarf steuert der lokale Dienst. Details und Grenzen stehen in TEST_REPORT.md.

**Cloud bleibt optional:** Auf **Cloud · nur Text** umstellen, kompatible HTTPS-API-Adresse, Modell und Schlüssel eintragen und die Textübertragung für diese Auswertung ausdrücklich erlauben. Basisadressen sind beispielsweise `https://openrouter.ai/api/v1/` und `https://api.openai.com/v1/`. Der Anbieter muss das JSON-Schema unterstützen. Gesendet werden nur Transkripttext, Belegkennungen und Verarbeitungsanweisungen. Audio, lokale Dateipfade und Sitzungsdateien gehören nicht zur Anfrage. Es gibt keine automatische Weiterleitung und keinen Cloud-Ersatz bei lokalen Fehlern. Schlüssel werden kontogebunden mit Windows DPAPI verschlüsselt gespeichert. Echte Cloud-Provider und ihre Abrechnung wurden ohne Zugangsdaten nicht aufgerufen; Anfrageformat und Datenschutzgrenzen wurden simuliert geprüft.

Lokal sind nur Loopback-Adressen dieses Computers erlaubt. Den lokalen Dienst selbst mit lokal ausgeführten Modellen betreiben; eine dort separat konfigurierte Cloud-Weiterleitung kann die Companion App nicht kontrollieren. Bekannte Cloud-Modellkennzeichnungen werden lokal abgelehnt.

Lange Transkripte werden in begrenzten Anfragen verarbeitet. Jeder geprüfte Abschnitt wird vor der Fortschrittsbestätigung gespeichert. **Pausieren und sichern** oder Schließen erhält bestätigte Arbeitsschritte. Derselbe Stand kann mit unverändertem Transkript, Modell und Anbieter fortgesetzt werden. Änderungen daran erfordern eine neue Auswertung. Fertige Fassungen bleiben erhalten und lassen sich ohne laufenden KI-Dienst öffnen.

Unter **notes/AUSWERTUNG** im Sitzungsordner liegen `run.json` mit Modell, Promptversion und eingefrorener Vorlage, `source.json` mit Originalstand und Prüfsumme, Zwischenstände je Schritt und Abschnitt, `sections.json`, `notes.md`, `source.md` und `notes.html`. Auswertungen aus 0.3 verwenden weiterhin `result.json`. Wiederverwendbare Vorlagen stehen unter `data/analysis-templates.json`. Die eigenständig lesbare HTML-Datei enthält das Originaltranskript für ihre Quellenverweise. Relative Termine bleiben im Wortlaut erhalten und werden zusammen mit dem Meetingdatum angezeigt.

Ergebnisse sind **KI-Entwürfe**. Vorhandene Quellen, wörtliche Zitate sowie genannte Zuständige und Termine werden technisch geprüft. Das beweist nicht die richtige inhaltliche Deutung. Unbekannte Zuständige und Termine bleiben leer. Aufgaben werden nicht automatisch ausgeführt.

API-Grundlagen: [LM Studio](https://lmstudio.ai/docs/developer/openai-compat/structured-output), [Ollama-Kompatibilität](https://docs.ollama.com/api/openai-compatibility), [OpenRouter](https://openrouter.ai/docs/guides/features/structured-outputs).

## Lokaler Betrieb und Grenzen

- Audio, Erkennung und Textjournal bleiben lokal. Die App hat keinen Cloud-Transkriptionsdienst und keinen Cloud-Ersatzpfad.
- Die großen Modelldateien werden aus Handys vorhandenem Cache gelesen. Es wird keine zweite Modellkopie angelegt.
- Die native Streaming-Laufzeit liegt bei der App. Handy selbst bleibt unabhängig und kann weiter für Diktate verwendet werden.
- Erste Initialisierung und GPU-Vorbereitung können einige Sekunden dauern. Die Audioaufnahme läuft dabei bereits.
- Zeitangaben zeigen Aufnahmebereiche; bei fehlenden Treiberzeitmarken sind sie als angenähert gekennzeichnet. Es sind keine exakten Wortzeitmarken.
- Keine automatische Sprechertrennung oder Echounterdrückung. Die Nachbearbeitung wird vom Nutzer ausgelöst.
- Geräteverlust, Ruhezustand und voller Datenträger werden nicht als nahtlos fortgesetzte Aufnahme ausgegeben.
- Sitzungen sind unverschlüsselte lokale Dateien. Ein automatisch synchronisierter Projektordner wäre eine gesonderte Datenweitergabe durch die Synchronisationssoftware.

## Entwickeln und prüfen

Mit .NET SDK 10 im Projektordner ausführen:

~~~powershell
./build.ps1
~~~

Beim ersten Build bezieht setup-native.ps1 die offizielle transcribe.cpp-Laufzeit **0.4.1** mit festgelegter SHA-256-Prüfsumme. NuGet bezieht NAudio.Wasapi **3.3.0**. Das sind Programm- und Bibliotheksdownloads, keine Modelldownloads oder Übertragung von Aufnahmen. Versionen und Lizenzen liegen im Projekt beziehungsweise in der Ausgabe.

Diagnosemodi der EXE:

| Modus | Prüfung |
| --- | --- |
| --self-test AUSGABEORDNER | PCM-Zeitachse, Grenzen, Stille, leise Signale und Fehlerfälle |
| --streaming-checks AUSGABEORDNER | Textjournal, beschädigte Metadaten und erzeugte Zwei-Stunden-Zeitachse |
| --streaming-smoke AUSGABEORDNER optionalGERÄTEID | Echte Systemaudioaufnahme mit Live-Text vor Aufnahmeende |
| --streaming-recovery AUSGABEORDNER | Gezielt beendeter eigener Testprozess, Wiederherstellung und ASR-Neustart |
| --stream-probe-cpu AUSGABEORDNER PCM-WAV | Streaming im Prozessor-Ersatzbetrieb |
| --memory-profile AUSGABEORDNER vulkan\|cpu optionalFENSTERZAHL | Temporäre Speicherzuweisungen und Speicherwerte beider Prozesse; lokale Teststimme, zwei Spuren, 30 Sekunden je Fenster; Standard 12, maximal 240 Fenster |
| --soak-test NEUER_AUSGABEORDNER optionalSEKUNDEN | Echtzeit-Dauertest mit zwei synthetischen Spuren, Speicherprotokoll und kontrollierten Prozessabbrüchen; Standard 7.207 Sekunden, keine Aufnahme realer Geräte |
| --notes-checks AUSGABEORDNER | Quellen, Datenschutz, Verschlüsselung, Wiederaufnahme, Prozessabbruch und UI mit simulierten Modellantworten |
| --notes-local-smoke AUSGABEORDNER MODELLKENNUNG | Echter lokaler Modelltest mit Referenzauftrag und unverbindlichen Vorschlägen |
| --diagnose AUSGABEORDNER | Geräte und installierte Modelle |

Die Audiotests verwenden eine lokal erzeugte Teststimme. Der automatische Hardwaretest öffnet kein Mikrofon. Der Zwei-Stunden-Dateitest erzeugt rund 461 MB synthetisches PCM; er ersetzt keinen zweistündigen Praxistest. Ergebnisse und Grenzen stehen im [Prüfbericht](TEST_REPORT.md).

HandyMeeting ist ein unabhängiger Arbeitsname und kein offizielles Handy-Produkt.

Externe Agenten können über die dokumentierte [lokale Agentenschnittstelle](AGENT_API.md) Fähigkeiten und Vorlagen lesen sowie nach ausdrücklicher Textfreigabe abgeschlossene Transkripte lesen und lokale Auswertungen erzeugen. Eine vollständige Fernsteuerung von Aufnahme und Oberfläche wird damit noch nicht angeboten. Cloud-Aufrufe sind in dieser Schnittstelle nicht verfügbar. Repository-Vorbereitung und sicherer Quelltext-Export: [REPOSITORY_READY.md](REPOSITORY_READY.md).

Der wiederholbare Ablauf **./verify.ps1** baut mit gesperrten Paketversionen, behandelt Compilerwarnungen als Fehler und prüft Audiosicherung, Nachbearbeitung und Testwächter ohne große Modelle oder Cloud-Zugangsdaten. Ein GitHub-Workflow ist unter `.github/workflows/windows.yml` vorbereitet; ein tatsächlicher CI-Lauf erfolgt erst im veröffentlichten Repository. Projektregeln stehen in [AGENTS.md](AGENTS.md), Produktziele und Freigabekriterien in [MASTERPLAN.md](MASTERPLAN.md).
