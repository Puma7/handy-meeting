using System.IO;
using System.Security.Cryptography;
using System.Text.Json;

namespace HandyMeeting;

public enum AnalysisKind { Analysis, Summary, Decisions, Tasks }
public sealed class AnalysisStep
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string Name { get; set; } = "Analyse";
    public string Prompt { get; set; } = "Arbeite die wichtigsten belegbaren Erkenntnisse heraus.";
    public AnalysisKind Kind { get; set; }
}
public sealed class AnalysisTemplate
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string Name { get; set; } = "Standard";
    public List<AnalysisStep> Steps { get; set; } = [];
    public string Hash => Convert.ToHexString(SHA256.HashData(JsonSerializer.SerializeToUtf8Bytes(new { Protocol = NotesProtocol.StepPromptVersion, Name, Steps })));
    public AnalysisTemplate Copy() => JsonSerializer.Deserialize<AnalysisTemplate>(JsonSerializer.Serialize(this))!;
    public void Validate()
    {
        if (!Guid.TryParseExact(Id, "N", out _) || string.IsNullOrWhiteSpace(Name) || Name.Length > 100 || Steps is null || Steps.Count == 0
            || Steps.Any(s => s is null || !Guid.TryParseExact(s.Id, "N", out _) || string.IsNullOrWhiteSpace(s.Name) || s.Name.Length > 100 || string.IsNullOrWhiteSpace(s.Prompt) || s.Prompt.Length > 8000 || !Enum.IsDefined(s.Kind))
            || Steps.Select(s => s.Id).Distinct(StringComparer.Ordinal).Count() != Steps.Count)
            throw new InvalidDataException("Die Vorlage benötigt einen Namen und gültige Schritte mit eindeutigen Kennungen, Namen und Prompts (höchstens 8000 Zeichen). / Invalid template.");
    }
    public override string ToString() => Name;
    public static AnalysisTemplate Default() => new()
    {
        Name = "Standard", Steps =
        [
            new() { Name = "Zusammenfassung", Kind = AnalysisKind.Summary, Prompt = "Fasse das Gespräch auf Deutsch knapp und verständlich zusammen. Hebe Themen und Ergebnisse hervor." },
            new() { Name = "Entscheidungen", Kind = AnalysisKind.Decisions, Prompt = "Extrahiere ausschließlich ausdrücklich getroffene Entscheidungen. Wünsche, Vorschläge und unverbindliche Überlegungen sind keine Entscheidungen. Auch Aufgabenübernahmen und Aussagen über bestehende oder fortbestehende Zustände sind keine Beschlüsse. Prüfe für jeden Eintrag: Belegt das Zitat ausdrücklich eine getroffene Auswahl oder Änderung? Wenn es nur beschreibt, was ist oder bleibt, lasse es weg. Ohne ausdrücklichen Beschluss bleibt items leer." },
            new() { Name = "To-dos", Kind = AnalysisKind.Tasks, Prompt = "Extrahiere ausschließlich ausdrücklich zugesagte oder zugewiesene Aufgaben mit Zuständigen und Terminen, soweit genannt. Unverbindliche Vorschläge sind keine Aufgaben." }
        ]
    };
}
public sealed record NotesSection(string Id, string Name, AnalysisKind Kind, List<MeetingFact> Items);
public sealed class TemplateLibrary
{
    public List<AnalysisTemplate> Templates { get; set; } = [AnalysisTemplate.Default()];
    public string? SelectedId { get; set; }
    public static string FilePath => Path.Combine(UserSettings.DataRoot, "analysis-templates.json");
    public static TemplateLibrary Load()
    {
        if (!File.Exists(FilePath)) { var initial = new TemplateLibrary(); initial.Save(); return initial; }
        var library = JsonData.Read<TemplateLibrary>(FilePath) ?? throw new InvalidDataException("Vorlagen sind nicht lesbar. Die vorhandene Datei bleibt erhalten. / Cannot read templates.");
        library.Validate(); return library;
    }
    public void Validate()
    {
        if (Templates is null || Templates.Count == 0 || Templates.Select(t => t.Id).Distinct().Count() != Templates.Count) throw new InvalidDataException("Ungültige Vorlagenbibliothek. / Invalid template library.");
        foreach (var template in Templates) template.Validate();
    }
    public void Save() { Validate(); JsonData.Save(FilePath, this); }
}

public static class SectionValidation
{
    public static List<MeetingFact> Parse(string json, IReadOnlyDictionary<string, TranscriptSource> sources, AnalysisKind kind, int max = 30)
    {
        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.ValueKind != JsonValueKind.Object || doc.RootElement.EnumerateObject().Count() != 1 || !doc.RootElement.TryGetProperty("items", out var items)) throw new InvalidDataException("Ungültige Schrittantwort. / Invalid step response.");
        // Reuse the strict fact/evidence validator, including duplicate-field and literal quote checks.
        var wrapper = "{\"summary\":[],\"decisions\":[],\"todos\":" + items.GetRawText() + "}";
        var result = NotesValidation.Parse(wrapper, sources, max).Todos;
        if (kind != AnalysisKind.Tasks && result.Any(f => f.Owner is not null || f.Due is not null)) throw new InvalidDataException("Zuständige und Termine gehören ausschließlich zu Aufgaben. / Task metadata outside tasks.");
        return result;
    }
    public static void Validate(NotesSection section, IReadOnlyDictionary<string, TranscriptSource> sources)
        => Parse(JsonSerializer.Serialize(new { items = section.Items }), sources, section.Kind, 100000);
}
