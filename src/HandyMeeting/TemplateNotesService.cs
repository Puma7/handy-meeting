using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace HandyMeeting;

public static class TemplateNotesService
{
    private static readonly JsonSerializerOptions Wire = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping };
    internal static NotesResult Load(string folder, NotesRun run, TranscriptSnapshot snapshot)
    {
        var template = run.Template!; template.Validate();
        if (run.TemplateHash != template.Hash || run.PromptVersion != NotesProtocol.StepPromptVersion) throw new InvalidDataException("Die gespeicherte Vorlage oder Promptversion wurde verändert. / Stored template or prompt version changed.");
        var sections = JsonData.Read<List<NotesSection>>(Path.Combine(folder, "sections.json")) ?? throw new InvalidDataException("Ergebnisse fehlen. / Missing results.");
        if (sections.Count != template.Steps.Count || sections.Where((s, i) => s.Id != template.Steps[i].Id || s.Name != template.Steps[i].Name || s.Kind != template.Steps[i].Kind).Any()) throw new InvalidDataException("Ergebnis und Vorlage passen nicht zusammen. / Template mismatch.");
        var sources = snapshot.Sources.ToDictionary(s => s.Id);
        foreach (var section in sections) SectionValidation.Validate(section, sources);
        return new(run, snapshot, new(), folder) { Sections = sections };
    }
    public static async Task<NotesResult> GenerateAsync(string sessionFolder, TranscriptSnapshot snapshot, AnalysisTemplate template, NotesConnection connection,
        bool consent, string? resumeId, IProgress<string>? progress, CancellationToken token, System.Net.Http.HttpMessageHandler? testHandler = null)
    {
        template = template.Copy(); template.Validate(); snapshot = snapshot with { Sources = snapshot.Sources.ToList() }; snapshot.Validate();
        using var client = new TextModelClient(connection, consent, testHandler);
        if (string.IsNullOrWhiteSpace(connection.Model)) throw new InvalidOperationException("Bitte ein Textmodell auswählen. / Choose a text model.");
        Directory.CreateDirectory(MeetingNotesService.RunsFolder(sessionFolder));
        using var lease = new FileStream(Path.Combine(MeetingNotesService.RunsFolder(sessionFolder), "processing.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        var batches = MeetingNotesService.Batches(snapshot);
        var id = resumeId ?? $"{DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}"[..28];
        var folder = MeetingNotesService.RunFolder(sessionFolder, id);
        NotesRun run;
        if (resumeId is not null)
        {
            run = JsonData.Read<NotesRun>(Path.Combine(folder, "run.json")) ?? throw new InvalidDataException("Arbeitsstand fehlt. / Missing checkpoint.");
            if (run.Id != id || run.PromptVersion != NotesProtocol.StepPromptVersion || run.SourceHash != snapshot.Hash || run.ConnectionFingerprint != connection.Fingerprint || run.TemplateHash != template.Hash || run.Template?.Hash != template.Hash || run.Batches != batches.Count * template.Steps.Count)
                throw new InvalidOperationException("Transkript, Vorlage, Prompt, Modell oder Anbieter geändert. Bitte eine neue Auswertung starten. / Settings changed; start a new analysis.");
            var stored = JsonData.Read<TranscriptSnapshot>(Path.Combine(folder, "source.json")) ?? throw new InvalidDataException("Original fehlt. / Missing original.");
            stored.Validate(); if (stored.Hash != snapshot.Hash) throw new InvalidDataException("Originalstand verändert. / Original changed.");
            snapshot = stored; template = run.Template!;
            if (run.Status == "complete") return MeetingNotesService.Load(sessionFolder, id);
        }
        else
        {
            Directory.CreateDirectory(folder);
            run = new() { Id = id, SourceHash = snapshot.Hash, Model = connection.Model, Mode = connection.Mode, Endpoint = connection.BaseUri().AbsoluteUri, ConnectionFingerprint = connection.Fingerprint,
                Template = template, TemplateHash = template.Hash, PromptVersion = NotesProtocol.StepPromptVersion, Batches = batches.Count * template.Steps.Count };
            JsonData.Save(Path.Combine(folder, "source.json"), snapshot);
        }
        run.Status = "working"; run.Error = null; run.CompletedBatches = 0; SaveRun();
        try
        {
            var sections = new List<NotesSection>();
            foreach (var step in template.Steps)
            {
                var facts = new List<MeetingFact>();
                for (var i = 0; i < batches.Count; i++)
                {
                    token.ThrowIfCancellationRequested(); progress?.Report($"{step.Name} · {i + 1}/{batches.Count} · {run.CompletedBatches}/{run.Batches}");
                    var allowed = batches[i].ToDictionary(s => s.Id);
                    var path = Path.Combine(folder, $"step-{step.Id}-{i:D5}.json");
                    var input = JsonSerializer.Serialize(new { transcript = batches[i].Select(s => new { sourceId = s.Id, track = s.Track, text = s.Text }), scope = "This is one transcript portion; no whole-meeting speaker statistics can be inferred." }, Wire);
                    var json = File.Exists(path) ? File.ReadAllText(path) : await client.GenerateStepAsync(input, step, false, token);
                    var items = SectionValidation.Parse(json, allowed, step.Kind);
                    if (!File.Exists(path)) JsonData.SaveText(path, json);
                    facts.AddRange(items); run.CompletedBatches++; SaveRun();
                }
                facts = MeetingNotesService.Dedupe(facts);
                // Only summaries are condensed. Decisions, tasks and custom analyses keep all validated entries.
                while (step.Kind == AnalysisKind.Summary && facts.Count > 8)
                {
                    progress?.Report($"{step.Name} · {(Ui.English ? "Condensing overview" : "Gesamtüberblick verdichten")} …");
                    var reduced = new List<MeetingFact>();
                    for (var i = 0; i < facts.Count; i += 2)
                    {
                        token.ThrowIfCancellationRequested();
                        if (i + 1 == facts.Count) { reduced.Add(facts[i]); continue; }
                        var pair = facts.Skip(i).Take(2).ToList();
                        var input = JsonSerializer.Serialize(new { verifiedSummary = pair }, Wire);
                        var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(input)));
                        var path = Path.Combine(folder, $"reduce-{step.Id}-{hash}.json");
                        var ids = pair.SelectMany(f => f.Evidence).Select(e => e.SourceId).ToHashSet();
                        var allowed = snapshot.Sources.Where(s => ids.Contains(s.Id)).ToDictionary(s => s.Id);
                        var json = File.Exists(path) ? File.ReadAllText(path) : await client.GenerateStepAsync(input, step, true, token);
                        var items = SectionValidation.Parse(json, allowed, step.Kind);
                        var quotes = pair.SelectMany(f => f.Evidence).ToHashSet();
                        if (items.Count != 1 || items[0].Evidence.Any(e => !quotes.Contains(e))) throw new InvalidDataException("Ungültige Verdichtung. / Invalid condensation.");
                        if (!File.Exists(path)) JsonData.SaveText(path, json);
                        reduced.Add(items[0]);
                    }
                    facts = reduced;
                }
                var section = new NotesSection(step.Id, step.Name, step.Kind, facts);
                SectionValidation.Validate(section, snapshot.Sources.ToDictionary(s => s.Id)); sections.Add(section);
            }
            token.ThrowIfCancellationRequested(); JsonData.Save(Path.Combine(folder, "sections.json"), sections);
            MeetingNotesService.ExportSections(folder, snapshot, sections, run);
            run.Status = "complete"; SaveRun();
            return new(run, snapshot, new(), folder) { Sections = sections };
        }
        catch (Exception error) { run.Status = "paused"; run.Error = error is OperationCanceledException ? "Pausiert / Paused" : error.Message; SaveRun(); throw; }
        void SaveRun() => JsonData.Save(Path.Combine(folder, "run.json"), run);
    }
    public static string PlainText(NotesResult result)
    {
        var output = new StringBuilder(result.Original.Title + $"\n{result.Original.StartedAt:yyyy-MM-dd HH:mm zzz}\n" + Ui.T("KI-Entwurf · Inhalt und Belege prüfen.") + "\n");
        foreach (var section in result.AllSections)
        {
            output.AppendLine().AppendLine(section.Name);
            if (section.Items.Count == 0) output.AppendLine(Ui.T("Keine belegten Einträge."));
            foreach (var fact in section.Items)
            {
                output.Append("• ").Append(fact.Text);
                if (section.Kind == AnalysisKind.Tasks) output.Append($" · {Ui.T("Zuständig")}: {fact.Owner ?? Ui.T("nicht genannt")} · {Ui.T("Termin")}: {fact.Due ?? Ui.T("nicht genannt")}");
                output.AppendLine(" " + string.Join(" ", fact.Evidence.Select(e => "[" + e.SourceId + "]")));
                foreach (var evidence in fact.Evidence) output.AppendLine($"  [{evidence.SourceId}] „{evidence.Quote}“");
            }
        }
        return output.ToString();
    }
}
