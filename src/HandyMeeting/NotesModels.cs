using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace HandyMeeting;

public static class NotesProtocol
{
    public const string PromptVersion = "meeting-notes-v1"; // Legacy 0.3 runs keep their original protocol.
    public const string StepPromptVersion = "meeting-steps-v1"; // Bump when step instructions/schema change after release.
}
public sealed record TranscriptSource(string Id, string Track, long StartMs, long EndMs, string Text);
public sealed record TranscriptSnapshot(string SessionId, string Title, string Hash, List<TranscriptSource> Sources)
{
    public DateTimeOffset StartedAt { get; init; }
    public static string CalculateHash(string sessionId, List<TranscriptSource> sources) => Convert.ToHexString(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(new { Id = sessionId, sources })));
    public void Validate()
    {
        if (Sources is null || Sources.Count == 0 || Sources.Any(s => s is null || !System.Text.RegularExpressions.Regex.IsMatch(s.Id ?? "", @"^S\d{5,}$") || s.Track is not ("system" or "microphone") || s.StartMs < 0 || s.EndMs < s.StartMs || string.IsNullOrEmpty(s.Text))
            || !Sources.Any(s => !string.IsNullOrWhiteSpace(s.Text))
            || Sources.Select(s => s.Id).Distinct(StringComparer.Ordinal).Count() != Sources.Count || Hash != CalculateHash(SessionId, Sources))
            throw new InvalidDataException("Der gespeicherte Originaltranskriptstand ist beschädigt oder verändert.");
    }
    public static TranscriptSnapshot Create(SessionData data)
    {
        if (data.Status != "complete" || data.Parts.Any(p => p.Status != "complete" || !p.AudioClosed))
            throw new InvalidOperationException("Bitte zuerst alle Transkriptionsabschnitte abschließen.");
        var sources = new List<TranscriptSource>();
        foreach (var part in data.Parts.OrderBy(p => p.StartSample).ThenBy(p => p.Source, StringComparer.Ordinal))
        {
            if (string.IsNullOrWhiteSpace(part.Text)) continue;
            // Bound even legacy, unusually long transcript sections. IDs belong to this frozen snapshot.
            for (var offset = 0; offset < part.Text.Length;)
            {
                var count = Math.Min(2400, part.Text.Length - offset);
                if (offset + count < part.Text.Length)
                {
                    if (char.IsHighSurrogate(part.Text[offset + count - 1]) && char.IsLowSurrogate(part.Text[offset + count])) count--;
                    var boundary = part.Text.LastIndexOf(' ', offset + count - 1, Math.Min(count, 200));
                    if (boundary > offset) count = boundary - offset + 1;
                }
                sources.Add(new($"S{sources.Count + 1:D5}", part.Source, part.StartMs, part.EndMs, part.Text.Substring(offset, count))); offset += count;
            }
        }
        if (sources.Count == 0) throw new InvalidOperationException("Diese Sitzung enthält noch keinen Transkripttext.");
        var hash = CalculateHash(data.Id, sources);
        return new(data.Id, data.Title, hash, sources) { StartedAt = data.StartedAt };
    }
}
public sealed record NoteEvidence(string SourceId, string Quote);
public sealed class MeetingFact
{
    public string Text { get; set; } = "";
    public string? Owner { get; set; }
    public string? Due { get; set; }
    public List<NoteEvidence> Evidence { get; set; } = [];
}
public sealed class MeetingNotes
{
    public List<MeetingFact> Summary { get; set; } = [];
    public List<MeetingFact> Decisions { get; set; } = [];
    public List<MeetingFact> Todos { get; set; } = [];
}
public static class NotesValidation
{
    public static MeetingNotes Parse(string json, IReadOnlyDictionary<string, TranscriptSource> allowed, int maxItemsPerGroup = 30)
    {
        using var document = JsonDocument.Parse(json);
        RequireFields(document.RootElement, ["summary", "decisions", "todos"]);
        foreach (var group in document.RootElement.EnumerateObject())
        {
            if (group.Value.ValueKind != JsonValueKind.Array || group.Value.GetArrayLength() > maxItemsPerGroup) throw new InvalidDataException("Ungültiger Ergebnisbereich.");
            foreach (var fact in group.Value.EnumerateArray())
            {
                RequireFields(fact, ["text", "owner", "due", "evidence"]);
                var evidenceList = fact.EnumerateObject().Single(p => p.Name.Equals("evidence", StringComparison.OrdinalIgnoreCase)).Value;
                if (evidenceList.ValueKind != JsonValueKind.Array) throw new InvalidDataException("Quellenbelege fehlen.");
                foreach (var evidence in evidenceList.EnumerateArray()) RequireFields(evidence, ["sourceId", "quote"]);
            }
        }
        var result = JsonSerializer.Deserialize<MeetingNotes>(json, JsonData.Options) ?? throw new InvalidDataException("Leere KI-Antwort.");
        Validate(result, allowed); return result;
    }
    private static void RequireFields(JsonElement element, string[] expected)
    {
        if (element.ValueKind != JsonValueKind.Object) throw new InvalidDataException("Ungültige JSON-Struktur.");
        var fields = element.EnumerateObject().Select(p => p.Name).ToList();
        if (fields.Count != expected.Length || fields.Distinct(StringComparer.OrdinalIgnoreCase).Count() != fields.Count || expected.Any(name => !fields.Contains(name, StringComparer.OrdinalIgnoreCase)))
            throw new InvalidDataException("Die KI-Antwort enthält fehlende, unbekannte oder doppelte Felder.");
    }
    public static void Validate(MeetingNotes result, IReadOnlyDictionary<string, TranscriptSource> allowed)
    {
        if (result.Summary is null || result.Decisions is null || result.Todos is null) throw new InvalidDataException("Die KI-Antwort enthält nicht alle Ergebnisbereiche.");
        foreach (var group in new[] { result.Summary, result.Decisions, result.Todos })
        {
            foreach (var fact in group)
            {
                if (fact is null || string.IsNullOrWhiteSpace(fact.Text) || fact.Text.Length > 1600 || fact.Evidence is null || fact.Evidence.Count is < 1 or > 6)
                    throw new InvalidDataException("Eine KI-Aussage hat keinen gültigen Quellenbeleg.");
                foreach (var evidence in fact.Evidence)
                    if (evidence is null || string.IsNullOrEmpty(evidence.SourceId) || !allowed.TryGetValue(evidence.SourceId, out var source)
                        || string.IsNullOrWhiteSpace(evidence.Quote) || evidence.Quote.Length is < 8 or > 600
                        || !source.Text.Contains(evidence.Quote, StringComparison.Ordinal))
                        throw new InvalidDataException("Ein KI-Beleg stimmt nicht mit dem Originaltext überein. Der bisherige Arbeitsstand bleibt erhalten.");
                foreach (var value in new[] { fact.Owner, fact.Due })
                    if (value is not null && (string.IsNullOrWhiteSpace(value) || value.Length > 200 || !fact.Evidence.Any(e => e.Quote.Contains(value, StringComparison.OrdinalIgnoreCase))))
                        throw new InvalidDataException("Zuständigkeit oder Termin ist nicht wörtlich durch ein Zitat belegt.");
            }
        }
        if (result.Summary.Concat(result.Decisions).Any(f => f.Owner is not null || f.Due is not null)) throw new InvalidDataException("Zuständige und Termine gehören ausschließlich zu To-dos.");
    }
}
public enum TextProcessingMode { Local, Cloud }
public sealed class NotesSettings
{
    public string LocalEndpoint { get; set; } = "http://127.0.0.1:1234/v1/";
    public string LocalModel { get; set; } = "";
    public string CloudEndpoint { get; set; } = "https://openrouter.ai/api/v1/";
    public string CloudModel { get; set; } = "";
    public string ProtectedLocalKey { get; set; } = "";
    public string ProtectedCloudKey { get; set; } = "";
    public static string FilePath => Path.Combine(UserSettings.DataRoot, "notes-settings.json");
    public static NotesSettings Load()
    {
        try { return JsonData.Read<NotesSettings>(FilePath) ?? new(); }
        catch (Exception error) when (error is IOException or JsonException or UnauthorizedAccessException) { return new(); }
    }
    public void Save() => JsonData.Save(FilePath, this);
#if PORTABLE
    // Portable preview keeps keys only in process memory until platform keychain integration is verified.
    public static string Protect(string secret) => string.IsNullOrEmpty(secret) ? "" : throw new PlatformNotSupportedException("Portable API keys are session-only. No plaintext key will be saved.");
    public static string Unprotect(string secret) => string.IsNullOrEmpty(secret) ? "" : throw new PlatformNotSupportedException("This encrypted key belongs to the Windows application.");
#else
    public static string Protect(string secret) => string.IsNullOrEmpty(secret) ? "" : Convert.ToBase64String(ProtectedData.Protect(Encoding.UTF8.GetBytes(secret), null, DataProtectionScope.CurrentUser));
    public static string Unprotect(string secret) => string.IsNullOrEmpty(secret) ? "" : Encoding.UTF8.GetString(ProtectedData.Unprotect(Convert.FromBase64String(secret), null, DataProtectionScope.CurrentUser));
#endif
}
public sealed record NotesConnection(TextProcessingMode Mode, string Endpoint, string Model, string ApiKey)
{
    public Uri BaseUri()
    {
        if (!Uri.TryCreate(Endpoint.Trim(), UriKind.Absolute, out var uri) || !string.IsNullOrEmpty(uri.UserInfo) || !string.IsNullOrEmpty(uri.Query) || !string.IsNullOrEmpty(uri.Fragment))
            throw new InvalidOperationException("Bitte eine gültige API-Adresse ohne Zugangsdaten oder Zusatzparameter eingeben.");
        var host = uri.DnsSafeHost.Trim('[', ']');
        var loopback = host.Equals("localhost", StringComparison.OrdinalIgnoreCase) || (System.Net.IPAddress.TryParse(host, out var ip) && System.Net.IPAddress.IsLoopback(ip));
        if (Mode == TextProcessingMode.Local)
        {
            if (!loopback || uri.Scheme is not ("http" or "https")) throw new InvalidOperationException("Im lokalen Modus sind ausschließlich Loopback-Adressen auf diesem Computer erlaubt.");
            if (host.Equals("localhost", StringComparison.OrdinalIgnoreCase)) uri = new UriBuilder(uri) { Host = "127.0.0.1" }.Uri;
            if (Model.EndsWith(":cloud", StringComparison.OrdinalIgnoreCase) || Model.EndsWith("-cloud", StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("Ein als Cloud-Modell gekennzeichnetes Modell ist im lokalen Modus nicht erlaubt.");
        }
        else if (loopback || uri.Scheme != "https") throw new InvalidOperationException("Für Cloud-Anbieter ist eine HTTPS-Adresse erforderlich.");
        return new Uri(uri.AbsoluteUri.TrimEnd('/') + "/");
    }
    public string Fingerprint => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes($"{NotesProtocol.PromptVersion}\n{Mode}\n{BaseUri()}\n{Model}")));
}
