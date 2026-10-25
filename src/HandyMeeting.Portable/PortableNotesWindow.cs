using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;

namespace HandyMeeting;

public sealed class PortableNotesWindow : Window
{
    private readonly string folder;
    private readonly TranscriptSnapshot snapshot;
    private readonly TemplateLibrary library = TemplateLibrary.Load();
    private readonly NotesSettings settings = NotesSettings.Load();
    private readonly ComboBox templates = new(), versions = new(), mode = new() { ItemsSource = new[] { "Lokal / Local", "Cloud · nur Text / Text only" }, SelectedIndex = 0 };
    private readonly TextBox endpoint = new(), model = new(), key = new() { PasswordChar = '●', Watermark = "Schlüssel nur für diese Sitzung / Session-only key" };
    private readonly CheckBox consent = new() { Content = "Ich erlaube nur für diesen Lauf die Textübertragung an den gewählten Cloud-Anbieter. / I authorize text transfer for this run." };
    private readonly TabControl tabs = new();
    private readonly TextBlock status = new() { TextWrapping = TextWrapping.Wrap };
    private readonly TextBox source = new() { IsReadOnly = true, AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 100 };
    private readonly StackPanel controls = new() { Spacing = 8 };
    private AnalysisTemplate active;
    private NotesResult? result;
    private CancellationTokenSource? cancellation;
    private bool updating;
    public PortableNotesWindow(string folder, TranscriptSnapshot snapshot)
    {
        this.folder = folder; this.snapshot = snapshot; active = library.Templates[0].Copy();
        Title = "HandyMeeting · Nachbearbeitung / Analysis"; Width = 1100; Height = 850; MinWidth = 800; MinHeight = 700;
        var root = new Grid { Margin = new(24), RowDefinitions = new("Auto,Auto,*,Auto,Auto"), RowSpacing = 12 };
        root.Children.Add(new TextBlock { Text = "Was möchtest du aus dem Meeting mitnehmen?", FontSize = 24, FontWeight = FontWeight.SemiBold });
        mode.SelectionChanged += (_, _) => LoadConnection();
        controls.Children.Add(mode); controls.Children.Add(endpoint); controls.Children.Add(model); controls.Children.Add(key); controls.Children.Add(consent);
        foreach (var field in new[] { endpoint, model, key }) field.TextChanged += (_, _) => consent.IsChecked = false;
        LoadConnection();
        controls.Children.Add(new TextBlock { Text = "Vorlage / Template" }); controls.Children.Add(templates); templates.ItemsSource = library.Templates; templates.SelectedIndex = 0;
        templates.SelectionChanged += (_, _) => { if (!updating && templates.SelectedItem is AnalysisTemplate template) { active = template.Copy(); result = null; BuildTabs(); } };
        var actions = new WrapPanel();
        actions.Children.Add(Button("Vorlage kopieren / Duplicate", async () =>
        {
            var name = await PromptAsync("Name der Vorlage / Template name", active.Name + " · Copy"); if (name is null) return;
            active = active.Copy(); active.Id = Guid.NewGuid().ToString("N"); active.Name = name; library.Templates.Add(active.Copy()); library.Save(); RefreshTemplates();
        }));
        actions.Children.Add(Button("+ Schritt / Step", async () => { var step = await EditAsync(new()); if (step is null) return; active.Steps.Add(step); SaveTemplate(); }));
        actions.Children.Add(Button("Auswerten / Analyze", async () => await GenerateAsync(null)));
        actions.Children.Add(Button("Fortsetzen / Resume", async () => { if (versions.SelectedItem is NotesRun run) await GenerateAsync(run.Id); }));
        actions.Children.Add(Button("Pausieren / Pause", () => { cancellation?.Cancel(); return Task.CompletedTask; }, true));
        controls.Children.Add(actions); controls.Children.Add(new TextBlock { Text = "API-Schlüssel bleiben in dieser Vorschau nur im Arbeitsspeicher. / Preview: API keys are kept in memory only.", FontSize = 12 });
        controls.Children.Add(versions); versions.ItemsSource = MeetingNotesService.Runs(folder);
        versions.SelectionChanged += (_, _) =>
        {
            if (updating || versions.SelectedItem is not NotesRun run) return;
            try
            {
                if (run.Status == "complete") { result = MeetingNotesService.Load(folder, run.Id); active = result.Run.Template?.Copy() ?? AnalysisTemplate.Default(); BuildTabs(); }
                else { result = null; if (run.Template is not null) active = run.Template.Copy(); BuildTabs(); status.Text = "Gesicherter Arbeitsstand / Saved progress"; }
            }
            catch (Exception e) { status.Text = e.Message; }
        };
        Grid.SetRow(controls, 1); root.Children.Add(controls);
        Grid.SetRow(tabs, 2); root.Children.Add(tabs);
        Grid.SetRow(source, 3); root.Children.Add(source);
        var footer = new StackPanel { Spacing = 8 }; footer.Children.Add(status); footer.Children.Add(Button("Alle Ergebnisse kopieren / Copy all", async () => { if (result is not null && Clipboard is { } clipboard) await clipboard.SetTextAsync(TemplateNotesService.PlainText(result)); }));
        Grid.SetRow(footer, 4); root.Children.Add(footer); Content = root; BuildTabs();
        Closing += (_, e) => { if (cancellation is not null) { e.Cancel = true; cancellation.Cancel(); status.Text = "Gesicherte Schritte bleiben erhalten. / Checkpoints are retained."; } };
    }
    private void LoadConnection()
    {
        endpoint.Text = mode.SelectedIndex == 0 ? settings.LocalEndpoint : settings.CloudEndpoint; model.Text = mode.SelectedIndex == 0 ? settings.LocalModel : settings.CloudModel;
        key.Text = ""; consent.IsChecked = false; consent.IsVisible = mode.SelectedIndex == 1;
    }
    private void RefreshTemplates()
    {
        updating = true; templates.ItemsSource = library.Templates.ToList(); templates.SelectedItem = library.Templates.FirstOrDefault(t => t.Id == active.Id); updating = false; result = null; BuildTabs();
    }
    private void SaveTemplate()
    {
        if (result is not null) { active.Id = Guid.NewGuid().ToString("N"); active.Name += " · Copy"; }
        var index = library.Templates.FindIndex(t => t.Id == active.Id);
        if (index >= 0) library.Templates[index] = active.Copy(); else library.Templates.Add(active.Copy()); library.Save(); RefreshTemplates();
    }
    private void BuildTabs()
    {
        tabs.Items.Clear();
        for (var i = 0; i < active.Steps.Count; i++)
        {
            var step = active.Steps[i]; var panel = new StackPanel { Spacing = 10, Margin = new(12) };
            panel.Children.Add(Button("Auftrag bearbeiten / Edit instructions", async () => { var edited = await EditAsync(step); if (edited is null) return; active.Steps[active.Steps.IndexOf(step)] = edited; SaveTemplate(); }));
            foreach (var fact in result?.AllSections.ElementAtOrDefault(i)?.Items ?? [])
            {
                panel.Children.Add(new TextBlock { Text = fact.Text + (step.Kind == AnalysisKind.Tasks ? $" · {fact.Owner ?? "?"} · {fact.Due ?? "?"}" : ""), TextWrapping = TextWrapping.Wrap });
                foreach (var evidence in fact.Evidence)
                    panel.Children.Add(Button("↗ " + evidence.SourceId, () =>
                    {
                        var original = result!.Original.Sources.Single(s => s.Id == evidence.SourceId); source.Text = original.Text;
                        var at = original.Text.IndexOf(evidence.Quote, StringComparison.Ordinal); source.SelectionStart = at; source.SelectionEnd = at + evidence.Quote.Length; return Task.CompletedTask;
                    }));
            }
            tabs.Items.Add(new TabItem { Header = step.Name, Content = new ScrollViewer { Content = panel } });
        }
        tabs.SelectedIndex = 0;
    }
    private Button Button(string label, Func<Task> action, bool duringWork = false)
    {
        var button = new Button { Content = label, Margin = new(0, 0, 8, 4), Padding = new(10, 6) };
        button.Click += async (_, _) => { if (cancellation is not null && !duringWork) return; try { await action(); } catch (Exception e) { status.Text = e.Message; } }; return button;
    }
    private async Task GenerateAsync(string? resume)
    {
        var connection = new NotesConnection(mode.SelectedIndex == 0 ? TextProcessingMode.Local : TextProcessingMode.Cloud, endpoint.Text ?? "", model.Text ?? "", key.Text ?? "");
        _ = connection.BaseUri();
        if (connection.Mode == TextProcessingMode.Cloud && consent.IsChecked != true) throw new InvalidOperationException("Bitte die Textübertragung für diesen Lauf ausdrücklich freigeben. / Explicit text consent is required for this run.");
        if (connection.Mode == TextProcessingMode.Local) { settings.LocalEndpoint = connection.Endpoint; settings.LocalModel = connection.Model; }
        else { settings.CloudEndpoint = connection.Endpoint; settings.CloudModel = connection.Model; }
        settings.Save(); // Non-secret profiles persist; encrypted fields are preserved, session-only keys are never stored.
        cancellation = new(); var token = cancellation.Token; consent.IsEnabled = false; templates.IsEnabled = false; tabs.IsEnabled = false;
        mode.IsEnabled = false; endpoint.IsEnabled = false; model.IsEnabled = false; key.IsEnabled = false; versions.IsEnabled = false;
        try
        {
            result = await TemplateNotesService.GenerateAsync(folder, snapshot, active, connection, consent.IsChecked == true, resume, new Progress<string>(text => status.Text = text), token);
            updating = true; versions.ItemsSource = MeetingNotesService.Runs(folder); updating = false; BuildTabs(); status.Text = "KI-Entwurf gespeichert. Inhalt und Belege bitte prüfen. / AI draft saved; review content and evidence.";
        }
        finally { cancellation.Dispose(); cancellation = null; consent.IsChecked = false; consent.IsEnabled = true; templates.IsEnabled = true; tabs.IsEnabled = true; mode.IsEnabled = true; endpoint.IsEnabled = true; model.IsEnabled = true; key.IsEnabled = true; versions.IsEnabled = true; }
    }
    private async Task<string?> PromptAsync(string heading, string initial)
    {
        var input = new TextBox { Text = initial, MaxLength = 100 }; var stack = new StackPanel { Margin = new(20), Spacing = 12 }; stack.Children.Add(new TextBlock { Text = heading }); stack.Children.Add(input);
        var dialog = new Window { Title = heading, Width = 500, Height = 180, Content = stack, WindowStartupLocation = WindowStartupLocation.CenterOwner };
        var save = new Button { Content = "Speichern / Save" }; save.Click += (_, _) => { if (!string.IsNullOrWhiteSpace(input.Text)) dialog.Close(input.Text.Trim()); }; stack.Children.Add(save); return await dialog.ShowDialog<string?>(this);
    }
    private async Task<AnalysisStep?> EditAsync(AnalysisStep initial)
    {
        var name = new TextBox { Text = initial.Name, MaxLength = 100 };
        var prompt = new TextBox { Text = initial.Prompt, AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 160, MaxLength = 8000 };
        var kind = new ComboBox { ItemsSource = new[] { "Eigene Auswertung / Custom", "Kurzer Überblick / Summary", "Beschlüsse / Decisions", "Aufgaben mit Zuständigen / Tasks" }, SelectedIndex = (int)initial.Kind };
        var stack = new StackPanel { Margin = new(22), Spacing = 12 };
        foreach (var control in new Control[] { new TextBlock { Text = "Name des Bereichs / Section name" }, name, new TextBlock { Text = "Was soll die KI herausarbeiten? / What should the AI extract?" }, prompt,
            new TextBlock { Text = "Ausgabeform / Output style" }, kind, new TextBlock { Text = "Jeder Punkt benötigt ein Originalzitat. Audiospuren sind keine Personen. / Every point needs a verbatim quote. Tracks are not people.", TextWrapping = TextWrapping.Wrap } }) stack.Children.Add(control);
        var dialog = new Window { Title = "Analyseauftrag / Analysis instructions", Width = 700, Height = 540, Content = stack, WindowStartupLocation = WindowStartupLocation.CenterOwner };
        var save = new Button { Content = "Speichern / Save" }; save.Click += (_, _) => { if (!string.IsNullOrWhiteSpace(name.Text) && !string.IsNullOrWhiteSpace(prompt.Text)) dialog.Close(new AnalysisStep { Id = initial.Id, Name = name.Text.Trim(), Prompt = prompt.Text.Trim(), Kind = (AnalysisKind)kind.SelectedIndex }); }; stack.Children.Add(save);
        return await dialog.ShowDialog<AnalysisStep?>(this);
    }
}
