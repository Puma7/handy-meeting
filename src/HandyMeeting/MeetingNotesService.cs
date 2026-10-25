using System.IO;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace HandyMeeting;

public sealed class NotesRun
{
    public string Id { get; set; } = "";
    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.Now;
    public string Status { get; set; } = "working";
    public string SourceHash { get; set; } = "";
    public string ConnectionFingerprint { get; set; } = "";
    public string Endpoint { get; set; } = "";
    public string Model { get; set; } = "";
    public string PromptVersion { get; set; } = NotesProtocol.PromptVersion;
    public TextProcessingMode Mode { get; set; }
    public int Batches { get; set; }
    public int CompletedBatches { get; set; }
    public string? Error { get; set; }
    public AnalysisTemplate? Template { get; set; }
    public string? TemplateHash { get; set; }
    public override string ToString() => $"{CreatedAt:dd.MM. HH:mm} · {(Mode == TextProcessingMode.Local ? "Lokal" : "Cloud")} · {Model} · {(Status == "complete" ? "Fertig" : "Fortsetzen möglich")}";
}
public sealed record NotesResult(NotesRun Run, TranscriptSnapshot Original, MeetingNotes Notes, string Folder)
{
    public List<NotesSection>? Sections { get; init; }
    public List<NotesSection> AllSections => Sections ?? [new("summary", "Zusammenfassung", AnalysisKind.Summary, Notes.Summary), new("decisions", "Entscheidungen", AnalysisKind.Decisions, Notes.Decisions), new("todos", "To-dos", AnalysisKind.Tasks, Notes.Todos)];
}

public static class MeetingNotesService
{
    private static readonly JsonSerializerOptions WireJson = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping };
    public static List<List<TranscriptSource>> Batches(TranscriptSnapshot snapshot)
    {
        var batches = new List<List<TranscriptSource>>(); var current = new List<TranscriptSource>(); var length = 0;
        foreach (var source in snapshot.Sources)
        {
            var size = JsonSerializer.Serialize(new { source.Id, source.Text }, WireJson).Length;
            if (length + size > 4500 && current.Count > 0) { batches.Add(current); current = []; length = 0; }
            current.Add(source); length += size;
        }
        if (current.Count > 0) batches.Add(current);
        return batches;
    }
    public static string RunsFolder(string sessionFolder) => Path.Combine(sessionFolder, "notes");
    public static List<NotesRun> Runs(string sessionFolder)
    {
        var root = RunsFolder(sessionFolder); var runs = new List<NotesRun>();
        if (!Directory.Exists(root)) return runs;
        foreach (var folder in Directory.EnumerateDirectories(root))
        {
            if (!ValidId(Path.GetFileName(folder))) continue;
            try { if (JsonData.Read<NotesRun>(Path.Combine(folder, "run.json")) is { } run && ValidId(run.Id) && run.Id == Path.GetFileName(folder)) runs.Add(run); }
            catch (Exception error) when (error is IOException or JsonException or UnauthorizedAccessException) { }
        }
        return runs.OrderByDescending(r => r.CreatedAt).ToList();
    }
    private static bool ValidId(string id) => Regex.IsMatch(id, @"^\d{8}-\d{6}-[a-f0-9]{12}$", RegexOptions.CultureInvariant);
    internal static string RunFolder(string sessionFolder, string id)
    {
        if (!ValidId(id)) throw new InvalidDataException("Ungültige Auswertungskennung.");
        return Path.Combine(RunsFolder(sessionFolder), id);
    }
    public static NotesResult Load(string sessionFolder, string id)
    {
        var folder = RunFolder(sessionFolder, id);
        var run = JsonData.Read<NotesRun>(Path.Combine(folder, "run.json")) ?? throw new InvalidDataException("Auswertung fehlt.");
        var original = JsonData.Read<TranscriptSnapshot>(Path.Combine(folder, "source.json")) ?? throw new InvalidDataException("Originaltranskript fehlt.");
        original.Validate();
        if (run.Id != id || run.SourceHash != original.Hash) throw new InvalidDataException("Auswertung und Originaltranskript gehören nicht zusammen.");
        if (run.Template is not null) return TemplateNotesService.Load(folder, run, original);
        var notes = NotesValidation.Parse(File.ReadAllText(Path.Combine(folder, "result.json")), original.Sources.ToDictionary(s => s.Id), 100000);
        return new(run, original, notes, folder);
    }
    public static async Task<NotesResult> GenerateAsync(string sessionFolder, TranscriptSnapshot snapshot, NotesConnection connection,
        bool cloudTextConsent, string? resumeId, IProgress<string>? progress, CancellationToken token, HttpMessageHandler? testHandler = null)
    {
        // Consent and destination validation happen before creating a run or opening a network client.
        snapshot = snapshot with { Sources = snapshot.Sources?.ToList() ?? [] };
        snapshot.Validate();
        if (string.IsNullOrWhiteSpace(connection.Model)) throw new InvalidOperationException("Bitte ein Textmodell auswählen.");
        using var client = new TextModelClient(connection, cloudTextConsent, testHandler);
        Directory.CreateDirectory(RunsFolder(sessionFolder));
        using var lease = new FileStream(Path.Combine(RunsFolder(sessionFolder), "processing.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        var batches = Batches(snapshot);
        var id = resumeId ?? $"{DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}"[..28];
        var folder = RunFolder(sessionFolder, id); Directory.CreateDirectory(folder);
        NotesRun run;
        if (resumeId is null)
        {
            run = new() { Id = id, SourceHash = snapshot.Hash, ConnectionFingerprint = connection.Fingerprint, Endpoint = connection.BaseUri().AbsoluteUri, Model = connection.Model, Mode = connection.Mode, Batches = batches.Count };
            JsonData.Save(Path.Combine(folder, "source.json"), snapshot);
        }
        else
        {
            run = JsonData.Read<NotesRun>(Path.Combine(folder, "run.json")) ?? throw new InvalidDataException("Gesicherter Arbeitsstand fehlt.");
            if (run.SourceHash != snapshot.Hash || run.ConnectionFingerprint != connection.Fingerprint || run.Batches != batches.Count)
                throw new InvalidOperationException("Transkript, Modell oder Anbieter haben sich geändert. Bitte eine neue Auswertung starten.");
            var stored = JsonData.Read<TranscriptSnapshot>(Path.Combine(folder, "source.json")) ?? throw new InvalidDataException("Der gesicherte Originaltranskriptstand fehlt.");
            stored.Validate();
            if (stored.Hash != snapshot.Hash) throw new InvalidDataException("Der gesicherte Originaltranskriptstand passt nicht zum Arbeitsstand.");
            snapshot = stored;
            if (run.Status == "complete") return Load(sessionFolder, id);
        }
        run.Status = "working"; run.Error = null; SaveRun();
        try
        {
            var all = new MeetingNotes();
            for (var i = 0; i < batches.Count; i++)
            {
                token.ThrowIfCancellationRequested(); progress?.Report($"Abschnitt {i + 1} von {batches.Count} auswerten …");
                var allowed = batches[i].ToDictionary(s => s.Id); var path = Path.Combine(folder, $"map-{i:D5}.json"); MeetingNotes notes;
                if (File.Exists(path))
                {
                    notes = NotesValidation.Parse(File.ReadAllText(path), allowed);
                    if (notes.Summary.Count > 3 || notes.Decisions.Count > 30 || notes.Todos.Count > 30) throw new InvalidDataException("Gesicherter Abschnitt überschreitet die Ergebnisgrenzen.");
                }
                else
                {
                    var input = JsonSerializer.Serialize(new { transcript = batches[i].Select(s => new { sourceId = s.Id, text = s.Text }) }, WireJson);
                    notes = NotesValidation.Parse(await client.GenerateAsync(input, false, 3, token), allowed);
                    if (notes.Summary.Count > 3) throw new InvalidDataException("Die Abschnittszusammenfassung ist zu lang.");
                    JsonData.Save(path, notes); // Durable before progress is acknowledged.
                }
                all.Summary.AddRange(notes.Summary); all.Decisions.AddRange(notes.Decisions); all.Todos.AddRange(notes.Todos);
                run.CompletedBatches = i + 1; SaveRun();
            }
            all.Summary = Dedupe(all.Summary); all.Decisions = Dedupe(all.Decisions); all.Todos = Dedupe(all.Todos);
            var round = 0;
            while (all.Summary.Count > 8)
            {
                token.ThrowIfCancellationRequested(); round++;
                var reduced = new List<MeetingFact>();
                // Pairwise reduction keeps requests bounded even for very long meetings.
                for (var i = 0; i < all.Summary.Count; i += 2)
                {
                    if (i + 1 == all.Summary.Count) { reduced.Add(all.Summary[i]); continue; }
                    progress?.Report($"Gesamtzusammenfassung verdichten · Durchgang {round} …");
                    var pair = all.Summary.Skip(i).Take(2).ToList();
                    var input = JsonSerializer.Serialize(new { verifiedSummary = pair }, WireJson);
                    var cacheKey = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(input)))[..20];
                    var path = Path.Combine(folder, "reduce-" + cacheKey + ".json");
                    var allowedIds = pair.SelectMany(p => p.Evidence).Select(e => e.SourceId).ToHashSet(StringComparer.Ordinal);
                    var allowed = snapshot.Sources.Where(s => allowedIds.Contains(s.Id)).ToDictionary(s => s.Id);
                    var notes = NotesValidation.Parse(File.Exists(path) ? File.ReadAllText(path) : await client.GenerateAsync(input, true, 1, token), allowed);
                    NotesValidation.Validate(notes, allowed);
                    if (notes.Summary.Count != 1 || notes.Decisions.Count != 0 || notes.Todos.Count != 0) throw new InvalidDataException("Die Gesamtzusammenfassung entspricht nicht dem verlangten Format.");
                    // A reducer may only cite evidence it was actually given, never another part of a source.
                    var quotes = pair.SelectMany(p => p.Evidence).ToHashSet();
                    if (notes.Summary[0].Evidence.Any(e => !quotes.Contains(e))) throw new InvalidDataException("Die Gesamtzusammenfassung verwendet einen nicht mitgelieferten Beleg.");
                    JsonData.Save(path, notes); reduced.Add(notes.Summary[0]);
                }
                all.Summary = reduced;
            }
            token.ThrowIfCancellationRequested(); NotesValidation.Validate(all, snapshot.Sources.ToDictionary(s => s.Id));
            JsonData.Save(Path.Combine(folder, "result.json"), all);
            Export(folder, snapshot, all, run);
            run.Status = "complete"; SaveRun();
            return new(run, snapshot, all, folder);
        }
        catch (Exception error)
        {
            run.Status = "paused"; run.Error = error is OperationCanceledException ? "Pausiert; gesicherte Abschnitte bleiben erhalten." : error.Message;
            SaveRun(); throw;
        }
        void SaveRun() => JsonData.Save(Path.Combine(folder, "run.json"), run);
    }
    internal static List<MeetingFact> Dedupe(List<MeetingFact> facts)
    {
        var result = new List<MeetingFact>();
        foreach (var group in facts.GroupBy(f => (f.Text.Trim(), f.Owner, f.Due)))
        {
            var first = group.First(); result.Add(new() { Text = first.Text, Owner = first.Owner, Due = first.Due, Evidence = group.SelectMany(f => f.Evidence).Distinct().Take(6).ToList() });
        }
        return result;
    }
    private static string Track(TranscriptSource source) => source.Track == "system" ? "Computerton" : "Mikrofon";
    private static string Markdown(string value)
    {
        var output = new StringBuilder();
        foreach (var c in value)
        {
            if (c == '<') output.Append("&lt;");
            else if (c == '&') output.Append("&amp;");
            else { if ("\\`*_{}[]()#+-.!|>".Contains(c)) output.Append('\\'); output.Append(c); }
        }
        return output.ToString();
    }
    public static string SourceLabel(TranscriptSource source) => $"{ClockTime.Format(source.StartMs)}–{ClockTime.Format(source.EndMs)} · {Track(source)} · {source.Id}";
    private static void Export(string folder, TranscriptSnapshot snapshot, MeetingNotes notes, NotesRun run)
        => ExportSections(folder, snapshot, new NotesResult(run, snapshot, notes, folder).AllSections, run);
    internal static void ExportSections(string folder, TranscriptSnapshot snapshot, List<NotesSection> sections, NotesRun run)
    {
        var date = snapshot.StartedAt == default ? "Datum nicht gespeichert" : $"Meeting vom {snapshot.StartedAt:dd.MM.yyyy HH:mm zzz}";
        var md = new StringBuilder($"# {Markdown(snapshot.Title.Replace('\r', ' ').Replace('\n', ' '))}\n\n{date}\n\nKI-Entwurf · {(run.Mode == TextProcessingMode.Local ? "lokal" : "Cloud, ausschließlich Text")} · Modell: {Markdown(run.Model)}\n\nBelege wurden auf vorhandene Quellen und wörtliche Zitate geprüft. Die inhaltliche Deutung bleibt zu prüfen. Zeiten bezeichnen Aufnahmeabschnitte, keine exakten Wortzeiten. Relative Termine beziehen sich auf das Meetingdatum.\n\n");
        var html = new StringBuilder("<!doctype html><html lang=\"de\"><meta charset=\"utf-8\"><title>Meetingnotizen</title><style>body{font:16px/1.6 system-ui;max-width:950px;margin:40px auto;padding:0 20px;color:#172b25}a{color:#165d48}blockquote{border-left:3px solid #ccc;padding-left:16px}small{color:#56665e}</style><body>");
        string H(string value) => WebUtility.HtmlEncode(value);
        html.Append($"<h1>{H(snapshot.Title)}</h1><p>{H(date)}</p><p>KI-Entwurf. Quellen und wörtliche Zitate technisch geprüft; inhaltliche Deutung bitte prüfen. Zeitangaben markieren Aufnahmeabschnitte. Relative Termine beziehen sich auf das Meetingdatum.</p>");
        foreach (var section in sections)
        {
            var title = section.Name; var facts = section.Items;
            md.AppendLine("## " + Markdown(title.Replace('\r', ' ').Replace('\n', ' ')) + "\n"); html.Append($"<h2>{H(title)}</h2><ul>");
            if (facts.Count == 0) { md.AppendLine("Keine ausdrücklich belegt.\n"); html.Append("<li>Keine ausdrücklich belegt.</li>"); }
            foreach (var fact in facts)
            {
                var detail = section.Kind == AnalysisKind.Tasks ? $" · Zuständig: {fact.Owner ?? "nicht genannt"} · Termin: {fact.Due ?? "nicht genannt"}" : "";
                md.Append("- ").Append(Markdown(fact.Text.Replace('\r', ' ').Replace('\n', ' ') + detail));
                html.Append($"<li>{H(fact.Text + detail)}<br>");
                foreach (var evidence in fact.Evidence)
                {
                    md.Append($" [{evidence.SourceId}](source.md#{evidence.SourceId.ToLowerInvariant()})");
                    html.Append($"<a href=\"#{evidence.SourceId}\">{evidence.SourceId}</a> <small>„{H(evidence.Quote)}“</small><br>");
                }
                md.AppendLine(); html.Append("</li>");
            }
            md.AppendLine(); html.Append("</ul>");
        }
        var sourceMd = new StringBuilder("# Unveränderter Transkriptstand der Auswertung\n\n");
        html.Append("<h2>Originalstellen</h2>");
        foreach (var source in snapshot.Sources)
        {
            sourceMd.AppendLine($"<a id=\"{source.Id.ToLowerInvariant()}\"></a>\n\n## {SourceLabel(source)}\n\n{Markdown(source.Text)}\n");
            html.Append($"<section id=\"{source.Id}\"><h3>{H(SourceLabel(source))}</h3><blockquote>{H(source.Text).Replace("\n", "<br>")}</blockquote></section>");
        }
        html.Append("</body></html>");
        JsonData.SaveText(Path.Combine(folder, "notes.md"), md.ToString());
        JsonData.SaveText(Path.Combine(folder, "source.md"), sourceMd.ToString());
        JsonData.SaveText(Path.Combine(folder, "notes.html"), html.ToString());
    }
}
