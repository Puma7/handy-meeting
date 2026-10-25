using System.Buffers.Binary;
using System.Diagnostics;
using System.Security.Cryptography;

namespace HandyMeeting;

public static class PortableCaptureChecks
{
    public sealed record ConfirmedPart(string File, string Source, long Samples, string Hash);
    public sealed record Ready(string Manifest, List<ConfirmedPart> Parts);
    public static async Task SyntheticSourceAsync(string[] args)
    {
        var level = (short)(args.Any(a => a.Contains("microphone", StringComparison.Ordinal)) ? 2000 : 1000);
        var bytes = new byte[3200]; for (var i = 0; i < bytes.Length; i += 2) BinaryPrimitives.WriteInt16LittleEndian(bytes.AsSpan(i), level);
        using var output = Console.OpenStandardOutput();
        for (var i = 0; i < 600; i++) { await output.WriteAsync(bytes); await output.FlushAsync(); await Task.Delay(100); }
    }
    public static async Task HostAsync(string root)
    {
        var session = SessionStore.Create(Path.Combine(root, "sessions"), "Synthetic capture crash", "fixture", new("synthetic-system", "Synthetic system"), new("synthetic-microphone", "Synthetic microphone"));
        using var lease = session.AcquireLease(); await using var capture = new PortableCapture(session);
        await capture.StartAsync(new("synthetic-system", "Synthetic system"), new("synthetic-microphone", "Synthetic microphone"));
        while (session.SavedAudioMs < 2000) await Task.Delay(50);
        var parts = session.PartsSnapshot();
        foreach (var part in parts) session.PersistText(part, "fixture", new() { Full = "Confirmed " + part.Source, Committed = "Confirmed " + part.Source }, part.SampleCount, false);
        session.SetStatus("recording");
        var confirmed = parts.Select(p => new ConfirmedPart(p.File, p.Source, p.SampleCount, Prefix(session.AudioPath(p), p.SampleCount))).ToList();
        JsonData.Save(Path.Combine(root, "ready.json"), new Ready(session.ManifestPath, confirmed));
        await Task.Delay(Timeout.Infinite);
    }
    private static string Prefix(string path, long samples)
    {
        using var input = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite); input.Position = 44;
        var bytes = new byte[checked((int)samples * 2)]; input.ReadExactly(bytes); return Convert.ToHexString(SHA256.HashData(bytes));
    }
    public static async Task<int> RunAsync(string root)
    {
        if (!OperatingSystem.IsLinux()) throw new PlatformNotSupportedException("This capture-process fixture targets Linux.");
        Directory.CreateDirectory(root);
        var helpers = Path.Combine(root, "fake-pulse"); Directory.CreateDirectory(helpers);
        JsonData.SaveText(Path.Combine(helpers, "pactl"), "#!/bin/sh\nprintf 'Synthetic audio service\\n'\n");
        JsonData.SaveText(Path.Combine(helpers, "parec"), "#!/bin/sh\nif [ \"$1\" = --version ]; then printf 'Synthetic parec\\n'; exit 0; fi\nexec \"$HANDYMEETING_TEST_SOURCE\" --synthetic-source \"$@\"\n");
        foreach (var file in Directory.EnumerateFiles(helpers)) File.SetUnixFileMode(file, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
        info.ArgumentList.Add("--portable-capture-host"); info.ArgumentList.Add(root);
        info.Environment["PATH"] = helpers + Path.PathSeparator + info.Environment["PATH"];
        info.Environment["HANDYMEETING_TEST_SOURCE"] = Environment.ProcessPath;
        using var child = Process.Start(info)!;
        try
        {
            var wait = Stopwatch.StartNew();
            while (!File.Exists(Path.Combine(root, "ready.json")))
            {
                if (child.HasExited || wait.Elapsed.TotalSeconds > 25) throw new InvalidOperationException("Synthetic capture did not produce a confirmed checkpoint.");
                await Task.Delay(25);
            }
            var ready = JsonData.Read<Ready>(Path.Combine(root, "ready.json"))!; child.Kill(true); await child.WaitForExitAsync();
            var recovered = SessionStore.Open(ready.Manifest);
            if (ready.Parts.Count != 2 || ready.Parts.Any(p => Prefix(Path.Combine(recovered.Folder, "audio", p.File), p.Samples) != p.Hash)
                || !recovered.Data.Parts.Any(p => p.Text == "Confirmed system") || !recovered.Data.Parts.Any(p => p.Text == "Confirmed microphone")) throw new InvalidOperationException("Confirmed capture data was lost.");
            JsonData.Save(Path.Combine(root, "capture-checks.json"), new { Passed = true, SyntheticOnly = true, RealDevices = false, ProcessKilled = true, ConfirmedAudioPrefixesEqual = true, BothTranscriptCheckpointsRecovered = true, Confirmed = ready.Parts }); return 0;
        }
        finally { if (!child.HasExited) { child.Kill(true); await child.WaitForExitAsync(); } }
    }
}
