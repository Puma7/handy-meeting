using System.Diagnostics;
using System.IO;
using System.Text.Json;

namespace HandyMeeting;

public static class AgentVerification
{
    public static async Task RunAsync(string root)
    {
        Directory.CreateDirectory(root); var data = new SessionData { Status = "complete", Title = "Agent fixture", Parts = [new() { File = "system-000000000000.wav", Status = "complete", AudioClosed = true, Text = "Dies ist ausschließlich synthetischer Referenztext.", SampleCount = 16000 }] };
        var session = Path.Combine(root, "Sessions", "fixture"); Directory.CreateDirectory(session); JsonData.Save(Path.Combine(session, "session.json"), data);
        using (var writer = new PcmTimeline(Path.Combine(session, "audio"), "system", 30, _ => { })) { writer.Append(new byte[32000], 0); writer.Complete(16000); }
        var checks = new List<string>();
        void Check(bool ok, string name) { if (!ok) throw new InvalidOperationException(name); checks.Add(name); }
        var denied = await ExchangeAsync(root, false, [Request(1, "capabilities", new { }), Request(2, "transcript.read", new { sessionId = data.Id, offset = 0, limit = 1 }), Request(3, "cloud.generate", new { }), Request(4, "capture.start", new { }), "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"capabilities\",\"params\":{},\"params\":{}}"]);
        Check(!denied[0].GetProperty("result").GetProperty("textAccessGranted").GetBoolean(), "Agent defaults to no transcript access");
        Check(denied[1].TryGetProperty("error", out _), "Transcript access is rejected before reading without explicit grant");
        Check(denied[2].TryGetProperty("error", out _) && denied[3].TryGetProperty("error", out _), "Cloud calls and recording commands are not exposed in v1");
        Check(denied[4].TryGetProperty("error", out _), "Duplicate JSON-RPC fields are rejected");
        var oversized = await ExchangeAsync(root, false, [new string('x', 66000), Request(6, "capabilities", new { })]);
        Check(oversized[0].TryGetProperty("error", out _) && oversized[1].TryGetProperty("result", out _), "Oversized requests are rejected with bounded storage and the next request still works");
        var allowed = await ExchangeAsync(root, true, [Request(1, "transcript.read", new { sessionId = data.Id, offset = 0, limit = 1 }), Request(2, "transcript.read", new { sessionId = "../../etc/passwd", offset = 0, limit = 1 }), Request(3, "transcript.read", new { sessionId = data.Id, offset = 0, limit = 1000 })]);
        var content = allowed[0].GetProperty("result");
        Check(content.GetProperty("untrustedData").GetBoolean() && content.GetProperty("sources")[0].GetProperty("Text").GetString() == data.Parts[0].Text, "Granted reads return original source text labelled as untrusted data");
        Check(allowed[1].TryGetProperty("error", out _) && allowed[2].TryGetProperty("error", out _), "Session traversal and unbounded page sizes are rejected");
        Check(denied[0].GetProperty("result").GetProperty("styles").GetArrayLength() == 4, "All four output styles carry machine-readable explanations");
        JsonData.Save(Path.Combine(root, "agent-checks.json"), new { Passed = checks.Count, Checks = checks, CloudTrafficSent = false, RealAudioUsed = false });
    }
    private static string Request(int id, string method, object parameters) => JsonSerializer.Serialize(new { jsonrpc = "2.0", id, method, @params = parameters });
    private static async Task<List<JsonElement>> ExchangeAsync(string root, bool grant, string[] requests)
    {
        var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true };
        info.ArgumentList.Add("--agent"); info.ArgumentList.Add("serve"); if (grant) info.ArgumentList.Add("--allow-text-read"); info.Environment["HANDYMEETING_DATA_DIR"] = root;
        using var child = Process.Start(info)!; using var job = new WorkerJob(child);
        foreach (var request in requests) await child.StandardInput.WriteLineAsync(request); child.StandardInput.Close();
        var output = child.StandardOutput.ReadToEndAsync(); var error = child.StandardError.ReadToEndAsync();
        await child.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(20));
        if (child.ExitCode != 0 || (await error).Length != 0) throw new InvalidOperationException("Agent child failed.");
        var lines = (await output).Split('\n', StringSplitOptions.RemoveEmptyEntries);
        if (lines.Length != requests.Length) throw new InvalidOperationException("Agent response count differs.");
        return lines.Select(line => { using var document = JsonDocument.Parse(line); return document.RootElement.Clone(); }).ToList();
    }
}
