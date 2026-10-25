using System.IO;
using System.Text.Json;

namespace HandyMeeting;

public static class ModelPaths
{
    public static string Resolve(HandyModel model, string handyExecutable)
    {
        if (!model.SupportsStreaming) throw new InvalidOperationException("Bitte ein installiertes Streaming-Modell auswählen.");
        if (string.IsNullOrWhiteSpace(model.Filename) || model.Filename != Path.GetFileName(model.Filename)) throw new InvalidDataException("Ungültiger Modell-Dateiname.");
        var candidates = new List<string>();
        if (model.Source.ValueKind == JsonValueKind.Object && model.Source.TryGetProperty("HuggingFace", out var hub))
        {
            var repo = hub.GetProperty("repo_id").GetString() ?? "";
            var revision = hub.GetProperty("revision").GetString() ?? "";
            if (repo.Contains("..") || repo.Contains('\\') || revision.Contains("..") || revision.Contains('/') || revision.Contains('\\')) throw new InvalidDataException("Ungültiger Modellcache-Verweis.");
            var cache = Environment.GetEnvironmentVariable("HF_HUB_CACHE");
            if (string.IsNullOrWhiteSpace(cache))
            {
                var hfHome = Environment.GetEnvironmentVariable("HF_HOME");
                cache = string.IsNullOrWhiteSpace(hfHome) ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".cache", "huggingface", "hub") : Path.Combine(hfHome, "hub");
            }
            var snapshots = Path.Combine(cache, "models--" + repo.Replace("/", "--"), "snapshots");
            candidates.Add(Path.Combine(snapshots, revision, model.Filename));
            if (Directory.Exists(snapshots)) candidates.AddRange(Directory.EnumerateDirectories(snapshots).Select(folder => Path.Combine(folder, model.Filename)));
        }
        var localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var roaming = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        foreach (var folder in new[] { Path.Combine(Path.GetDirectoryName(handyExecutable)!, "Data", "models"), Path.Combine(roaming, "com.pais.handy", "models"), Path.Combine(localAppData, "com.pais.handy", "models"), Path.Combine(roaming, "Handy", "models") })
            candidates.Add(Path.Combine(folder, model.Filename));
        var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        foreach (var folder in new[] { Path.Combine(home, "Library/Application Support/com.pais.handy/models"), Path.Combine(home, ".local/share/com.pais.handy/models"), Path.Combine(home, ".config/com.pais.handy/models") })
            candidates.Add(Path.Combine(folder, model.Filename));
        return candidates.FirstOrDefault(File.Exists) ?? throw new FileNotFoundException("Die installierte Streaming-Modelldatei wurde nicht gefunden. Bitte das Modell in Handy prüfen.", model.Filename);
    }
}
