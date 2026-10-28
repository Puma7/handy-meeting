using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.Json;
using Microsoft.Win32;

namespace HandyMeeting;

public sealed class HandyClient(string executable)
{
    public static string? FindInstalled()
    {
        if (!OperatingSystem.IsWindows())
        {
            var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            var paths = OperatingSystem.IsMacOS()
                ? new[] { "/Applications/Handy.app/Contents/MacOS/Handy", "/Applications/Handy.app/Contents/MacOS/handy", Path.Combine(home, "Applications/Handy.app/Contents/MacOS/Handy"), Path.Combine(home, "Applications/Handy.app/Contents/MacOS/handy") }
                : (Environment.GetEnvironmentVariable("PATH") ?? "").Split(Path.PathSeparator).Where(p => !string.IsNullOrWhiteSpace(p)).Select(p => Path.Combine(p, "handy")).Concat(new[] { Path.Combine(home, "Applications/Handy.AppImage"), "/opt/Handy/handy", "/usr/bin/handy" }).ToArray();
            return paths.FirstOrDefault(File.Exists);
        }
        foreach (var process in Process.GetProcessesByName("handy"))
        {
            using (process)
                try { var path = process.MainModule?.FileName; if (path is not null && File.Exists(path)) return path; }
                catch (Exception) { /* Other users' or elevated processes may be inaccessible. */ }
        }
        var candidates = new List<string> { Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Handy", "handy.exe"), Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Handy", "handy.exe") };
        foreach (var hive in new[] { Registry.CurrentUser, Registry.LocalMachine })
        {
            using var uninstall = hive.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall");
            if (uninstall is null) continue;
            foreach (var name in uninstall.GetSubKeyNames())
            {
                using var app = uninstall.OpenSubKey(name);
                if (app?.GetValue("DisplayName") as string != "Handy") continue;
                if (app.GetValue("InstallLocation") is string folder) candidates.Add(Path.Combine(folder, "handy.exe"));
            }
        }
        return candidates.FirstOrDefault(File.Exists);
    }
    public async Task<List<HandyModel>> ModelsAsync(CancellationToken token)
    {
        var output = await RunAsync(["--list-models", "--json"], TimeSpan.FromSeconds(45), token);
        return (JsonSerializer.Deserialize<List<HandyModel>>(output, JsonData.Options) ?? []).Where(m => m.IsDownloaded).ToList();
    }
    public async Task<string> TranscribeAsync(string file, string modelId, CancellationToken token)
    {
        var output = await RunAsync(["--transcribe-file", Path.GetFullPath(file), "--model", modelId, "--json"], TimeSpan.FromMinutes(15), token);
        using var result = JsonDocument.Parse(output);
        if (!result.RootElement.TryGetProperty("text", out var text) || text.ValueKind != JsonValueKind.String) throw new InvalidDataException("Handy hat kein gültiges Text-Ergebnis zurückgegeben.");
        return text.GetString()?.Trim() ?? "";
    }
    private async Task<string> RunAsync(IEnumerable<string> arguments, TimeSpan timeout, CancellationToken token)
    {
        if (!File.Exists(executable)) throw new FileNotFoundException("Handy wurde nicht gefunden. Bitte handy.exe auswählen.", executable);
        var info = new ProcessStartInfo(executable) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true, StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8, WorkingDirectory = Path.GetDirectoryName(executable)! };
        foreach (var argument in arguments) info.ArgumentList.Add(argument);
        // No URL, network client or model download is involved. Handy owns inference and its own settings.
        using var process = new Process { StartInfo = info };
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(token); deadline.CancelAfter(timeout);
        process.Start();
        var stdout = process.StandardOutput.ReadToEndAsync();
        var stderr = process.StandardError.ReadToEndAsync();
        try
        {
            await process.WaitForExitAsync(deadline.Token);
            var output = await stdout; var error = await stderr;
            if (process.ExitCode != 0)
            {
                // Show a bounded diagnostic to the user; never persist Handy's potentially sensitive logs.
                var lastLines = string.Join(Environment.NewLine, error.Split('\n').TakeLast(6));
                throw new InvalidOperationException($"Handy meldet Fehler {process.ExitCode}. {lastLines}");
            }
            return output.Trim('\uFEFF', '\r', '\n', ' ');
        }
        catch (OperationCanceledException)
        {
            try { if (!process.HasExited) process.Kill(entireProcessTree: true); } catch (InvalidOperationException) { }
            await process.WaitForExitAsync();
            await Task.WhenAll(stdout, stderr);
            if (token.IsCancellationRequested) throw;
            throw new TimeoutException("Handy hat innerhalb des Zeitlimits nicht geantwortet. Die Audiodateien bleiben erhalten.");
        }
    }
}

public static class Transcription
{
    public static string RenderLive(SessionStore session)
    {
        var parts = session.RecentTextPartsSnapshot();
        var output = new StringBuilder();
        foreach (var part in parts)
        {
            output.AppendLine($"[{ClockTime.Format(part.StartMs)}] {(part.Source == "system" ? "Computerton" : "Mikrofon")}{(part.Status == "complete" ? "" : " · laufend")}");
            output.AppendLine(part.Text); output.AppendLine();
        }
        return output.Length > 0 ? output.ToString() : "Audio wird lokal gesichert. Das Streaming-Modell bereitet den ersten Text vor …";
    }
    public static async Task RunAsync(SessionStore session, HandyClient handy, string modelId, Action<string> progress, CancellationToken token)
    {
        session.Data.ModelId = modelId; session.Data.Status = "transcribing"; session.Save();
        var parts = session.Data.Parts.OrderBy(p => p.StartSample).ThenBy(p => p.Source).ToList();
        try
        {
            for (var i = 0; i < parts.Count; i++)
            {
                token.ThrowIfCancellationRequested(); var part = parts[i];
                if (part.Status == "complete" && part.ModelId == modelId) continue;
                var source = part.Source == "system" ? "Computerton" : "Mikrofon";
                progress($"{source} · Abschnitt {i + 1} von {parts.Count} · {ClockTime.Format(part.StartMs)}");
                part.Status = "transcribing"; part.Error = null; session.Save();
                // Digital silence never reaches ASR: it must not turn into hallucinated text.
                part.Text = part.HasSignal ? await handy.TranscribeAsync(session.AudioPath(part), modelId, token) : "";
                part.ModelId = modelId; part.Status = "complete"; session.Save(); Export(session);
            }
            session.Data.Status = "complete"; session.Save(); Export(session);
        }
        catch (Exception e)
        {
            foreach (var part in parts.Where(p => p.Status == "transcribing"))
            { part.Status = "pending"; part.Error = e is OperationCanceledException ? "Abgebrochen; kann erneut gestartet werden." : e.Message; }
            session.Data.Status = e is OperationCanceledException ? "recorded" : "error"; session.Save(); Export(session); throw;
        }
    }
    public static string Render(SessionStore session, bool markdown)
    {
        var output = new StringBuilder();
        string Literal(string text) => markdown ? MarkdownText.Escape(text) : text;
        output.AppendLine(markdown ? "# " + Literal(session.Data.Title.Replace('\r', ' ').Replace('\n', ' ')) : session.Data.Title);
        output.AppendLine($"{session.Data.StartedAt:dd.MM.yyyy HH:mm} · {ClockTime.Format(session.Data.DurationMs)} · Modell: {Literal(session.Data.ModelId)}");
        output.AppendLine("Zeitangaben markieren Aufnahmeabschnitte, keine exakten Wortzeiten.");
        if (session.Data.Status != "complete") output.AppendLine("UNVOLLSTÄNDIG: Es stehen noch Transkriptionsabschnitte aus.");
        foreach (var warning in session.Data.Warnings) output.AppendLine("HINWEIS: " + Literal(warning));
        foreach (var part in session.Data.Parts.OrderBy(p => p.StartSample).ThenBy(p => p.Source))
        {
            if (string.IsNullOrWhiteSpace(part.Text)) continue;
            if (part.Status != "complete" && part.ModelId != session.Data.ModelId) continue;
            output.AppendLine();
            var heading = $"[{ClockTime.Format(part.StartMs)}–{ClockTime.Format(part.EndMs)}] {(part.Source == "system" ? "Computerton" : "Mikrofon")}";
            output.AppendLine(markdown ? "## " + heading : heading); output.AppendLine(); output.AppendLine(Literal(part.Text));
        }
        return output.ToString();
    }
    public static void Export(SessionStore session)
    {
        JsonData.SaveText(Path.Combine(session.Folder, "transcript.md"), Render(session, true));
        JsonData.SaveText(Path.Combine(session.Folder, "transcript.txt"), Render(session, false));
        JsonData.Save(Path.Combine(session.Folder, "transcript.json"), session.Data);
    }
}
