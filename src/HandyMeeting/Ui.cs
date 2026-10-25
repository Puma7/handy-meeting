using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Media;

namespace HandyMeeting;

public sealed class AppearanceSettings
{
    public bool English { get; set; }
    public bool Dark { get; set; }
}
public static class Ui
{
    private sealed class Translation(DependencyObject target, DependencyProperty property, string original)
    {
        public string Original = original;
        public bool Applying;
        public void Apply()
        {
            Applying = true;
            try { target.SetCurrentValue(property, T(Original)); } finally { Applying = false; }
        }
        public void Changed(object? sender, EventArgs e)
        {
            if (Applying) return;
            Original = target.GetValue(property) as string ?? ""; Apply();
        }
        public void Detach() => DependencyPropertyDescriptor.FromProperty(property, target.GetType()).RemoveValueChanged(target, Changed);
    }
    private static readonly ConditionalWeakTable<DependencyObject, List<Translation>> Tracked = new();
    private static readonly List<WeakReference<Window>> Windows = [];
    private static bool initialized;
    public static bool English { get; private set; }
    public static bool Dark { get; private set; }
    public static void Initialize()
    {
        if (initialized) return; initialized = true;
        try { var settings = JsonData.Read<AppearanceSettings>(Path.Combine(UserSettings.DataRoot, "appearance.json")); English = settings?.English == true; Dark = settings?.Dark == true; }
        catch (Exception e) when (e is IOException or System.Text.Json.JsonException or UnauthorizedAccessException) { }
        ApplyTheme();
    }
    public static void Change(bool english, bool dark)
    {
        English = english; Dark = dark; ApplyTheme();
        JsonData.Save(Path.Combine(UserSettings.DataRoot, "appearance.json"), new AppearanceSettings { English = English, Dark = Dark });
        foreach (var reference in Windows.ToList())
            if (reference.TryGetTarget(out var window)) { ApplyTranslations(window); SetTips(window); }
        Windows.RemoveAll(w => !w.TryGetTarget(out _));
    }
    public static string T(string text)
    {
        if (!English) return text;
        if (Words.TryGetValue(text, out var translated)) return translated;
        return TranslateStatus(text);
    }
    public static void Attach(Window window)
    {
        Initialize(); Windows.Add(new(window)); Scan(window); SetTips(window);
        window.AddHandler(FrameworkElement.LoadedEvent, new RoutedEventHandler((_, e) => { if (e.OriginalSource is DependencyObject item) Scan(item); }));
        window.Closed += (_, _) => Detach(window);
    }
    private static IEnumerable<DependencyObject> Children(DependencyObject root)
    {
        foreach (var child in LogicalTreeHelper.GetChildren(root)) if (child is DependencyObject item) yield return item;
    }
    private static void Scan(DependencyObject target)
    {
        if (!Tracked.TryGetValue(target, out _))
        {
            var entries = new List<Translation>(); Tracked.Add(target, entries);
            void Track(DependencyProperty property)
            {
                if (BindingOperations.IsDataBound(target, property) || target.GetValue(property) is not string original) return;
                var entry = new Translation(target, property, original); entries.Add(entry);
                DependencyPropertyDescriptor.FromProperty(property, target.GetType()).AddValueChanged(target, entry.Changed); entry.Apply();
            }
            if (target is TextBlock) Track(TextBlock.TextProperty);
            if (target is Button or CheckBox or ComboBoxItem) Track(ContentControl.ContentProperty);
            if (target is Window) Track(Window.TitleProperty);
            if (target is FrameworkElement element)
            {
                ToolTipService.SetInitialShowDelay(element, 650); ToolTipService.SetShowDuration(element, 30000); ToolTipService.SetShowOnDisabled(element, true);
                if (element.ToolTip is string && !Tips.ContainsKey(element.Name)) Track(FrameworkElement.ToolTipProperty);
                element.Unloaded += (_, _) => { if (Tracked.TryGetValue(target, out var old)) { foreach (var entry in old) entry.Detach(); Tracked.Remove(target); } };
            }
        }
        foreach (var child in Children(target)) Scan(child);
    }
    private static void Detach(DependencyObject root)
    {
        if (Tracked.TryGetValue(root, out var entries)) { foreach (var entry in entries) entry.Detach(); Tracked.Remove(root); }
        foreach (var child in Children(root)) Detach(child);
    }
    private static void ApplyTranslations(DependencyObject root)
    {
        if (Tracked.TryGetValue(root, out var entries)) foreach (var entry in entries) entry.Apply();
        foreach (var child in Children(root)) ApplyTranslations(child);
    }
    private static void SetTips(Window window)
    {
        foreach (var (name, tip) in Tips)
            if (window.FindName(name) is FrameworkElement item) item.ToolTip = English && EnglishTips.TryGetValue(name, out var english) ? english : tip;
        foreach (var name in new[] { "StatusText", "ProgressText", "SafetyText" })
            if (window.FindName(name) is TextBlock text) text.SetBinding(FrameworkElement.ToolTipProperty, new Binding("Text") { Source = text });
    }
    private static void ApplyTheme()
    {
        if (Application.Current is null) return;
        Application.Current.Resources["EditStepLabel"] = English ? "Edit instructions" : "Auftrag bearbeiten";
        Application.Current.Resources["EditStepTip"] = English ? "Changes the name and instructions of the selected section." : "Ändert Name und Arbeitsauftrag des ausgewählten Bereichs.";
        void Brush(object key, string light, string dark) => Application.Current.Resources[key] = new SolidColorBrush((Color)ColorConverter.ConvertFromString(Dark ? dark : light));
        Brush("Ink", "#172B25", "#E3ECE7"); Brush("Muted", "#61736B", "#A7BBB0");
        Brush("Canvas", "#F4F7F5", "#121B17"); Brush("Surface", "#FFFFFF", "#1C2922");
        Brush("Line", "#D5DED8", "#43584B"); Brush("Hover", "#E3EFE8", "#324E3D");
        Brush("Disabled", "#E7ECE9", "#27362E"); Brush("DisabledInk", "#67746D", "#92A69A");
        Brush(SystemColors.WindowBrushKey, "#FFFFFF", "#1C2922"); Brush(SystemColors.WindowTextBrushKey, "#172B25", "#E3ECE7");
        Brush(SystemColors.ControlBrushKey, "#F4F7F5", "#26392E"); Brush(SystemColors.ControlTextBrushKey, "#172B25", "#E3ECE7");
        Brush(SystemColors.HighlightBrushKey, "#256D50", "#387D59"); Brush(SystemColors.HighlightTextBrushKey, "#FFFFFF", "#FFFFFF");
        Brush(SystemColors.GrayTextBrushKey, "#67746D", "#92A69A");
    }
    private static readonly Dictionary<string, string> Tips = new()
    {
        ["StartButton"] = "Sichert Computerton und optional das Mikrofon getrennt. Die Erkennung läuft parallel auf diesem Computer.",
        ["StopButton"] = "Beendet die Aufnahme, schließt Audiodateien ab und lässt offene Erkennungsabschnitte nachlaufen.",
        ["MuteButton"] = "Schaltet nur die Mikrofonspur dieser App stumm. Die Stummschaltung in Zoom oder Teams ist unabhängig davon.",
        ["CancelButton"] = "Pausiert nur die KI-Verarbeitung und sichert den Arbeitsstand. Eine laufende Audioaufnahme geht weiter.",
        ["RetryButton"] = "Setzt die lokale Erkennung aus dem gesicherten Audio fort. Bereits vorhandener Text bleibt erhalten.",
        ["CopyTranscriptButton"] = "Kopiert das gesamte bisherige Transkript beider Spuren, einschließlich Live-Text und früherer Abschnitte. Laufender Text kann sich noch ändern.",
        ["HistoryOpenButton"] = "Öffnet die ausgewählte gespeicherte Sitzung mit Transkript und Wiederherstellungsstatus.",
        ["HistoryRefreshButton"] = "Liest die Liste gespeicherter Sitzungen erneut ein.",
        ["OpenSessionButton"] = "Wählt eine session.json aus einer bestehenden HandyMeeting-Sitzung aus.",
        ["NotesButton"] = "Erstellt überprüfbare KI-Entwürfe aus dem abgeschlossenen Transkript. Lokal als Standard, Cloud nur mit Textfreigabe.",
        ["FolderButton"] = "Öffnet den lokalen Ordner der Sitzung oder Auswertung im Explorer.",
        ["BrowseButton"] = "Wählt die installierte handy.exe aus, um deren verfügbare Streaming-Modelle zu lesen.",
        ["ReloadButton"] = "Prüft die bereits in Handy installierten Streaming-Modelle. Lädt keine Modelle herunter.",
        ["MicCombo"] = "Zusätzliches Mikrofon. Mit Kopfhörern vermeidest du doppelte Stimmen durch Lautsprecher-Echo.",
        ["LanguageCombo"] = "Sprache der Oberfläche. Das Originaltranskript und die Ausgabesprache deiner Prompts bleiben unverändert.",
        ["ThemeCombo"] = "Helles oder dunkles Erscheinungsbild für alle Fenster. Die Auswahl wird lokal gespeichert.",
        ["TemplatesCombo"] = "Wiederverwendbare Tabs und Prompts. Rechtsklick auf einen Tab zum Bearbeiten; Plus fügt einen Schritt hinzu.",
        ["VersionsCombo"] = "Gespeicherte Auswertungen mit ihrem ursprünglichen Transkript und ihrer ursprünglichen Vorlage.",
        ["GenerateButton"] = "Wertet das Original mit allen Schritten der Vorlage aus. Jeder Schritt wird gesichert; ältere Fassungen bleiben erhalten.",
        ["ResumeButton"] = "Verwendet gesicherte Schritte erneut. Vorlage, Prompts, Anbieter, Modell und Transkript müssen übereinstimmen.",
        ["CopyButton"] = "Kopiert alle Ergebnis-Tabs in ihrer Reihenfolge als lesbaren Klartext mit Quellenkennungen und wörtlichen Zitaten.",
        ["ModelsButton"] = "Fragt die Modellliste beim gewählten Dienst ab. Lokal sind nur Adressen dieses Computers erlaubt.",
        ["EndpointInput"] = "OpenAI-kompatible Text-API. Lokal ausschließlich Loopback; Cloud ausschließlich HTTPS und mit ausdrücklicher Textfreigabe.",
        ["ApiKeyInput"] = "Optionaler Schlüssel für diesen Dienst. Wird mit deinem Windows-Konto verschlüsselt gespeichert.",
        ["ResultTabs"] = "Rechtsklick: Prompt bearbeiten, Reihenfolge ändern oder Tab entfernen. Plus: eigenen Verarbeitungsschritt hinzufügen."
    };
    private static readonly Dictionary<string, string> EnglishTips = new()
    {
        ["StartButton"] = "Saves system audio and the optional microphone separately. Recognition runs locally in parallel.",
        ["StopButton"] = "Stops recording, closes audio files and lets recognition catch up with pending sections.",
        ["MuteButton"] = "Mutes only this app's microphone track. Zoom and Teams mute controls are independent.",
        ["CancelButton"] = "Pauses AI processing and saves progress. An active audio recording continues.",
        ["RetryButton"] = "Resumes local recognition from saved audio. Existing text is retained.",
        ["CopyTranscriptButton"] = "Copies the entire transcript of both tracks, including current live text and earlier sections. Live text may still change.",
        ["HistoryOpenButton"] = "Opens the selected saved session with its transcript and recovery status.",
        ["HistoryRefreshButton"] = "Refreshes the list of saved sessions.",
        ["OpenSessionButton"] = "Selects a session.json file from an existing HandyMeeting session.",
        ["NotesButton"] = "Creates reviewable AI drafts from the completed transcript. Local by default; cloud requires text consent.",
        ["FolderButton"] = "Opens the local session or analysis folder in Explorer.",
        ["BrowseButton"] = "Selects the installed handy.exe to read its available streaming models.",
        ["ReloadButton"] = "Checks streaming models already installed in Handy. Does not download models.",
        ["MicCombo"] = "Additional microphone. Use headphones to avoid duplicate voices from speaker echo.",
        ["LanguageCombo"] = "Interface language. Does not change the original transcript or the output language specified in your prompts.",
        ["ThemeCombo"] = "Light or dark appearance for all windows. Saved locally.",
        ["TemplatesCombo"] = "Reusable tabs and prompts. Right-click a tab to edit it; Plus adds a step.",
        ["VersionsCombo"] = "Saved analyses with their original transcript and template.",
        ["GenerateButton"] = "Processes the original with every template step. Each step is saved; earlier versions are retained.",
        ["ResumeButton"] = "Reuses saved steps. Template, prompts, provider, model and transcript must match.",
        ["CopyButton"] = "Copies every result tab in order as readable plain text with source IDs and verbatim quotes.",
        ["ModelsButton"] = "Requests the model list from the selected service. Local mode only permits addresses on this computer.",
        ["EndpointInput"] = "OpenAI-compatible text API. Loopback only in local mode; HTTPS and explicit text consent in cloud mode.",
        ["ApiKeyInput"] = "Optional key for this service. Stored encrypted using your Windows account.",
        ["ResultTabs"] = "Right-click to edit a prompt, reorder or remove a tab. Plus adds a custom processing step."
    };
    private static string TranslateStatus(string text)
    {
        foreach (var (pattern, replacement) in StatusPatterns)
            if (System.Text.RegularExpressions.Regex.IsMatch(text, pattern)) return System.Text.RegularExpressions.Regex.Replace(text, pattern, replacement);
        // Runtime diagnostics may include system/driver messages. Preserve those details verbatim.
        foreach (var (de, en) in StatusPrefixes)
            if (text.StartsWith(de, StringComparison.Ordinal)) return en + text[de.Length..];
        return text;
    }
    private static readonly (string Pattern, string Replacement)[] StatusPatterns =
    [
        (@"^Installierte Streaming-Modelle: (\d+)\. Audio und Live-Text werden parallel lokal gesichert\.$", "Installed streaming models: $1. Audio and live text are saved locally in parallel."),
        (@"^Audio auf Datenträger bis (.+) · Textstände im lokalen Journal$", "Audio saved to disk through $1 · Text checkpoints in the local journal"),
        (@"^Audio bis (.+) gespeichert · Originale bleiben erhalten$", "Audio saved through $1 · Originals retained"),
        (@"^Streaming · (.+) · Audio gesichert bis (.+) · Rückstand (.+)$", "Streaming · $1 · Audio saved through $2 · Backlog $3"),
        (@"^(\d+) Textmodelle gefunden\. Ein lokal installiertes Instruct-Modell mit JSON-Unterstützung wählen\.$", "$1 text models found. Choose a locally installed instruct model with JSON support."),
        (@"^Lokal: (\d+) Textabschnitte, danach gegebenenfalls eine Gesamtzusammenfassung\..*$", "Local: $1 text portions per step, then summary condensation if needed. Only services on this computer are allowed. Choose locally executed models in the local service. No automatic cloud fallback."),
        (@"^Cloud: (\d+) Textabschnitte, danach gegebenenfalls weitere Textanfragen\..*$", "Cloud: $1 text portions per step, with possible further text requests. Only transcript text, prompts and source IDs are sent. No audio or local file paths are sent. The provider's data policies apply."),
        (@"^(.*) · Transkriptstand ([A-F0-9]+)$", "$1 · Transcript version $2"),
        (@"^Version (.+) · lokal$", "Version $1 · local"),
        (@"^([0-9:–]+) · Computerton · (S\d+)$", "$1 · System audio · $2"),
        (@"^([0-9:–]+) · Mikrofon · (S\d+)$", "$1 · Microphone · $2")
    ];
    private static readonly Dictionary<string, string> StatusPrefixes = new()
    {
        ["Kopieren nicht möglich: "] = "Could not copy: ", ["Nachbearbeitung: "] = "Post-processing: ", ["Gespeicherte Auswertung: "] = "Saved analysis: ",
        ["Handy-Verbindung: "] = "Handy connection: ", ["Audiogeräte konnten nicht geladen werden: "] = "Could not load audio devices: ",
        ["Aufnahme konnte nicht starten: "] = "Could not start recording: ", ["Offene Abschnitte: "] = "Pending sections: ",
        ["Sitzung konnte nicht geöffnet werden (eventuell noch aktiv): "] = "Could not open session (it may still be recording): ",
        ["Originaltranskript dieser Sitzung"] = "Original transcript of this session", ["Originaltranskript einer früheren Fassung"] = "Original transcript of an earlier version",
        ["Lokal: "] = "Local processing: ", ["Cloud: "] = "Cloud processing: ",
        ["Einstellungen konnten nicht gespeichert werden: "] = "Could not save settings: ", ["Vorlage konnte nicht gespeichert werden: "] = "Could not save template: "
    };
    private static readonly Dictionary<string, string> Words = new()
    {
        ["HandyMeeting · Lokales Streaming"] = "HandyMeeting · Local streaming", ["HandyMeeting · Nachbearbeitung"] = "HandyMeeting · Post-processing",
        ["Audio sichern. Während des Meetings lokal mitschreiben."] = "Save audio. Transcribe locally while you meet.",
        ["Dein Meeting erscheint hier abschnittsweise als Computerton und optional als Mikrofonspur.\n\nFür deine eigene Stimme aktiviere das Mikrofon. Die Stummschaltung in Teams oder Zoom gilt nicht für diese App. Mit Kopfhörern vermeidest du doppelte Stimmen durch Lautsprecher-Echo."] = "Your meeting appears here in sections as system audio and an optional microphone track.\n\nEnable the microphone for your own voice. Muting in Teams or Zoom does not mute this app. Use headphones to avoid duplicate voices from speaker echo.",
        ["Aufnahme"] = "Recording", ["NAME DES MEETINGS"] = "MEETING NAME", ["COMPUTERTON · AUDIOAUSGABE DES MEETINGS"] = "SYSTEM AUDIO · MEETING OUTPUT",
        ["Mein Mikrofon zusätzlich aufnehmen"] = "Also record my microphone", ["Lokale Transkription"] = "Local transcription", ["INSTALLIERTES HANDY"] = "INSTALLED HANDY",
        ["INSTALLIERTES STREAMING-MODELL"] = "INSTALLED STREAMING MODEL", ["Auswählen"] = "Browse", ["Prüfen"] = "Check", ["Audiogeräte aktualisieren"] = "Refresh audio devices",
        ["Das Modell bleibt geladen. Audio und Live-Text werden parallel lokal gesichert. Ein Streaming-Fehler beendet die Audioaufnahme nicht."] = "The model stays loaded. Audio and live text are saved locally in parallel. A streaming error does not stop recording.",
        ["Aufnahme starten"] = "Start recording", ["Meeting beenden"] = "End meeting", ["Mikrofon stummschalten"] = "Mute microphone", ["Mikrofon wieder aufnehmen"] = "Unmute microphone",
        ["Transkription abbrechen"] = "Pause transcription", ["Streaming pausieren"] = "Pause streaming", ["Nachlauf pausieren"] = "Pause catch-up", ["Streaming fortsetzen"] = "Resume streaming",
        ["Live-Transkript"] = "Live transcript", ["Transkript kopieren"] = "Copy transcript", ["Erscheint während der Aufnahme"] = "Appears while recording",
        ["Historie"] = "History", ["Aktualisieren"] = "Refresh", ["Anzeigen"] = "Show", ["Sitzungsdatei öffnen"] = "Open session file", ["Offene Abschnitte nachholen"] = "Transcribe pending audio",
        ["Nachbearbeitung"] = "Post-processing", ["Ordner öffnen"] = "Open folder", ["Bereit · lokal"] = "Ready · local", ["Wird geprüft"] = "Checking", ["Wird eingerichtet …"] = "Setting up …",
        ["Aufnahme + Live-Text"] = "Recording + live text", ["Gesicherte Abschnitte nachholen"] = "Transcribing saved audio", ["Audio wird abgeschlossen"] = "Finalizing audio", ["Aufnahme startet"] = "Starting recording",
        ["Handy und Audiogeräte werden geprüft …"] = "Checking Handy and audio devices …", ["Originalaudio bleibt erhalten. Gespeicherte Textstände können aus dem Journal wiederhergestellt werden."] = "Original audio is retained. Saved text checkpoints can be recovered from the journal.",
        ["Installierte Streaming-Modelle in Handy werden gelesen …"] = "Reading streaming models installed in Handy …", ["Bitte in Handy ein Streaming-Modell installieren und anschließend erneut prüfen."] = "Install a streaming model in Handy, then check again.",
        ["Bitte ein Mikrofon auswählen oder die Mikrofonaufnahme ausschalten."] = "Select a microphone or disable microphone recording.",
        ["Vollständig lokal gespeichert"] = "Fully saved locally", ["Lokal gesichert · Fortsetzen möglich"] = "Saved locally · Can resume", ["Fortsetzen möglich"] = "Can resume", ["Gespeicherte Sitzung"] = "Saved session",
        ["Live-Ausschnitt · vollständiger Verlauf gespeichert"] = "Live excerpt · Full history saved", ["Originalaudio und alle gespeicherten Textstände bleiben lokal erhalten."] = "Original audio and all saved transcript versions remain on this computer.",
        ["Sitzung aus der Historie geöffnet."] = "Session opened from history.", ["Gesicherte Sitzung geöffnet. Offene Abschnitte können nachgeholt werden."] = "Saved session opened. Pending audio can be transcribed.",
        ["Streaming pausiert. Audio wird weiter lokal gesichert."] = "Streaming paused. Audio recording continues locally.", ["Nachlauf pausiert. Gesicherte Daten können über die Historie fortgesetzt werden."] = "Catch-up paused. Resume saved sessions from History.",
        ["Gesamtes Transkript kopiert · einschließlich bisherigem Live-Text."] = "Entire transcript copied · Including current live text.",
        ["Nachbearbeitung mit Originalbelegen"] = "Post-processing with original evidence", ["VERARBEITUNG"] = "PROCESSING", ["API-ADRESSE"] = "API ADDRESS", ["API-SCHLÜSSEL · OPTIONAL LOKAL"] = "API KEY · OPTIONAL LOCALLY",
        ["Streaming wird aus dem gesicherten Audio wieder aufgenommen."] = "Resuming streaming from saved audio.",
        ["Streaming-Modell wird einmalig geladen. Audio ist gesichert."] = "Loading the streaming model once. Audio is saved.",
        ["Streaming-Prozess startet neu; gesicherte Abschnitte werden nachgeholt."] = "Restarting the streaming process; catching up on saved audio.",
        ["Fertig. Audio, Textstände und Transkript sind lokal gespeichert."] = "Done. Audio, text checkpoints and transcript are saved locally.",
        ["Streaming pausiert. Audio und gespeicherte Textstände bleiben erhalten."] = "Streaming paused. Audio and saved text checkpoints are retained.",
        ["Der gespeicherte API-Schlüssel ist für dieses Windows-Konto nicht lesbar. Bitte neu eingeben."] = "This Windows account cannot read the saved API key. Please enter it again.",
        ["Die Originalstelle erscheint hier. Das belegte Zitat wird hervorgehoben. Zeitangaben beziehen sich auf Aufnahmeabschnitte, nicht auf einzelne Wörter."] = "Original passages appear here with the cited quote highlighted. Times refer to recording sections, not individual words.",
        ["Dieselbe Audioausgabe wählen, die Zoom oder Teams verwendet."] = "Choose the same audio output as Zoom or Teams.",
        ["Lokal · Standard"] = "Local · Default", ["Cloud · nur Text"] = "Cloud · Text only", ["TEXTMODELL"] = "TEXT MODEL", ["Modelle prüfen"] = "Check models",
        ["Lokal z. B. LM Studio oder Ollama. Das Textmodell wird getrennt vom Handy-Sprachmodell ausgewählt."] = "Use LM Studio or Ollama locally. Select a text model separately from Handy's speech model.",
        ["Ich erlaube für diese Auswertung die Übertragung des Transkripttexts an den gewählten Cloud-Anbieter. Es können Kosten entstehen."] = "For this analysis, I allow transcript text to be sent to the selected cloud provider. Charges may apply.",
        ["Neue Auswertung erstellen"] = "Create new analysis", ["Gesicherten Stand fortsetzen"] = "Resume saved progress", ["Pausieren und sichern"] = "Pause and save",
        ["Originaltranskript bleibt unverändert. Jede Auswertung wird als eigene Fassung gespeichert."] = "The original transcript stays unchanged. Each analysis is saved as a separate version.",
        ["Gespeicherte Auswertungen"] = "Saved analyses", ["Zusammenfassung"] = "Summary", ["Entscheidungen"] = "Decisions", ["To-dos"] = "To-dos", ["Originalstelle"] = "Original passage",
        ["Einen Quellenverweis links auswählen."] = "Select a source reference on the left.", ["Ergebnisse kopieren"] = "Copy all results", ["Ergebnisordner öffnen"] = "Open results folder",
        ["KI-Entwurf · Quellenkennungen und wörtliche Zitate technisch geprüft. Inhaltliche Deutung bitte prüfen."] = "AI draft · Source IDs and verbatim quotes checked. Please review the interpretation.",
        ["Keine belegten Einträge in dieser Fassung."] = "No supported entries in this version.", ["Keine belegten Einträge."] = "No supported entries.", ["KI-Entwurf · Inhalt und Belege prüfen."] = "AI draft · Review content and evidence.",
        ["Vorlage"] = "Template", ["Neue Vorlage"] = "New template", ["Duplizieren"] = "Duplicate", ["Umbenennen"] = "Rename", ["Entfernen"] = "Remove", ["Vorlage duplizieren"] = "Duplicate template", ["Vorlage umbenennen"] = "Rename template",
        ["Vorlagenvorschau"] = "Template preview",
        ["Auftrag bearbeiten"] = "Edit instructions", ["Ändert Name und Arbeitsauftrag des ausgewählten Bereichs."] = "Changes the name and instructions of the selected section.",
        ["Eigene Auswertung"] = "Custom analysis", ["Kurzer Überblick"] = "Brief overview", ["Beschlossene Entscheidungen"] = "Agreed decisions", ["Name des Bereichs"] = "Section name",
        ["Wie soll das Ergebnis aussehen?"] = "What should the output look like?", ["Was soll die KI aus dem Gespräch herausarbeiten?"] = "What should the AI extract from the conversation?",
        ["Beschreibe Thema und Ausgabesprache. Beispiel: Nenne auf Englisch die angesprochenen Risiken und offenen Fragen. Ein Prompt ist der Arbeitsauftrag für diesen Bereich."] = "Describe the topic and output language. Example: List the risks and open questions in English. A prompt is the instruction for this section.",
        ["Verdichtet die belegten Themen zu einem kurzen Gesamtüberblick. Zuständige und Termine gehören in Aufgaben."] = "Condenses supported topics into a brief overview. Owners and deadlines belong in tasks.",
        ["Sucht ausdrücklich getroffene Beschlüsse. Wünsche, Vorschläge und Zustandsbeschreibungen gelten nicht als Entscheidung."] = "Finds explicit decisions. Wishes, proposals and descriptions of existing states are excluded.",
        ["Sucht ausdrücklich zugesagte oder zugewiesene Aufgaben. Zuständige und Termine bleiben unbekannt, wenn sie nicht genannt wurden."] = "Finds explicitly accepted or assigned tasks. Owners and deadlines stay unknown when not stated.",
        ["Für eigene Themen wie Risiken, offene Fragen oder Fachbegriffe. Jeder Punkt braucht ein wörtliches Originalzitat. Audiospuren erlauben noch keine genaue Personenstatistik."] = "For topics such as risks, open questions or technical terms. Every point requires a verbatim quote. Audio tracks alone do not support exact speaker statistics.",
        ["Liest angeschlossene Audioausgaben und Mikrofone erneut ein."] = "Refreshes connected audio outputs and microphones.",
        ["Legt eine benannte Vorlage mit drei anpassbaren Startschritten an."] = "Creates a named template with three customizable initial steps.",
        ["Kopiert die aktuellen Tabs und Prompts in eine eigene Vorlage."] = "Copies the current tabs and prompts into a separate template.",
        ["Ändert den Vorlagennamen. Gespeicherte Auswertungen behalten ihren ursprünglichen Stand."] = "Changes the template name. Saved analyses retain their original version.",
        ["Entfernt diese Vorlage aus der Auswahl. Gespeicherte Auswertungen werden erhalten."] = "Removes this template from the list. Saved analyses are retained.",
        ["Verarbeitungsschritt bearbeiten"] = "Edit processing step", ["Bearbeiten"] = "Edit", ["Nach links"] = "Move left", ["Nach rechts"] = "Move right", ["Tab entfernen"] = "Remove tab",
        ["Eigenen Verarbeitungsschritt hinzufügen"] = "Add a custom processing step", ["Neue Analyse"] = "New analysis", ["Name des Tabs"] = "Tab name", ["Ergebnisart"] = "Result type", ["Freie Analyse"] = "Custom analysis", ["Aufgaben mit Zuständigen und Terminen"] = "Tasks with owners and deadlines",
        ["Prompt · Thema, Sprache und gewünschte Auswertung"] = "Prompt · Topic, language and desired analysis", ["Speichern"] = "Save", ["Abbrechen"] = "Cancel", [" · Kopie"] = " · Copy", ["Zuständig"] = "Owner", ["Termin"] = "Due", ["nicht genannt"] = "Not specified",
        ["Vorlage bereit. Änderungen gelten für neue Auswertungen; gespeicherte Fassungen bleiben erhalten."] = "Template ready. Changes apply to new analyses; saved versions are retained.",
        ["Mindestens eine Vorlage bleibt erhalten."] = "Keep at least one template.", ["Mindestens ein Tab bleibt erhalten."] = "Keep at least one tab.", ["Vorlage entfernen? Gespeicherte Auswertungen bleiben erhalten."] = "Remove this template? Saved analyses are retained.",
        ["Alle Tabs mit Quellenverweisen und Originalzitaten kopiert."] = "All tabs copied with source references and original quotes.",
        ["Auswertung lokal gespeichert. Die Quellenverweise öffnen die unveränderten Originalstellen."] = "Analysis saved locally. Source references open the unchanged original passages.",
        ["Pausiert. Bereits geprüfte Abschnitte sind gespeichert und können fortgesetzt werden."] = "Paused. Validated sections are saved and can be resumed.", ["Textmodelle beim gewählten Dienst prüfen …"] = "Checking text models at the selected service …", ["Modellprüfung pausiert."] = "Model check paused.",
        ["Einen Quellenverweis auswählen, um Originaltext und Zitat zu prüfen."] = "Select a source reference to review the original text and quote.", ["Gesicherter Arbeitsstand"] = "Saved progress",
        ["Quellen und wörtliche Zitate bleiben Pflicht. Jeder Schritt erhält Textabschnitte des Originals. Audiospuren sind keine Personen; genaue Sprecherstatistiken sind ohne Sprecherzuordnung nicht möglich."] = "Sources and verbatim quotes remain mandatory. Each step receives portions of the original. Audio tracks are not people; exact speaker statistics require speaker attribution."
    };
}
