using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace HandyMeeting;

public partial class NotesWindow : Window
{
    public sealed record EvidenceView(string Label, NoteEvidence Evidence);
    public sealed record FactView(string Text, string Metadata, List<EvidenceView> Evidence);
    private readonly string sessionFolder;
    private readonly TranscriptSnapshot snapshot;
    private readonly NotesSettings settings = NotesSettings.Load();
    private readonly TemplateLibrary library = TemplateLibrary.Load();
    private AnalysisTemplate active = AnalysisTemplate.Default();
    private bool templateSelection;
    private TextProcessingMode mode;
    private NotesResult? displayed;
    private CancellationTokenSource? cancellation;
    private bool populating = true, closeAfterStop, keyUnreadable;
    public NotesWindow(string sessionFolder, TranscriptSnapshot snapshot)
    {
        this.sessionFolder = sessionFolder; this.snapshot = snapshot;
        InitializeComponent(); MeetingTitle.Text = snapshot.Title + (snapshot.StartedAt == default ? "" : $" · {snapshot.StartedAt:dd.MM.yyyy HH:mm}") + " · Transkriptstand " + snapshot.Hash[..12];
        // Catch mouse/context-menu paste as well as typing in the editable model selector.
        ModelInput.AddHandler(System.Windows.Controls.Primitives.TextBoxBase.TextChangedEvent, new TextChangedEventHandler(ConnectionChanged));
        mode = TextProcessingMode.Local; LoadProfile(); populating = false;
        active = (library.Templates.FirstOrDefault(t => t.Id == library.SelectedId) ?? library.Templates[0]).Copy();
        RefreshTemplates(); BuildTabs(); RefreshVersions(); UpdateButtons(); Ui.Attach(this);
        if (displayed is null && VersionsCombo.SelectedItem is not NotesRun) SourceText.Text = Ui.T(SourceText.Text);
    }
    private NotesConnection Connection() => new(mode, EndpointInput.Text.Trim(), ModelInput.Text.Trim(), ApiKeyInput.Password);
    private void SaveProfile()
    {
        if (mode == TextProcessingMode.Local) { settings.LocalEndpoint = EndpointInput.Text.Trim(); settings.LocalModel = ModelInput.Text.Trim(); if (!keyUnreadable) settings.ProtectedLocalKey = NotesSettings.Protect(ApiKeyInput.Password); }
        else { settings.CloudEndpoint = EndpointInput.Text.Trim(); settings.CloudModel = ModelInput.Text.Trim(); if (!keyUnreadable) settings.ProtectedCloudKey = NotesSettings.Protect(ApiKeyInput.Password); }
        settings.Save();
    }
    private void LoadProfile()
    {
        populating = true;
        EndpointInput.Text = mode == TextProcessingMode.Local ? settings.LocalEndpoint : settings.CloudEndpoint;
        ModelInput.ItemsSource = null; ModelInput.Text = mode == TextProcessingMode.Local ? settings.LocalModel : settings.CloudModel;
        keyUnreadable = false;
        try { ApiKeyInput.Password = NotesSettings.Unprotect(mode == TextProcessingMode.Local ? settings.ProtectedLocalKey : settings.ProtectedCloudKey); }
        catch (Exception error) when (error is System.Security.Cryptography.CryptographicException or FormatException) { keyUnreadable = true; ApiKeyInput.Password = ""; ProgressText.Text = "Der gespeicherte API-Schlüssel ist für dieses Windows-Konto nicht lesbar. Bitte neu eingeben."; }
        CloudConsent.IsChecked = false; CloudConsent.Visibility = mode == TextProcessingMode.Cloud ? Visibility.Visible : Visibility.Collapsed;
        UpdatePrivacy();
        populating = false;
    }
    private void ModeChanged(object sender, SelectionChangedEventArgs e)
    {
        if (populating || EndpointInput is null) return;
        try { SaveProfile(); mode = ModeCombo.SelectedIndex == 0 ? TextProcessingMode.Local : TextProcessingMode.Cloud; LoadProfile(); UpdateButtons(); }
        catch (Exception error) { ProgressText.Text = error.Message; }
    }
    private void ConnectionChanged(object sender, TextChangedEventArgs e) { if (!populating && CloudConsent is not null) { CloudConsent.IsChecked = false; UpdateButtons(); } }
    private void KeyChanged(object sender, RoutedEventArgs e) { if (!populating && CloudConsent is not null) { keyUnreadable = false; CloudConsent.IsChecked = false; UpdateButtons(); } }
    private void ModelChanged(object sender, SelectionChangedEventArgs e) { if (!populating) { CloudConsent.IsChecked = false; UpdateButtons(); } }
    private void ModelTyped(object sender, KeyEventArgs e) { if (!populating) { CloudConsent.IsChecked = false; UpdateButtons(); } }
    private void ConsentChanged(object sender, RoutedEventArgs e) { if (!populating) UpdateButtons(); }
    private void UpdateButtons()
    {
        if (GenerateButton is null) return;
        var idle = cancellation is null;
        ConnectionPanel.IsEnabled = idle; VersionsCombo.IsEnabled = idle; CloudConsent.IsEnabled = idle; TemplatePanel.IsEnabled = idle; ResultTabs.IsEnabled = idle;
        var approved = mode == TextProcessingMode.Local || CloudConsent.IsChecked == true;
        GenerateButton.IsEnabled = idle && approved && !string.IsNullOrWhiteSpace(ModelInput.Text);
        ModelsButton.IsEnabled = idle && approved;
        ResumeButton.IsEnabled = false;
        if (idle && approved && VersionsCombo.SelectedItem is NotesRun run && run.Status != "complete" && run.SourceHash == snapshot.Hash)
        {
            try { ResumeButton.IsEnabled = run.ConnectionFingerprint == Connection().Fingerprint && (run.Template is null || run.TemplateHash == active.Hash); } catch (InvalidOperationException) { }
        }
        CancelButton.Visibility = idle ? Visibility.Collapsed : Visibility.Visible;
        CopyButton.IsEnabled = displayed is not null; FolderButton.IsEnabled = displayed is not null;
    }
    private async void CheckModels(object sender, RoutedEventArgs e)
    {
        if (cancellation is not null) return;
        cancellation = new(); UpdateButtons(); ProgressText.Text = "Textmodelle beim gewählten Dienst prüfen …";
        try
        {
            using var client = new TextModelClient(Connection(), CloudConsent.IsChecked == true);
            var models = await client.ModelsAsync(cancellation.Token);
            var previous = ModelInput.Text; populating = true; ModelInput.ItemsSource = models;
            ModelInput.Text = models.Contains(previous) ? previous : models.FirstOrDefault(m => m.Contains("gemma", StringComparison.OrdinalIgnoreCase)) ?? models.FirstOrDefault() ?? previous;
            populating = false; CloudConsent.IsChecked = false; SaveProfile(); ProgressText.Text = $"{models.Count} Textmodelle gefunden. Ein lokal installiertes Instruct-Modell mit JSON-Unterstützung wählen.";
        }
        catch (OperationCanceledException) { ProgressText.Text = "Modellprüfung pausiert."; }
        catch (Exception error) { ProgressText.Text = error.Message; }
        finally { populating = false; EndOperation(); }
    }
    private async void Generate(object sender, RoutedEventArgs e) => await GenerateAsync(null);
    private async void Resume(object sender, RoutedEventArgs e) { if (VersionsCombo.SelectedItem is NotesRun run) await GenerateAsync(run.Id); }
    private async Task GenerateAsync(string? id)
    {
        if (cancellation is not null) return;
        cancellation = new(); UpdateButtons();
        try
        {
            var connection = Connection(); _ = connection.BaseUri(); SaveProfile();
            var progress = new Progress<string>(message => ProgressText.Text = message);
            var legacy = id is not null && (VersionsCombo.SelectedItem as NotesRun)?.Template is null;
            var result = legacy
                ? await MeetingNotesService.GenerateAsync(sessionFolder, snapshot, connection, CloudConsent.IsChecked == true, id, progress, cancellation.Token)
                : await TemplateNotesService.GenerateAsync(sessionFolder, snapshot, active, connection, CloudConsent.IsChecked == true, id, progress, cancellation.Token);
            RefreshVersions(result.Run.Id); ShowResult(result); ProgressText.Text = "Auswertung lokal gespeichert. Die Quellenverweise öffnen die unveränderten Originalstellen.";
        }
        catch (OperationCanceledException) { ProgressText.Text = "Pausiert. Bereits geprüfte Abschnitte sind gespeichert und können fortgesetzt werden."; RefreshVersions(id); }
        catch (Exception error) { ProgressText.Text = error.Message; RefreshVersions(id); }
        finally { CloudConsent.IsChecked = false; EndOperation(); }
    }
    private void EndOperation()
    {
        cancellation?.Dispose(); cancellation = null; UpdateButtons(); if (closeAfterStop) Close();
    }
    private void Cancel(object sender, RoutedEventArgs e) => cancellation?.Cancel();
    private void RefreshVersions(string? selected = null)
    {
        var runs = MeetingNotesService.Runs(sessionFolder); VersionsCombo.ItemsSource = runs;
        VersionsCombo.SelectedItem = runs.FirstOrDefault(r => r.Id == selected) ?? runs.FirstOrDefault();
    }
    private void VersionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (VersionsCombo.SelectedItem is NotesRun run && run.Status == "complete")
        {
            try { ShowResult(MeetingNotesService.Load(sessionFolder, run.Id)); }
            catch (Exception error) { displayed = null; BuildTabs(); ProgressText.Text = "Gespeicherte Auswertung: " + error.Message; }
        }
        else if (VersionsCombo.SelectedItem is NotesRun partial)
        {
            displayed = null;
            if (partial.Template is not null) { active = partial.Template.Copy(); RefreshTemplates(); }
            BuildTabs();
            SourceHeading.Text = "Gesicherter Arbeitsstand"; SourceText.Text = Ui.English
                ? $"{partial.CompletedBatches} of {partial.Batches} text portions are validated and saved. Resume with the same template, prompts, provider and model."
                : $"{partial.CompletedBatches} von {partial.Batches} Textabschnitten sind geprüft und gespeichert. Mit denselben Vorlagen-, Prompt-, Anbieter- und Modelleinstellungen lässt sich die Auswertung fortsetzen.";
        }
        UpdateButtons();
    }
    internal void ShowResult(NotesResult result)
    {
        displayed = result;
        active = result.Run.Template?.Copy() ?? AnalysisTemplate.Default();
        RefreshTemplates(); BuildTabs();
        SourceText.Text = Ui.T("Einen Quellenverweis auswählen, um Originaltext und Zitat zu prüfen.");
        SourceHeading.Text = result.Original.Hash == snapshot.Hash ? "Originaltranskript dieser Sitzung" : "Originaltranskript einer früheren Fassung";
        UpdateButtons();
    }
    private void ShowEvidence(object sender, RoutedEventArgs e) { if (sender is Button { DataContext: EvidenceView view }) NavigateToEvidence(view.Evidence); }
    internal void NavigateToEvidence(NoteEvidence evidence)
    {
        var source = displayed?.Original.Sources.FirstOrDefault(s => s.Id == evidence.SourceId); if (source is null) return;
        var offset = source.Text.IndexOf(evidence.Quote, StringComparison.Ordinal); if (offset < 0) return;
        SourceHeading.Text = MeetingNotesService.SourceLabel(source); SourceText.Text = source.Text;
        SourceText.Focus(); SourceText.Select(offset, evidence.Quote.Length);
        var line = SourceText.GetLineIndexFromCharacterIndex(offset); if (line >= 0) SourceText.ScrollToLine(line);
    }
    internal string SelectedEvidenceForTest => SourceText.SelectedText;
    private void CopyNotes(object sender, RoutedEventArgs e)
    {
        if (displayed is null) return;
        try { Clipboard.SetText(TemplateNotesService.PlainText(displayed)); ProgressText.Text = "Alle Tabs mit Quellenverweisen und Originalzitaten kopiert."; }
        catch (Exception error) { ProgressText.Text = "Kopieren nicht möglich: " + error.Message; }
    }
    private void OpenResultFolder(object sender, RoutedEventArgs e)
    {
        if (displayed is not null) Process.Start(new ProcessStartInfo("explorer.exe") { UseShellExecute = true, ArgumentList = { displayed.Folder } });
    }
    private void WindowClosing(object? sender, CancelEventArgs e)
    {
        if (cancellation is not null) { e.Cancel = true; closeAfterStop = true; cancellation.Cancel(); }
    }
    private void RefreshTemplates()
    {
        templateSelection = true;
        var templates = library.Templates.ToList();
        var selected = templates.FirstOrDefault(t => t.Id == active.Id && t.Hash == active.Hash);
        if (selected is null) { selected = active; templates.Add(active); }
        TemplatesCombo.ItemsSource = templates; TemplatesCombo.SelectedItem = selected;
        templateSelection = false;
    }
    private void TemplateChanged(object sender, SelectionChangedEventArgs e)
    {
        if (templateSelection || TemplatesCombo.SelectedItem is not AnalysisTemplate selected || cancellation is not null) return;
        active = selected.Copy(); library.SelectedId = selected.Id;
        try { library.Save(); ResetResult(); }
        catch (Exception error) { ProgressText.Text = error.Message; }
    }
    private void ResetResult()
    {
        displayed = null; VersionsCombo.SelectedItem = null; CloudConsent.IsChecked = false; BuildTabs(); UpdateButtons();
        SourceText.Text = Ui.T("Einen Quellenverweis auswählen, um Originaltext und Zitat zu prüfen.");
        SourceHeading.Text = Ui.T("Vorlagenvorschau");
        ProgressText.Text = Ui.T("Vorlage bereit. Änderungen gelten für neue Auswertungen; gespeicherte Fassungen bleiben erhalten.");
    }
    private void SaveActive()
    {
        try
        {
            active.Validate();
            var index = library.Templates.FindIndex(t => t.Id == active.Id);
            // A historic run is immutable. Editing its definition forks a reusable template.
            if (displayed is not null || (VersionsCombo.SelectedItem as NotesRun)?.Template is not null)
            { active.Id = Guid.NewGuid().ToString("N"); active.Name += Ui.T(" · Kopie"); index = -1; }
            if (index < 0) library.Templates.Add(active.Copy()); else library.Templates[index] = active.Copy();
            library.SelectedId = active.Id; library.Save(); RefreshTemplates(); ResetResult();
        }
        catch (Exception error) { ProgressText.Text = Ui.T("Vorlage konnte nicht gespeichert werden: ") + error.Message; }
    }
    private void NewTemplate(object sender, RoutedEventArgs e)
    {
        var name = TemplateEditor.Name(this, "Neue Vorlage", ""); if (name is null) return;
        active = AnalysisTemplate.Default(); active.Name = name;
        displayed = null; VersionsCombo.SelectedItem = null; SaveActive();
    }
    private void DuplicateTemplate(object sender, RoutedEventArgs e)
    {
        var name = TemplateEditor.Name(this, "Vorlage duplizieren", active.Name + Ui.T(" · Kopie")); if (name is null) return;
        active = active.Copy(); active.Id = Guid.NewGuid().ToString("N"); active.Name = name;
        displayed = null; VersionsCombo.SelectedItem = null; SaveActive();
    }
    private void RenameTemplate(object sender, RoutedEventArgs e)
    {
        var name = TemplateEditor.Name(this, "Vorlage umbenennen", active.Name); if (name is null) return;
        active.Name = name; SaveActive();
    }
    private void RemoveTemplate(object sender, RoutedEventArgs e)
    {
        if (library.Templates.Count <= 1) { ProgressText.Text = Ui.T("Mindestens eine Vorlage bleibt erhalten."); return; }
        if (MessageBox.Show(this, Ui.T("Vorlage entfernen? Gespeicherte Auswertungen bleiben erhalten."), active.Name, MessageBoxButton.YesNo) != MessageBoxResult.Yes) return;
        try { library.Templates.RemoveAll(t => t.Id == active.Id); active = library.Templates[0].Copy(); library.SelectedId = active.Id; library.Save(); RefreshTemplates(); ResetResult(); }
        catch (Exception error) { ProgressText.Text = Ui.T("Vorlage konnte nicht gespeichert werden: ") + error.Message; }
    }
    private void BuildTabs()
    {
        UpdatePrivacy();
        var selected = ResultTabs.SelectedIndex;
        ResultTabs.Items.Clear();
        var sections = displayed?.AllSections;
        for (var i = 0; i < active.Steps.Count; i++)
        {
            var step = active.Steps[i]; var section = sections?.ElementAtOrDefault(i);
            var sources = displayed?.Original.Sources.ToDictionary(s => s.Id);
            var views = section?.Items.Select(f => new FactView(f.Text, section.Kind == AnalysisKind.Tasks ? $"{Ui.T("Zuständig")}: {f.Owner ?? Ui.T("nicht genannt")} · {Ui.T("Termin")}: {f.Due ?? Ui.T("nicht genannt")}" : "",
                f.Evidence.Select(e => new EvidenceView(MeetingNotesService.SourceLabel(sources![e.SourceId]), e)).ToList())).ToList();
            var items = new ItemsControl { Style = (Style)FindResource("NotesList"), ItemTemplate = (DataTemplate)FindResource("FactTemplate"), ItemsSource = views };
            var tab = new TabItem { Header = step.Name, ToolTip = step.Prompt, Content = new ScrollViewer { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = items } };
            var menu = new ContextMenu();
            void Command(string title, Action action)
            {
                var item = new MenuItem { Header = Ui.T(title), IsEnabled = cancellation is null };
                item.Click += (_, _) => { try { action(); } catch (Exception error) { ProgressText.Text = error.Message; } }; menu.Items.Add(item);
            }
            Command("Bearbeiten", () => EditStep(step));
            Command("Nach links", () => Move(step, -1)); Command("Nach rechts", () => Move(step, 1));
            Command("Tab entfernen", () => { if (active.Steps.Count <= 1) { ProgressText.Text = Ui.T("Mindestens ein Tab bleibt erhalten."); return; } active.Steps.Remove(step); SaveActive(); });
            tab.ContextMenu = menu; ResultTabs.Items.Add(tab);
        }
        ResultTabs.SelectedIndex = Math.Clamp(selected, 0, active.Steps.Count - 1);
    }
    private void AddStep(object sender, RoutedEventArgs e)
    {
        if (cancellation is not null) return;
        var step = TemplateEditor.Step(this, new AnalysisStep { Name = Ui.T("Neue Analyse") });
        if (step is not null) { active.Steps.Add(step); SaveActive(); ResultTabs.SelectedIndex = active.Steps.Count - 1; }
    }
    private void EditSelectedStep(object sender, RoutedEventArgs e)
    {
        if (cancellation is null && ResultTabs.SelectedIndex >= 0 && ResultTabs.SelectedIndex < active.Steps.Count) EditStep(active.Steps[ResultTabs.SelectedIndex]);
    }
    private void EditStep(AnalysisStep step)
    {
        var edited = TemplateEditor.Step(this, step);
        if (edited is not null) { active.Steps[active.Steps.IndexOf(step)] = edited; SaveActive(); }
    }
    private void Move(AnalysisStep step, int delta)
    {
        var index = active.Steps.IndexOf(step); var next = index + delta;
        if (next < 0 || next >= active.Steps.Count) return;
        (active.Steps[index], active.Steps[next]) = (active.Steps[next], active.Steps[index]); SaveActive(); ResultTabs.SelectedIndex = next;
    }
    private void UpdatePrivacy()
    {
        var portions = MeetingNotesService.Batches(snapshot).Count; var steps = active.Steps.Count;
        PrivacyText.Text = Ui.English
            ? $"{steps} steps × {portions} text portions = {steps * portions} initial requests; summaries may need further requests. " + (mode == TextProcessingMode.Local
                ? "Local services on this computer only. Choose locally executed models; no automatic cloud fallback."
                : "Cloud sends transcript text, prompts and source IDs only. Audio and local paths stay on this computer. The provider's data policies apply.")
            : $"{steps} Schritte × {portions} Textabschnitte = {steps * portions} erste Anfragen; Zusammenfassungen können weitere Anfragen benötigen. " + (mode == TextProcessingMode.Local
                ? "Nur Dienste auf diesem Computer. Ausschließlich lokal ausgeführte Modelle wählen; kein automatischer Cloud-Ersatz."
                : "Cloud überträgt nur Transkripttext, Prompts und Belegkennungen. Audio und lokale Pfade bleiben auf diesem Computer. Die Datenregeln des Anbieters gelten.");
    }
}
