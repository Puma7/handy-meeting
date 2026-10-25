using System.Diagnostics;
using System.Text.Json;

namespace HandyMeeting;

public static class PortableDevices
{
    public static async Task<List<AudioDevice>> ListAsync(bool microphone, CancellationToken token)
    {
        if (OperatingSystem.IsMacOS()) return [new("default", microphone ? "Default microphone" : "System audio")];
        if (!OperatingSystem.IsLinux()) throw new PlatformNotSupportedException("Use the verified Windows application on Windows.");
        var json = await RunTextAsync("pactl", ["--format=json", "list", microphone ? "sources" : "sinks"], token);
        using var data = JsonDocument.Parse(json);
        return data.RootElement.EnumerateArray().Where(d => !microphone || !d.GetProperty("name").GetString()!.EndsWith(".monitor", StringComparison.Ordinal))
            .Select(d => new AudioDevice(d.GetProperty("name").GetString()!, d.TryGetProperty("description", out var description) ? description.GetString()! : d.GetProperty("name").GetString()!)).ToList();
    }
    internal static async Task<string> RunTextAsync(string file, string[] args, CancellationToken token)
    {
        var info = new ProcessStartInfo(file) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true };
        foreach (var arg in args) info.ArgumentList.Add(arg);
        using var process = Process.Start(info) ?? throw new IOException("Could not start " + file);
        var output = process.StandardOutput.ReadToEndAsync(token); var error = process.StandardError.ReadToEndAsync(token);
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(token); deadline.CancelAfter(TimeSpan.FromSeconds(15));
        try { await process.WaitForExitAsync(deadline.Token); }
        catch { if (!process.HasExited) process.Kill(true); throw; }
        var result = await output; _ = await error;
        if (process.ExitCode != 0) throw new IOException(file + " failed. Check the local audio service and installed dependencies.");
        return result;
    }
}

/// <summary>Each track has its own capture process and durable writer. Inference never owns capture.</summary>
public sealed class PortableCapture : IAsyncDisposable
{
    private sealed class Track(Process process, PcmTimeline timeline)
    {
        public Process Process = process;
        public PcmTimeline Timeline = timeline;
        public Task Pump = Task.CompletedTask;
        public Task ErrorDrain = Task.CompletedTask;
        public bool Failed;
    }
    private readonly List<Track> tracks = [];
    private readonly SessionStore session;
    private readonly Stopwatch clock = new();
    private bool stopping;
    public event Action<string>? Failed;
    public long ElapsedMs => clock.ElapsedMilliseconds;
    public PortableCapture(SessionStore session) => this.session = session;
    public async Task StartAsync(AudioDevice system, AudioDevice? microphone)
    {
        if (OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Use the Windows release for WASAPI recording.");
        if (OperatingSystem.IsLinux())
        {
            await PortableDevices.RunTextAsync("pactl", ["info"], CancellationToken.None);
            await PortableDevices.RunTextAsync("parec", ["--version"], CancellationToken.None);
        }
        clock.Start(); session.NoteEstimatedTiming();
        try
        {
            StartTrack("system", system.Id);
            if (microphone is not null) StartTrack("microphone", microphone.Id);
            await Task.Delay(250);
            if (tracks.Any(t => t.Process.HasExited)) throw new IOException("Audio helper did not start. Check system audio/microphone permissions and the selected devices.");
        }
        catch { await StopAsync(); throw; }
    }
    private void StartTrack(string source, string device)
    {
        var info = new ProcessStartInfo(OperatingSystem.IsMacOS() ? Path.Combine(AppContext.BaseDirectory, "HandyMeetingCapture") : "parec")
        { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardInput = true, RedirectStandardError = true };
        var args = OperatingSystem.IsMacOS() ? new[] { "--track", source }
            : new[] { "--raw", "--format=s16le", "--rate=16000", "--channels=1", "--latency-msec=100", "--device=" + (source == "system" ? device + ".monitor" : device) };
        foreach (var arg in args) info.ArgumentList.Add(arg);
        var process = Process.Start(info) ?? throw new IOException("Audio helper failed to start.");
        var timeline = new PcmTimeline(Path.Combine(session.Folder, "audio"), source, 30, session.AddPart, part => session.AudioCheckpoint(part, false));
        var track = new Track(process, timeline); tracks.Add(track);
        track.ErrorDrain = Task.Run(async () => { while (await process.StandardError.ReadLineAsync() is not null) { } });
        track.Pump = Task.Run(async () =>
        {
            try
            {
                var buffer = new byte[3200]; var pending = -1; long next = -1;
                var reading = process.StandardOutput.BaseStream.ReadAsync(buffer).AsTask();
                while (true)
                {
                    if (!reading.IsCompleted)
                    {
                        await Task.WhenAny(reading, Task.Delay(250));
                        if (!reading.IsCompleted) { timeline.PadTo(Math.Max(0, ElapsedMs * 16 - 8000)); timeline.Checkpoint(); continue; }
                    }
                    var read = await reading; if (read == 0) break;
                    byte[] data;
                    if (pending >= 0) { data = new byte[read + 1]; data[0] = (byte)pending; buffer.AsSpan(0, read).CopyTo(data.AsSpan(1)); pending = -1; }
                    else data = buffer.AsSpan(0, read).ToArray();
                    if (data.Length % 2 != 0) { pending = data[^1]; Array.Resize(ref data, data.Length - 1); }
                    if (next < 0) next = Math.Max(0, ElapsedMs * 16 - data.Length / 2);
                    timeline.Append(data, next); next += data.Length / 2; timeline.Checkpoint();
                    reading = process.StandardOutput.BaseStream.ReadAsync(buffer).AsTask();
                }
                if (!stopping) throw new IOException("Audio helper stopped unexpectedly.");
            }
            catch (Exception error)
            {
                track.Failed = true;
                var message = source + ": " + error.Message;
                try { session.Warn(message); } catch (IOException) { }
                Failed?.Invoke(message);
            }
        });
    }
    public async Task StopAsync()
    {
        if (stopping) return; stopping = true; clock.Stop();
        foreach (var track in tracks)
        {
            if (!track.Process.HasExited)
            {
                if (OperatingSystem.IsMacOS()) { track.Process.StandardInput.Close(); }
                else track.Process.Kill();
            }
        }
        foreach (var track in tracks)
        {
            try { await track.Process.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(5)); }
            catch (TimeoutException) { track.Process.Kill(true); }
            await track.Pump; await track.ErrorDrain;
            if (track.Failed) track.Timeline.Dispose(); else track.Timeline.Complete(Math.Max(track.Timeline.WrittenSamples, ElapsedMs * 16));
            track.Timeline.Dispose(); track.Process.Dispose();
        }
        session.Data.DurationMs = ElapsedMs; session.Data.EndedAt = DateTimeOffset.Now; session.SetStatus("recorded"); session.ExportSnapshot();
    }
    public async ValueTask DisposeAsync() => await StopAsync();
}
