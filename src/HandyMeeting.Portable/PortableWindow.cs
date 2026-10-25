using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;

namespace HandyMeeting;

public sealed class PortableWindow : Window
{
    private readonly MeetingWorkspace workspace = new();
    private readonly TextBox title = new() { Text = "Mein Meeting" }, handy = new() { Text = HandyClient.FindInstalled() ?? "", Watermark = "Handy-App oder AppImage auswählen / Handy executable path" };
    private readonly ComboBox output = new(), microphone = new(), models = new(), history = new();
    private readonly CheckBox includeMic = new() { Content = "Mikrofon zusätzlich aufnehmen / Include microphone" };
    private readonly TextBox transcript = new() { IsReadOnly = true, AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 180 };
    private readonly TextBlock status = new() { TextWrapping = TextWrapping.Wrap, Text = "Portable Vorschau · Aufnahmen auf diesem Betriebssystem müssen noch praktisch geprüft werden." };
    private readonly StackPanel setup = new() { Spacing = 8 };
    private readonly Button start, stop, notes;
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private bool busy, closing;
    public PortableWindow()
    {
        Title = "HandyMeeting · Portable Preview"; Width = 1100; Height = 900; MinWidth = 820; MinHeight = 700;
        var root = new Grid { Margin = new(26), RowDefinitions = new("Auto,Auto,Auto,*,Auto"), RowSpacing = 16 };
        var header = new DockPanel();
        var theme = new Button { Content = "◐ Hell / Dunkel", HorizontalAlignment = HorizontalAlignment.Right };
        theme.Click += (_, _) => Application.Current!.RequestedThemeVariant = ActualThemeVariant == ThemeVariant.Dark ? ThemeVariant.Light : ThemeVariant.Dark;
        DockPanel.SetDock(theme, Dock.Right); header.Children.Add(theme);
        header.Children.Add(new TextBlock { Text = "HandyMeeting", FontSize = 30, FontWeight = FontWeight.SemiBold }); root.Children.Add(header);
        var audio = new StackPanel { Spacing = 8 }; var recognition = new StackPanel { Spacing = 8 };
        var columns = new Grid { ColumnDefinitions = new("*,24,*") }; columns.Children.Add(audio); Grid.SetColumn(recognition, 2); columns.Children.Add(recognition); setup.Children.Add(columns);
        void Label(StackPanel panel, string text, Control control) { control.HorizontalAlignment = HorizontalAlignment.Stretch; panel.Children.Add(new TextBlock { Text = text, FontWeight = FontWeight.SemiBold }); panel.Children.Add(control); }
        Label(audio, "Meeting", title); Label(recognition, "Handy", handy);
        var check = Button("Handy & Audio prüfen / Check", async () => await LoadAsync()); recognition.Children.Add(check);
        Label(audio, "Computerton / System audio", output); audio.Children.Add(includeMic); Label(audio, "Mikrofon / Microphone", microphone); Label(recognition, "Streaming-Modell / Model", models);
        recognition.Children.Add(new TextBlock { Text = "Audio und Live-Text bleiben lokal. Ein Erkennungsfehler beendet die Aufnahme nicht. / Audio and live text stay local. An inference error does not stop recording.", TextWrapping = TextWrapping.Wrap, Margin = new(0, 10, 0, 0) });
        Grid.SetRow(setup, 1); root.Children.Add(setup);
        var actions = new WrapPanel { Orientation = Orientation.Horizontal };
        start = Button("Aufnahme starten / Start", async () =>
        {
            if (models.SelectedItem is not HandyModel model || output.SelectedItem is not AudioDevice device) throw new InvalidOperationException("Bitte zuerst Handy und Audio prüfen. / Check Handy and audio first.");
            await workspace.StartAsync(title.Text ?? "Meeting", model, handy.Text ?? "", device, includeMic.IsChecked == true ? microphone.SelectedItem as AudioDevice ?? throw new InvalidOperationException("Microphone missing.") : null);
            workspace.Capture!.Failed += message => Dispatcher.UIThread.Post(() => status.Text = message);
            status.Text = "Audio wird lokal gesichert. / Audio is being saved locally."; timer.Start();
        });
        start.Background = Brush.Parse("#1C6249"); start.Foreground = Brushes.White;
        stop = Button("Meeting beenden / Stop", async () => { await workspace.StopAsync(); timer.Stop(); transcript.Text = workspace.Session?.CopyTranscriptSnapshot(); await HistoryAsync(); });
        var copy = Button("Transkript kopieren / Copy", async () => { if (workspace.Session is { } s && Clipboard is { } clipboard) await clipboard.SetTextAsync(await Task.Run(s.CopyTranscriptSnapshot)); });
        notes = Button("Nachbearbeitung / Analysis", async () =>
        {
            if (workspace.Session is not { } session) return;
            var snapshot = TranscriptSnapshot.Create(JsonData.Read<SessionData>(session.ManifestPath)!);
            await new PortableNotesWindow(session.Folder, snapshot).ShowDialog(this);
        });
        foreach (var button in new[] { start, stop, copy, notes }) actions.Children.Add(button);
        Grid.SetRow(actions, 2); root.Children.Add(actions);
        Grid.SetRow(transcript, 3); root.Children.Add(transcript);
        var footer = new StackPanel { Spacing = 8 }; footer.Children.Add(status); footer.Children.Add(new TextBlock { Text = "Historie / History" }); footer.Children.Add(history);
        footer.Children.Add(Button("Sitzung anzeigen / Open", () => { if (history.SelectedItem is SessionChoice choice) { workspace.Open(choice.Id); transcript.Text = workspace.Session!.CopyTranscriptSnapshot(); } return Task.CompletedTask; }));
        history.HorizontalAlignment = HorizontalAlignment.Stretch;
        Grid.SetRow(footer, 4); root.Children.Add(footer); Content = root;
        timer.Tick += (_, _) =>
        {
            if (workspace.Session is { } session) transcript.Text = Transcription.RenderLive(session);
            if (workspace.Capture is { } capture) status.Text = ClockTime.Format(capture.ElapsedMs) + " · " + (workspace.Session?.Data.CaptureIncomplete == true ? workspace.Session.Data.Warnings.LastOrDefault() : workspace.Streaming?.Message);
            UpdateState();
        };
        Opened += async (_, _) => { try { await LoadAsync(); } catch (Exception e) { status.Text = e.Message; } UpdateState(); };
        Closing += (_, e) =>
        {
            if (closing) return;
            if (workspace.Recording || busy) { e.Cancel = true; status.Text = "Bitte zuerst Aufnahme oder Verarbeitung beenden. / Finish recording or processing first."; }
            else { closing = true; timer.Stop(); }
        };
        UpdateState();
    }
    private Button Button(string caption, Func<Task> action)
    {
        var button = new Button { Content = caption, Margin = new(0, 0, 8, 4), Padding = new(12, 8) };
        ToolTip.SetTip(button, caption);
        button.Click += async (_, _) =>
        {
            if (busy) return; busy = true; UpdateState();
            try { await action(); } catch (Exception e) { status.Text = e.Message; }
            finally { busy = false; UpdateState(); }
        };
        return button;
    }
    private async Task LoadAsync()
    {
        var installed = await new HandyClient(handy.Text ?? "").ModelsAsync(CancellationToken.None);
        models.ItemsSource = installed.Where(m => m.SupportsStreaming).ToList(); models.SelectedIndex = models.ItemCount > 0 ? 0 : -1;
        output.ItemsSource = await PortableDevices.ListAsync(false, CancellationToken.None); output.SelectedIndex = output.ItemCount > 0 ? 0 : -1;
        microphone.ItemsSource = await PortableDevices.ListAsync(true, CancellationToken.None); microphone.SelectedIndex = microphone.ItemCount > 0 ? 0 : -1;
        await HistoryAsync();
    }
    private Task HistoryAsync()
    {
        history.ItemsSource = MeetingWorkspace.History().Select(s => new SessionChoice(s.Id, $"{s.StartedAt:yyyy-MM-dd HH:mm} · {s.Title} · {s.Status}")).ToList(); history.SelectedIndex = history.ItemCount > 0 ? 0 : -1; return Task.CompletedTask;
    }
    private sealed record SessionChoice(string Id, string Label) { public override string ToString() => Label; }
    private void UpdateState()
    {
        if (start is null) return;
        setup.IsEnabled = !busy && !workspace.Recording; start.IsEnabled = !busy && !workspace.Recording;
        stop.IsEnabled = !busy && workspace.Recording; notes.IsEnabled = !busy && !workspace.Recording && workspace.Session?.Data.Status == "complete";
        history.IsEnabled = !busy && !workspace.Recording;
    }
}
