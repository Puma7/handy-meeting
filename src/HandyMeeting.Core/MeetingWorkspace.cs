namespace HandyMeeting;

public sealed class MeetingWorkspace : IAsyncDisposable
{
    public SessionStore? Session { get; private set; }
    public PortableCapture? Capture { get; private set; }
    public StreamingCoordinator? Streaming { get; private set; }
    private IDisposable? lease;
    public bool Recording => Capture is not null;
    public async Task StartAsync(string title, HandyModel model, string handyPath, AudioDevice system, AudioDevice? microphone)
    {
        if (Recording) throw new InvalidOperationException("A recording is already active.");
        var modelPath = ModelPaths.Resolve(model, handyPath);
        Session = SessionStore.Create(Path.Combine(UserSettings.ProjectRoot, "Sessions"), title, model.Id, system, microphone);
        lease = Session.AcquireLease(); Capture = new(Session);
        try
        {
            await Capture.StartAsync(system, microphone);
            Streaming = new(Session, model.Id, modelPath, true); Streaming.Start();
        }
        catch { await Capture.DisposeAsync(); Capture = null; lease.Dispose(); lease = null; throw; }
    }
    public async Task StopAsync()
    {
        if (Capture is null) return;
        try { await Capture.StopAsync(); if (Streaming is not null) await Streaming.FinishAsync(); }
        finally
        {
            if (Streaming is not null) { await Streaming.DisposeAsync(); Streaming = null; }
            Capture = null; lease?.Dispose(); lease = null; Session?.ExportSnapshot();
        }
    }
    public void Open(string sessionId)
    {
        if (Recording) throw new InvalidOperationException("Finish the active recording first.");
        var path = FindSession(sessionId); Session = SessionStore.Open(path);
    }
    public static string FindSession(string id)
    {
        if (!Guid.TryParseExact(id, "N", out _)) throw new InvalidDataException("Invalid session identifier.");
        foreach (var manifest in Manifests())
        {
            var data = JsonData.Read<SessionData>(manifest); if (data?.Id == id) return manifest;
        }
        throw new FileNotFoundException("Session not found.");
    }
    public static IEnumerable<string> Manifests()
    {
        var root = Path.Combine(UserSettings.ProjectRoot, "Sessions");
        return Directory.Exists(root) ? Directory.EnumerateFiles(root, "session.json", SearchOption.AllDirectories) : [];
    }
    public static List<SessionData> History() => Manifests().Select(p =>
    {
        try { return JsonData.Read<SessionData>(p); } catch (Exception e) when (e is IOException or System.Text.Json.JsonException) { return null; }
    }).OfType<SessionData>().OrderByDescending(s => s.StartedAt).ToList();
    public async ValueTask DisposeAsync()
    {
        if (Capture is not null)
        {
            await Capture.StopAsync(); if (Streaming is not null) await Streaming.PauseAsync();
        }
        if (Streaming is not null) await Streaming.DisposeAsync(); lease?.Dispose();
    }
}
