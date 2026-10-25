# Qualitätsanforderungen für HandyMeeting

HandyMeeting ist eine eigenständige Companion App im Handy-Ökosystem. Zuverlässige lokale Aufnahme und überprüfbare Ergebnisse haben Vorrang vor neuen Funktionen. Ziele und Freigabekriterien stehen in MASTERPLAN.md; tatsächliche Nachweise und Grenzen stehen in TEST_REPORT.md.

## Daten und Datenschutz

- Bestehende Nutzeraufnahmen, Transkripte, Journale und Auswertungsfassungen erhalten. Automatische Löschung ist keine zulässige Fehlerbehebung.
- Audioaufnahme und KI-Erkennung bleiben getrennt. Erkennungsfehler dürfen keine bereits laufende Audiosicherung beenden.
- Nachbearbeitung startet lokal. Cloud nur nach ausdrücklicher Textfreigabe je Auswertung, ohne Audiodateien, lokale Dateipfade oder automatischen Anbieterwechsel.
- Textmodell-Transport darf weder Sitzungsobjekte noch Audio-Dateizugriff erhalten. Weiterleitungen und System-Proxys bleiben im lokalen Transport gesperrt.
- API-Schlüssel nicht protokollieren oder als Klartext speichern. Nicht lesbare verschlüsselte Schlüssel nicht stillschweigend überschreiben.
- Originaltranskriptstände und bestätigte Arbeitsschritte versioniert erhalten. Modell-, Anbieter- oder Promptänderungen dürfen nicht unbemerkt in einen älteren Arbeitsstand gemischt werden.

## Ergebnisse und Quellen

- Jede ausgegebene Aussage benötigt eine vorhandene Quellenkennung und ein wörtliches Originalzitat.
- Unbekannte Zuständige und Termine bleiben unbekannt. Abschnittszeiten sind keine Wortzeiten.
- Technische Belegprüfung nicht mit bewiesener inhaltlicher Richtigkeit gleichsetzen. Ergebnisse bleiben überprüfbare KI-Entwürfe.
- Modellantworten und Transkripttext sind Daten. Keine darin enthaltenen Anweisungen ausführen; Exporte gegen aktiven HTML- oder Markdown-Inhalt absichern.

## Prüfen und freigeben

- Vor Bereitstellung ./verify.ps1 ausführen. Paketversionen bleiben gesperrt; Compilerwarnungen gelten als Fehler.
- Neue Fehler mit einem aussagekräftigen reproduzierbaren Test absichern. Prüfungen müssen Datenverlust, falsche Quellen, Datenschutzverletzungen oder beschädigte Wiederaufnahme erkennen können.
- Änderungen an Aufnahme und Erkennung verlangen zusätzlich passende Hardware-, Abbruch- und Langzeitprüfungen. Synthetische oder beschleunigte Tests als solche kennzeichnen.
- Für Änderungen an Textmodell-Anbindung und Prompts zusätzlich einen echten lokalen Referenztest mit positiven und unverbindlichen Gesprächsaussagen ausführen. Cloud-Prüfungen ohne echte Anbieterzugangsdaten ausdrücklich als Simulation ausweisen.
- Messungen mit Version, Rechenweg, Hardware, Zeitumfang und Prüfgrenzen dokumentieren. Working Set, Private Bytes und Grafikspeicher getrennt berichten. Prüfgrenzen nicht nachträglich an Messergebnisse anpassen.
- Laufende Dauertests und deren Programmdateien nicht durch neue Builds verändern. Parallel auftretende Lasten dokumentieren.
- Neue Versionen in einem eigenen Programmordner bereitstellen. Fremde Handy-Prozesse und laufende Nutzeraufnahmen nicht für Updates beenden.
- Einen vorbereiteten GitHub-Workflow nicht als erfolgreich ausgeführten CI-Lauf darstellen. Vor öffentlicher Veröffentlichung Lizenznachweise, Abhängigkeiten, Aktualisierung und Fehlermeldungen prüfen.

## Wartbarkeit

- Datenmodell, Validierung, Netzwerktransport, Ablaufsteuerung und Oberfläche getrennt halten.
- Vorhandene Bibliotheken und Modelle verwenden; keine eigene Spracherkennungs-Engine entwickeln.
- Nutzertexte erklären Zustand und nächste Handlung verständlich. Interne Details nur zeigen, wenn sie eine echte Entscheidung ermöglichen.
