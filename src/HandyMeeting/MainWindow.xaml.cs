using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Microsoft.Win32;
using NAudio.CoreAudioApi;

namespace HandyMeeting;

public partial class MainWindow : Window
{
    private enum Mode { Idle, Checking, Starting, Recording, Stopping, Transcribing }
    private Mode mode = Mode.Checking;
    private readonly UserSettings settings = UserSettings.Load();
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromMilliseconds(250) };
    private CaptureController? capture;
    private StreamingCoordinator? streaming;
    private SessionStore? session;
    private IDisposable? sessionLease;
    private bool allowClose, streamingPaused;
    private long displayedSequence = -1;
    private bool appearanceReady;
    private readonly string transcriptPlaceholder;

    public MainWindow()
    {
        InitializeComponent();
        Ui.Initialize(); LanguageCombo.SelectedIndex = Ui.English ? 1 : 0; ThemeCombo.SelectedIndex = Ui.Dark ? 1 : 0;
        transcriptPlaceholder = TranscriptBox.Text; TranscriptBox.Text = Ui.T(transcriptPlaceholder);
        appearanceReady = true; Ui.Attach(this);
        VersionText.Text = "Version " + typeof(MainWindow).Assembly.GetName().Version?.ToString(3) + " · lokal";
        HandyPathInput.Text = string.IsNullOrWhiteSpace(settings.HandyPath) ? HandyClient.FindInstalled() ?? "" : settings.HandyPath;
        MicCheck.IsChecked = settings.IncludeMicrophone;
        timer.Tick += (_, _) => RefreshLive();
        Loaded += async (_, _) => { await LoadDevicesAsync(); await LoadModelsAsync(); await LoadHistoryAsync(); };
        Closing += WindowClosing;
    }
    private void SetMode(Mode value)
    {
        mode = value;
        SetupPanel.IsEnabled = value == Mode.Idle;
        StartButton.IsEnabled = value == Mode.Idle && ModelCombo.SelectedItem is HandyModel && OutputCombo.SelectedItem is AudioDevice;
        StopButton.IsEnabled = value == Mode.Recording;
        MuteButton.Visibility = value == Mode.Recording && MicCheck.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        CancelButton.Visibility = value is Mode.Transcribing or Mode.Recording ? Visibility.Visible : Visibility.Collapsed;
        CancelButton.Content = value == Mode.Recording ? "Streaming pausieren" : "Nachlauf pausieren";
        OpenSessionButton.IsEnabled = value == Mode.Idle;
        HistoryCombo.IsEnabled = value == Mode.Idle;
        HistoryOpenButton.IsEnabled = value == Mode.Idle && HistoryCombo.SelectedItem is SessionHistoryItem;
        HistoryRefreshButton.IsEnabled = value == Mode.Idle;
        RetryButton.IsEnabled = session?.Data.Parts.Count > 0 && ModelCombo.SelectedItem is HandyModel &&
            (value == Mode.Idle || (value == Mode.Recording && (streamingPaused || streaming?.HasFailed == true)));
        RetryButton.Content = value == Mode.Recording ? "Streaming fortsetzen" : "Offene Abschnitte nachholen";
        FolderButton.IsEnabled = session is not null;
        CopyTranscriptButton.IsEnabled = session is not null;
        NotesButton.IsEnabled = value == Mode.Idle && session?.Data.Status == "complete" && session.Data.Parts.Any(p => !string.IsNullOrWhiteSpace(p.Text));
        StateText.Text = value switch
        {
            Mode.Recording => "Aufnahme + Live-Text", Mode.Transcribing => "Gesicherte Abschnitte nachholen",
            Mode.Stopping => "Audio wird abgeschlossen", Mode.Starting => "Aufnahme startet",
            Mode.Checking => "Wird geprüft", _ => "Bereit · lokal"
        };
    }
    private async Task LoadDevicesAsync()
    {
        try
        {
            var outputs = await Task.Run(() => AudioDevices.List(DataFlow.Render));
            var microphones = await Task.Run(() => AudioDevices.List(DataFlow.Capture));
            OutputCombo.ItemsSource = outputs; OutputCombo.SelectedItem = outputs.Find(d => d.Id == settings.OutputId) ?? outputs.FirstOrDefault();
            MicCombo.ItemsSource = microphones; MicCombo.SelectedItem = microphones.Find(d => d.Id == settings.MicrophoneId) ?? microphones.FirstOrDefault();
        }
        catch (Exception e) { StatusText.Text = "Audiogeräte konnten nicht geladen werden: " + e.Message; }
    }
    private async Task LoadModelsAsync()
    {
        SetMode(Mode.Checking);
        try
        {
            StatusText.Text = "Installierte Streaming-Modelle in Handy werden gelesen …";
            var models = (await new HandyClient(HandyPathInput.Text.Trim().Trim('"')).ModelsAsync(CancellationToken.None)).Where(m => m.SupportsStreaming).ToList();
            ModelCombo.ItemsSource = models;
            ModelCombo.SelectedItem = models.Find(m => m.Id == settings.ModelId) ?? models.Find(m => m.Id.Contains("nemotron-3.5")) ?? models.FirstOrDefault();
            StatusText.Text = models.Count > 0 ? $"Installierte Streaming-Modelle: {models.Count}. Audio und Live-Text werden parallel lokal gesichert." : "Bitte in Handy ein Streaming-Modell installieren und anschließend erneut prüfen.";
        }
        catch (Exception e) { ModelCombo.ItemsSource = null; StatusText.Text = "Handy-Verbindung: " + e.Message; }
        finally { SetMode(Mode.Idle); }
    }
    private async Task LoadHistoryAsync()
    {
        var selected = (HistoryCombo.SelectedItem as SessionHistoryItem)?.Manifest;
        var items = await Task.Run(() => SessionHistoryItem.Read(Path.Combine(UserSettings.ProjectRoot, "Sessions")));
        HistoryCombo.ItemsSource = items; HistoryCombo.SelectedItem = items.Find(i => i.Manifest == selected) ?? items.FirstOrDefault();
        if (mode == Mode.Idle) SetMode(mode);
    }
    private void SaveSettings()
    {
        settings.HandyPath = HandyPathInput.Text.Trim().Trim('"');
        settings.ModelId = (ModelCombo.SelectedItem as HandyModel)?.Id ?? "";
        settings.OutputId = (OutputCombo.SelectedItem as AudioDevice)?.Id ?? "";
        settings.MicrophoneId = (MicCombo.SelectedItem as AudioDevice)?.Id ?? "";
        settings.IncludeMicrophone = MicCheck.IsChecked == true; settings.Save();
    }
    private void StartStreaming(string modelId, string modelPath, bool recording)
    {
        streamingPaused = false;
        streaming = new StreamingCoordinator(session!, modelId, modelPath, recording);
        streaming.Updated += () => Dispatcher.BeginInvoke(() =>
        {
            if (mode is not (Mode.Recording or Mode.Transcribing)) return;
            RefreshLive();
            if (streaming?.HasFailed == true) SetMode(mode);
        });
        streaming.Start();
    }
    private void RefreshLive()
    {
        if (session is null || mode is not (Mode.Recording or Mode.Transcribing)) return;
        if (capture is not null)
        {
            TimerText.Text = ClockTime.Format(capture.ElapsedMs);
            SystemMeter.Value = capture.SystemPeak; MicMeter.Value = capture.MicPeak;
        }
        if (!streamingPaused && streaming is not null) StatusText.Text = streaming.Message;
        SafetyText.Text = $"Audio auf Datenträger bis {ClockTime.Format(session.SavedAudioMs)} · Textstände im lokalen Journal";
        if (displayedSequence != session.Data.SnapshotSequence)
        {
            TranscriptBox.Text = Transcription.RenderLive(session);
            displayedSequence = session.Data.SnapshotSequence;
            TranscriptBox.ScrollToEnd();
        }
    }
    internal void AttachSoakSessionForTest(SessionStore testSession, StreamingCoordinator testStreaming)
    {
        session = testSession; streaming = testStreaming; mode = Mode.Recording;
        timer.Start();
        Measure(new Size(1120, 940)); Arrange(new Rect(0, 0, 1120, 940));
    }
    internal void RefreshSoakLayoutForTest() => UpdateLayout();
    private async void StartRecording(object sender, RoutedEventArgs e)
    {
        if (mode != Mode.Idle || OutputCombo.SelectedItem is not AudioDevice output || ModelCombo.SelectedItem is not HandyModel model) return;
        var microphone = MicCheck.IsChecked == true ? MicCombo.SelectedItem as AudioDevice : null;
        if (MicCheck.IsChecked == true && microphone is null) { StatusText.Text = "Bitte ein Mikrofon auswählen oder die Mikrofonaufnahme ausschalten."; return; }
        SetMode(Mode.Starting);
        try
        {
            var modelPath = ModelPaths.Resolve(model, HandyPathInput.Text.Trim().Trim('"'));
            SaveSettings();
            session = SessionStore.Create(Path.Combine(UserSettings.ProjectRoot, "Sessions"), TitleInput.Text, model.Id, output, microphone);
            sessionLease = session.AcquireLease();
            capture = new CaptureController(session);
            capture.Failed += message => Dispatcher.BeginInvoke(async () =>
            {
                if (mode == Mode.Recording) { await StopCaptureAsync(false); StatusText.Text = message + " Gesicherte Daten bleiben in der Historie."; }
            });
            await capture.StartAsync(output, microphone);
            MuteButton.Content = "Mikrofon stummschalten"; TimerText.Text = "00:00";
            TranscriptBox.Text = "Audio wird bereits lokal gesichert. Das Streaming-Modell wird geladen; anschließend erscheint hier der laufende Text.";
            TranscriptHint.Text = "Live-Ausschnitt · vollständiger Verlauf gespeichert";
            displayedSequence = -1;
            SetMode(Mode.Recording); timer.Start();
            StartStreaming(model.Id, modelPath, true);
        }
        catch (Exception error)
        {
            if (capture is not null) { try { await capture.StopAsync(); } catch { } }
            capture = null; sessionLease?.Dispose(); sessionLease = null;
            StatusText.Text = "Aufnahme konnte nicht starten: " + error.Message; SetMode(Mode.Idle);
        }
    }
    private async Task StopCaptureAsync(bool finishStreaming)
    {
        if (capture is null) return;
        SetMode(Mode.Stopping);
        try
        {
            await capture.StopAsync(); capture = null;
            SystemMeter.Value = 0; MicMeter.Value = 0;
            TimerText.Text = ClockTime.Format(session!.Data.DurationMs);
            SetMode(Mode.Transcribing);
            if (streaming is not null)
            {
                if (finishStreaming && !streamingPaused) await streaming.FinishAsync();
                else await streaming.PauseAsync();
                StatusText.Text = streaming.Message;
            }
            session.ExportSnapshot(); TranscriptBox.Text = Transcription.Render(session, false);
            TranscriptHint.Text = session.Data.Status == "complete" ? "Vollständig lokal gespeichert" : "Lokal gesichert · Fortsetzen möglich";
            SafetyText.Text = $"Audio bis {ClockTime.Format(session.SavedAudioMs)} gespeichert · Originale bleiben erhalten";
        }
        catch (Exception error) { StatusText.Text = "Beim Abschließen ist ein Fehler aufgetreten: " + error.Message + " Gespeicherte Daten bleiben erhalten."; }
        finally
        {
            capture = null; timer.Stop();
            if (streaming is not null) { await streaming.DisposeAsync(); streaming = null; }
            sessionLease?.Dispose(); sessionLease = null;
            SetMode(Mode.Idle); await LoadHistoryAsync();
        }
    }
    private async Task ProcessSavedSessionAsync()
    {
        if (session is null || ModelCombo.SelectedItem is not HandyModel model) return;
        try
        {
            var path = ModelPaths.Resolve(model, HandyPathInput.Text.Trim().Trim('"'));
            sessionLease = session.AcquireLease();
            SetMode(Mode.Transcribing); displayedSequence = -1; timer.Start();
            StartStreaming(model.Id, path, false);
            await streaming!.FinishAsync();
            StatusText.Text = streaming.Message; TranscriptBox.Text = Transcription.Render(session, false);
            TranscriptHint.Text = session.Data.Status == "complete" ? "Vollständig lokal gespeichert" : "Fortsetzen möglich";
        }
        catch (Exception error) { StatusText.Text = "Offene Abschnitte: " + error.Message + " Audio und vorhandener Text bleiben erhalten."; }
        finally
        {
            timer.Stop(); if (streaming is not null) { await streaming.DisposeAsync(); streaming = null; }
            sessionLease?.Dispose(); sessionLease = null; SetMode(Mode.Idle); await LoadHistoryAsync();
        }
    }
    private async void StopRecording(object sender, RoutedEventArgs e) { if (mode == Mode.Recording) await StopCaptureAsync(true); }
    private void ToggleMicrophone(object sender, RoutedEventArgs e)
    {
        if (capture is null) return; capture.MicrophoneMuted = !capture.MicrophoneMuted;
        MuteButton.Content = capture.MicrophoneMuted ? "Mikrofon wieder aufnehmen" : "Mikrofon stummschalten";
    }
    private async void CancelTranscription(object sender, RoutedEventArgs e)
    {
        if (streaming is null) return;
        streamingPaused = true; await streaming.PauseAsync();
        StatusText.Text = mode == Mode.Recording ? "Streaming pausiert. Audio wird weiter lokal gesichert." : "Nachlauf pausiert. Gesicherte Daten können über die Historie fortgesetzt werden.";
        SetMode(mode);
    }
    private async void RetryTranscription(object sender, RoutedEventArgs e)
    {
        if (mode == Mode.Idle) await ProcessSavedSessionAsync();
        else if (mode == Mode.Recording && session is not null)
        {
            if (streaming is not null) await streaming.DisposeAsync();
            StartStreaming(session.Data.ModelId, session.Data.StreamingModelPath!, true); SetMode(mode);
        }
    }
    private void ShowSession(string manifest)
    {
        try
        {
            session = SessionStore.Open(manifest); TitleInput.Text = session.Data.Title; TimerText.Text = ClockTime.Format(session.Data.DurationMs);
            TranscriptBox.Text = Transcription.Render(session, false); TranscriptHint.Text = "Gespeicherte Sitzung";
            SafetyText.Text = "Originalaudio und alle gespeicherten Textstände bleiben lokal erhalten.";
            StatusText.Text = session.Data.Status == "complete" ? "Sitzung aus der Historie geöffnet." : "Gesicherte Sitzung geöffnet. Offene Abschnitte können nachgeholt werden.";
            SetMode(Mode.Idle);
        }
        catch (Exception error) { StatusText.Text = "Sitzung konnte nicht geöffnet werden (eventuell noch aktiv): " + error.Message; }
    }
    private void OpenSession(object sender, RoutedEventArgs e)
    {
        if (mode != Mode.Idle) return;
        var dialog = new OpenFileDialog { Title = "Gespeicherte Sitzung öffnen", Filter = "HandyMeeting-Sitzung (session.json)|session.json", InitialDirectory = Path.Combine(UserSettings.ProjectRoot, "Sessions") };
        if (dialog.ShowDialog() == true) ShowSession(dialog.FileName);
    }
    private void OpenHistory(object sender, RoutedEventArgs e) { if (mode == Mode.Idle && HistoryCombo.SelectedItem is SessionHistoryItem item) ShowSession(item.Manifest); }
    private async void RefreshHistory(object sender, RoutedEventArgs e) { if (mode == Mode.Idle) await LoadHistoryAsync(); }
    private void HistorySelectionChanged(object sender, SelectionChangedEventArgs e) { if (IsLoaded && mode == Mode.Idle) SetMode(mode); }
    private void OpenNotes(object sender, RoutedEventArgs e)
    {
        if (mode != Mode.Idle || session is null) return;
        try
        {
            TranscriptSnapshot snapshot;
            using (session.AcquireLease())
            {
                var current = JsonData.Read<SessionData>(session.ManifestPath) ?? throw new InvalidDataException("Die Sitzung ist nicht lesbar.");
                snapshot = TranscriptSnapshot.Create(current);
            }
            new NotesWindow(session.Folder, snapshot) { Owner = this }.ShowDialog();
        }
        catch (Exception error) { StatusText.Text = "Nachbearbeitung: " + error.Message; }
    }
    private void OpenFolder(object sender, RoutedEventArgs e)
    {
        if (session is not null) Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { session.Folder }, UseShellExecute = true });
    }
    private async void CopyTranscript(object sender, RoutedEventArgs e)
    {
        var current = session; if (current is null) return;
        CopyTranscriptButton.IsEnabled = false;
        try { var text = await Task.Run(current.CopyTranscriptSnapshot); Clipboard.SetText(text); StatusText.Text = Ui.T("Gesamtes Transkript kopiert · einschließlich bisherigem Live-Text."); }
        catch (Exception error) { StatusText.Text = Ui.T("Kopieren nicht möglich: ") + error.Message; }
        finally { CopyTranscriptButton.IsEnabled = session is not null; }
    }
    private void AppearanceChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!appearanceReady) return;
        try { Ui.Change(LanguageCombo.SelectedIndex == 1, ThemeCombo.SelectedIndex == 1); if (session is null) TranscriptBox.Text = Ui.T(transcriptPlaceholder); }
        catch (Exception error) { StatusText.Text = Ui.T("Einstellungen konnten nicht gespeichert werden: ") + error.Message; }
    }
    private async void BrowseHandy(object sender, RoutedEventArgs e)
    {
        var dialog = new OpenFileDialog { Title = "Installierte handy.exe auswählen", Filter = "Handy (handy.exe)|handy.exe|Anwendung (*.exe)|*.exe" };
        if (dialog.ShowDialog() == true) { HandyPathInput.Text = dialog.FileName; await LoadModelsAsync(); }
    }
    private async void ReloadModels(object sender, RoutedEventArgs e) { if (mode == Mode.Idle) await LoadModelsAsync(); }
    private async void RefreshDevices(object sender, RoutedEventArgs e) { if (mode == Mode.Idle) { await LoadDevicesAsync(); SetMode(Mode.Idle); } }
    private void ModelChanged(object sender, SelectionChangedEventArgs e) { if (IsLoaded && mode == Mode.Idle) SetMode(mode); }
    private void MicChoiceChanged(object sender, RoutedEventArgs e) { if (MicCombo is not null) MicCombo.IsEnabled = MicCheck.IsChecked == true; }
    private async void WindowClosing(object? sender, CancelEventArgs e)
    {
        if (allowClose || mode is Mode.Idle or Mode.Checking) { timer.Stop(); return; }
        e.Cancel = true;
        if (mode == Mode.Recording)
        {
            if (MessageBox.Show(this, "Aufnahme beenden und Audio sowie bisherige Textstände lokal sichern?", "HandyMeeting schließen", MessageBoxButton.YesNo, MessageBoxImage.Question) != MessageBoxResult.Yes) return;
            await StopCaptureAsync(false); allowClose = true; Close();
        }
        else if (mode == Mode.Transcribing && streaming is not null)
        { streamingPaused = true; await streaming.PauseAsync(); StatusText.Text = "Nachlauf pausiert. Nach dem Sichern kann das Fenster geschlossen werden."; }
    }
}
