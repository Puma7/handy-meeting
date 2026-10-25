using System.Diagnostics;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace HandyMeeting;

public static class JsonData
{
    public static readonly JsonSerializerOptions Options = new() { WriteIndented = true, PropertyNameCaseInsensitive = true };
    public static T? Read<T>(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        return JsonSerializer.Deserialize<T>(stream, Options);
    }
    public static void Save<T>(string path, T value) => WriteAtomic(path, JsonSerializer.SerializeToUtf8Bytes(value, Options));
    public static void SaveText(string path, string text) => WriteAtomic(path, System.Text.Encoding.UTF8.GetBytes(text));
    private static void WriteAtomic(string path, byte[] bytes)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temporary = path + ".tmp";
        using (var output = new FileStream(temporary, FileMode.Create, FileAccess.Write, FileShare.None, 4096, FileOptions.WriteThrough))
        { output.Write(bytes); output.Flush(true); }
        if (File.Exists(path)) File.Replace(temporary, path, path + ".bak", true);
        else File.Move(temporary, path);
    }
}

public sealed class UserSettings
{
    public string HandyPath { get; set; } = "";
    public string ModelId { get; set; } = "";
    public string OutputId { get; set; } = "";
    public string MicrophoneId { get; set; } = "";
    public bool IncludeMicrophone { get; set; }
    public static string ProjectRoot => AppStorage.Root;
    public static string DataRoot => Path.Combine(ProjectRoot, "data");
    public static UserSettings Load()
    {
        try { return JsonSerializer.Deserialize<UserSettings>(File.ReadAllText(Path.Combine(DataRoot, "settings.json")), JsonData.Options) ?? new(); }
        catch (Exception e) when (e is IOException or JsonException or UnauthorizedAccessException) { return new(); }
    }
    public void Save() => JsonData.Save(Path.Combine(DataRoot, "settings.json"), this);
}

public sealed class HandyModel
{
    [JsonPropertyName("id")] public string Id { get; set; } = "";
    [JsonPropertyName("name")] public string Name { get; set; } = "";
    [JsonPropertyName("is_downloaded")] public bool IsDownloaded { get; set; }
    [JsonPropertyName("supports_streaming")] public bool SupportsStreaming { get; set; }
    [JsonPropertyName("filename")] public string Filename { get; set; } = "";
    [JsonPropertyName("source")] public JsonElement Source { get; set; }
    public override string ToString() => Name;
}

public sealed record AudioDevice(string Id, string Name)
{
    public override string ToString() => Name;
}

public sealed class SessionData
{
    public int SchemaVersion { get; set; } = 1;
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string Title { get; set; } = "Meeting";
    public DateTimeOffset StartedAt { get; set; } = DateTimeOffset.Now;
    public DateTimeOffset? EndedAt { get; set; }
    public string Status { get; set; } = "recording";
    public string ModelId { get; set; } = "";
    public string OutputName { get; set; } = "";
    public string? MicrophoneName { get; set; }
    public int ChunkSeconds { get; set; } = 30;
    public long DurationMs { get; set; }
    public bool CaptureIncomplete { get; set; }
    public long TimingFallbackPackets { get; set; }
    public string? StreamingModelPath { get; set; }
    public string? StreamingError { get; set; }
    public string? StreamingBackend { get; set; }
    public long SnapshotSequence { get; set; }
    public List<string> Warnings { get; set; } = [];
    public List<AudioPart> Parts { get; set; } = [];
}

public sealed class AudioPart
{
    public string File { get; set; } = "";
    public string Source { get; set; } = "system";
    public long StartSample { get; set; }
    public long SampleCount { get; set; }
    public bool HasSignal { get; set; }
    public string Status { get; set; } = "pending";
    public string? Text { get; set; }
    public string? Error { get; set; }
    public string? ModelId { get; set; }
    public bool AudioClosed { get; set; } = true;
    public long ProcessedSamples { get; set; }
    public string CommittedText { get; set; } = "";
    public string TentativeText { get; set; } = "";
    public AudioPart Copy() => (AudioPart)MemberwiseClone();
    public long StartMs => StartSample * 1000 / PcmTimeline.SampleRate;
    public long EndMs => (StartSample + SampleCount) * 1000 / PcmTimeline.SampleRate;
}

public sealed partial class SessionStore
{
    private readonly object gate = new();
    public string Folder { get; }
    public SessionData Data { get; }
    public string ManifestPath => Path.Combine(Folder, "session.json");
    public SessionStore(string folder, SessionData data) { Folder = Path.GetFullPath(folder); Data = data; }
    public static SessionStore Create(string root, string title, string model, AudioDevice output, AudioDevice? microphone, int chunkSeconds = 30)
    {
        var data = new SessionData { Title = string.IsNullOrWhiteSpace(title) ? "Meeting" : title.Trim(), ModelId = model, OutputName = output.Name, MicrophoneName = microphone?.Name, ChunkSeconds = chunkSeconds };
        var folder = Path.Combine(root, $"{DateTime.Now:yyyy-MM-dd_HH-mm-ss}_{data.Id[..6]}");
        Directory.CreateDirectory(Path.Combine(folder, "audio"));
        var store = new SessionStore(folder, data); store.Save(); return store;
    }
    public void Save() { lock (gate) JsonData.Save(ManifestPath, Data); }
    public void AddPart(AudioPart part) => AudioCheckpoint(part, true);
    public void Warn(string message)
    {
        lock (gate) { Data.CaptureIncomplete = true; if (!Data.Warnings.Contains(message)) Data.Warnings.Add(message); Save(); }
    }
    public void NoteEstimatedTiming()
    {
        lock (gate)
        {
            Data.TimingFallbackPackets++;
            if (Data.TimingFallbackPackets != 1) return;
            Data.Warnings.Add("Der Audiotreiber liefert zeitweise keine präzisen Zeitmarken. Die Aufnahmebereiche sind anhand der Empfangszeit angenähert.");
            Save();
        }
    }
    public string AudioPath(AudioPart part)
    {
        if (!System.Text.RegularExpressions.Regex.IsMatch(part.File, @"^(system|microphone)-\d{12,}\.wav$")) throw new InvalidDataException("Ungültiger Audio-Dateiname in der Sitzung.");
        return Path.Combine(Folder, "audio", part.File);
    }
    public static SessionStore Open(string manifest)
    {
        var folder = Path.GetDirectoryName(Path.GetFullPath(manifest))!;
        using var lease = new FileStream(Path.Combine(folder, "session.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        SessionData data;
        var usedBackup = false;
        try { data = JsonSerializer.Deserialize<SessionData>(File.ReadAllText(manifest), JsonData.Options) ?? throw new InvalidDataException("Die Sitzung ist leer."); }
        catch (Exception error) when (error is IOException or JsonException)
        {
            data = JsonSerializer.Deserialize<SessionData>(File.ReadAllText(manifest + ".bak"), JsonData.Options) ?? throw new InvalidDataException("Die Sitzung und ihre Sicherung sind nicht lesbar.");
            usedBackup = true;
            if (File.Exists(manifest)) File.Move(manifest, manifest + ".damaged-" + Guid.NewGuid().ToString("N"));
        }
        if (data.SchemaVersion != 1 || data.ChunkSeconds is < 1 or > 300) throw new InvalidDataException("Diese Sitzungsversion wird nicht unterstützt.");
        var store = new SessionStore(Path.GetDirectoryName(Path.GetFullPath(manifest))!, data);
        var interrupted = data.Status is "recording" or "catching-up" or "streaming" || (data.EndedAt is null && data.Parts.Any(p => !p.AudioClosed));
        foreach (var part in data.Parts) _ = store.AudioPath(part);
        var audioFolder = Path.Combine(store.Folder, "audio");
        if (Directory.Exists(audioFolder))
        {
            foreach (var path in Directory.EnumerateFiles(audioFolder, "*.wav"))
            {
                var name = Path.GetFileName(path);
                var match = System.Text.RegularExpressions.Regex.Match(name, @"^(system|microphone)-(\d{12,})\.wav$");
                if (!match.Success || !long.TryParse(match.Groups[2].Value, out var start)) continue;
                var (samples, signal) = PcmTimeline.InspectAndRepair(path);
                var existing = data.Parts.FirstOrDefault(p => p.File == name);
                if (existing is null) data.Parts.Add(new() { File = name, Source = match.Groups[1].Value, StartSample = start, SampleCount = samples, HasSignal = signal });
                else { existing.SampleCount = samples; existing.HasSignal = signal; existing.AudioClosed = true; }
            }
        }
        foreach (var part in data.Parts)
        {
            if (!File.Exists(store.AudioPath(part))) throw new FileNotFoundException("Eine Audiodatei der Sitzung fehlt.", part.File);
            if (part.Status is "transcribing" or "streaming") part.Status = "pending";
        }
        store.ReplayTranscriptJournal();
        if (data.Status == "complete" && data.Parts.Any(p => p.Status != "complete")) data.Status = "recorded";
        data.DurationMs = Math.Max(data.DurationMs, data.Parts.Select(p => p.EndMs).DefaultIfEmpty().Max());
        if (interrupted) store.Warn("Die vorherige Aufnahme wurde unterbrochen. Bereits gespeichertes Audio wurde wiederhergestellt; der nicht gespeicherte Schluss kann fehlen.");
        if (usedBackup) store.Warn("Die Sitzungsübersicht wurde aus ihrer Sicherung und dem Textjournal wiederhergestellt.");
        if (data.Status != "complete") data.Status = "recorded";
        store.Save(); store.ExportSnapshot(); return store;
    }
}

public static class ClockTime
{
    public static long Qpc100ns => (long)(Stopwatch.GetTimestamp() * (10_000_000d / Stopwatch.Frequency));
    public static string Format(long milliseconds) => TimeSpan.FromMilliseconds(milliseconds).ToString(milliseconds >= 3_600_000 ? @"hh\:mm\:ss" : @"mm\:ss");
}
