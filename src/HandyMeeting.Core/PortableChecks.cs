using System.Buffers.Binary;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;

namespace HandyMeeting;

public static class PortableChecks
{
    public static async Task<int> RunAsync(string root)
    {
        var checks = new List<string>();
        void Check(bool condition, string name) { if (!condition) throw new InvalidOperationException(name); checks.Add(name); }
        try
        {
            var session = SessionStore.Create(Path.Combine(root, "Sessions"), "Portable synthetic check", "fixture", new("fake-system", "Synthetic system"), new("fake-mic", "Synthetic microphone"), 1);
            foreach (var (track, amplitude) in new[] { ("system", (short)1000), ("microphone", (short)2000) })
            {
                using var pcm = new PcmTimeline(Path.Combine(session.Folder, "audio"), track, 1, session.AddPart, p => session.AudioCheckpoint(p, false));
                var bytes = new byte[32000]; for (var i = 0; i < 16000; i++) BinaryPrimitives.WriteInt16LittleEndian(bytes.AsSpan(i * 2), amplitude);
                pcm.Append(bytes, 0); pcm.Append(bytes, 16000); pcm.Complete(32000);
            }
            var audioBefore = session.PartsSnapshot().ToDictionary(p => p.File, p => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(session.AudioPath(p)))));
            foreach (var part in session.PartsSnapshot()) session.PersistText(part, "fixture", new() { Full = "Portable original transcript " + part.Source, Committed = "Portable original transcript " + part.Source }, part.SampleCount, true);
            session.SetStatus("complete"); session.ExportSnapshot();
            var recovered = SessionStore.Open(session.ManifestPath);
            Check(recovered.Data.Parts.Count == 4 && recovered.Data.Parts.All(p => audioBefore[p.File] == Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(recovered.AudioPath(p))))), "Both synthetic PCM tracks survive recovery with identical SHA-256 values");
            Check(recovered.CopyTranscriptSnapshot().Contains("system") && recovered.CopyTranscriptSnapshot().Contains("microphone"), "Recovered transcript retains both tracks");
            var snapshot = TranscriptSnapshot.Create(recovered.Data); snapshot.Validate();
            Check(snapshot.Sources.Count == 4, "Portable source snapshots validate literal original text");
            var template = AnalysisTemplate.Default(); var copy = template.Copy(); copy.Steps[0].Prompt += " Changed.";
            Check(copy.Hash != template.Hash, "Prompt changes alter the immutable template fingerprint");
            var quoted = new NotesSection(template.Steps[0].Id, template.Steps[0].Name, template.Steps[0].Kind, [new() { Text = "Track recorded", Evidence = [new(snapshot.Sources[0].Id, snapshot.Sources[0].Text)] }]);
            SectionValidation.Validate(quoted, snapshot.Sources.ToDictionary(s => s.Id)); checks.Add("Portable notes retain mandatory verbatim evidence");
            var rejected = false;
            try { quoted.Items[0].Evidence[0] = new(snapshot.Sources[0].Id, "This is invented evidence."); SectionValidation.Validate(quoted, snapshot.Sources.ToDictionary(s => s.Id)); } catch (InvalidDataException) { rejected = true; }
            Check(rejected, "Invented evidence is rejected in the shared core");
            rejected = false; try { _ = new NotesConnection(TextProcessingMode.Local, "https://example.invalid/v1/", "model", "").BaseUri(); } catch (InvalidOperationException) { rejected = true; }
            Check(rejected, "Local text processing refuses remote endpoints");
            rejected = false; try { using var client = new TextModelClient(new(TextProcessingMode.Cloud, "https://example.invalid/v1/", "model", ""), false); } catch (InvalidOperationException) { rejected = true; }
            Check(rejected, "Cloud text processing requires explicit consent before opening transport");
            rejected = false; try { _ = NotesSettings.Protect("not-to-be-written"); } catch (PlatformNotSupportedException) { rejected = true; }
            Check(rejected, "Portable preview refuses plaintext key persistence");
            await CheckAgentAsync(root); checks.Add("Agent protocol discovers capabilities, refuses text without grant and rejects cloud actions");
            JsonData.Save(Path.Combine(root, "portable-checks.json"), new { Passed = checks.Count, Checks = checks, Platform = System.Runtime.InteropServices.RuntimeInformation.OSDescription, SyntheticOnly = true, RealDevicesTested = false, CloudTrafficSent = false });
            return 0;
        }
        catch (Exception e) { JsonData.Save(Path.Combine(root, "failure.json"), new { error = e.ToString() }); return 1; }
    }
    private static async Task CheckAgentAsync(string root)
    {
        var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true };
        info.ArgumentList.Add("--agent"); info.ArgumentList.Add("serve"); info.Environment["HANDYMEETING_DATA_DIR"] = root;
        using var process = Process.Start(info)!;
        foreach (var request in new[]
        {
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"capabilities\",\"params\":{}}",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"transcript.read\",\"params\":{\"sessionId\":\"bad\",\"offset\":0,\"limit\":1}}",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"cloud.generate\",\"params\":{}}"
        }) await process.StandardInput.WriteLineAsync(request);
        process.StandardInput.Close(); var output = await process.StandardOutput.ReadToEndAsync(); var error = await process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(15));
        var lines = output.Split('\n', StringSplitOptions.RemoveEmptyEntries); if (process.ExitCode != 0 || error.Length != 0 || lines.Length != 3) throw new InvalidOperationException("Agent protocol output corrupted.");
        using var capabilities = JsonDocument.Parse(lines[0]); using var denied = JsonDocument.Parse(lines[1]); using var cloud = JsonDocument.Parse(lines[2]);
        if (capabilities.RootElement.GetProperty("result").GetProperty("cloudExecution").GetBoolean() || !denied.RootElement.TryGetProperty("error", out _) || !cloud.RootElement.TryGetProperty("error", out _)) throw new InvalidOperationException("Agent consent boundary failed.");
    }
    public static async Task<int> NativeAsync(string root, string model, string wave)
    {
        try
        {
            var before = MemorySample.Read(); await using var worker = await StreamingProcess.StartAsync(model, true, CancellationToken.None);
            await worker.CallAsync(new() { Operation = "begin" }, CancellationToken.None);
            var samples = (new FileInfo(wave).Length - 44) / 2;
            for (long offset = 0; offset < samples; offset += 8000)
                await worker.CallAsync(new() { Operation = "feed", AudioPath = wave, OffsetSamples = offset, SampleCount = (int)Math.Min(8000, samples - offset) }, CancellationToken.None);
            var final = await worker.CallAsync(new() { Operation = "finalize" }, CancellationToken.None);
            if (string.IsNullOrWhiteSpace(final.Full)) throw new InvalidOperationException("Native portable stream returned no text.");
            JsonData.Save(Path.Combine(root, "native-check.json"), new { Passed = true, Backend = worker.Backend, final.Full, Before = before, After = MemorySample.Read(), SyntheticFile = true, RealDevice = false }); return 0;
        }
        catch (Exception e) { JsonData.Save(Path.Combine(root, "native-failure.json"), new { error = e.ToString() }); return 1; }
    }
}
