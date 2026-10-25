using System.Buffers.Binary;
using System.IO;
using System.Threading.Channels;
using NAudio.CoreAudioApi;
using NAudio.Wave;

namespace HandyMeeting;

public static class AudioDevices
{
    public static List<AudioDevice> List(DataFlow flow)
    {
        using var enumerator = new MMDeviceEnumerator();
        var result = new List<AudioDevice>();
        foreach (var device in enumerator.EnumerateAudioEndPoints(flow, DeviceState.Active))
            using (device) result.Add(new(device.ID, device.FriendlyName));
        string? defaultId = null;
        try { using var device = enumerator.GetDefaultAudioEndpoint(flow, Role.Multimedia); defaultId = device.ID; }
        catch (Exception) { /* Devices can disappear between enumeration and default lookup. */ }
        return result.OrderByDescending(d => d.Id == defaultId).ThenBy(d => d.Name).ToList();
    }
}

internal sealed record AudioPacket(byte[] Pcm, long Qpc, long ReceivedQpc, bool TimestampError, bool Discontinuity);

internal sealed class PacketPosition
{
    private long lastArrival;
    public long Locate(long origin, long qpc, long arrival, int samples, bool estimated, long written)
    {
        var arrivalSample = (long)Math.Round((arrival - origin) * (PcmTimeline.SampleRate / 10_000_000d));
        long position;
        if (!estimated) position = (long)Math.Round((qpc - origin) * (PcmTimeline.SampleRate / 10_000_000d));
        else if (lastArrival == 0 || arrival - lastArrival > 2_000_000L + samples * 10_000_000L / PcmTimeline.SampleRate)
            position = Math.Max(written, arrivalSample - samples);
        else
            // A single callback can drain many queued packets. Arrival timestamps must
            // never cause one packet to overwrite or trim another in that burst.
            position = written;
        lastArrival = arrival;
        return position;
    }
}

internal sealed class CaptureTrack : IAsyncDisposable
{
    private readonly MMDevice device;
    private readonly WasapiRecorder recorder;
    private readonly PcmTimeline timeline;
    private readonly Func<long> origin;
    private readonly Action<string> fail;
    private readonly string label;
    private readonly Channel<AudioPacket> packets = Channel.CreateBounded<AudioPacket>(new BoundedChannelOptions(128) { SingleReader = true, SingleWriter = false, FullMode = BoundedChannelFullMode.Wait });
    private readonly TaskCompletionSource stopped = new(TaskCreationOptions.RunContinuationsAsynchronously);
    private readonly Task pump;
    private readonly PacketPosition positions = new();
    private readonly CancellationTokenSource heartbeatCancellation = new();
    private readonly Task heartbeat;
    private bool stopping;
    private int failureReported;
    private float peak;
    private bool receivedAudioPacket;
    public bool Muted { get; set; }
    public float Peak => Volatile.Read(ref peak);
    public CaptureTrack(AudioDevice selected, bool loopback, SessionStore session, Func<long> origin, Action<string> fail)
    {
        this.origin = origin; this.fail = fail; label = loopback ? "Computerton" : "Mikrofon";
        using var enumerator = new MMDeviceEnumerator(); device = enumerator.GetDevice(selected.Id);
        try
        {
            var builder = new WasapiRecorderBuilder().WithDevice(device).WithFormat(new WaveFormat(16_000, 16, 1)).WithBufferLength(100).WithMmcssThreadPriority("Audio");
            if (loopback) builder.WithLoopbackCapture();
            recorder = builder.Build();
            timeline = new PcmTimeline(Path.Combine(session.Folder, "audio"), loopback ? "system" : "microphone", session.Data.ChunkSeconds, session.AddPart, part => session.AudioCheckpoint(part, false));
        }
        catch { device.Dispose(); throw; }
        recorder.DataAvailable += (buffer, flags, _, qpc) =>
        {
            var copy = buffer.ToArray();
            if (Muted) Array.Clear(copy);
            var maximum = 0;
            for (var i = 0; i + 1 < copy.Length; i += 2) maximum = Math.Max(maximum, Math.Abs((int)BinaryPrimitives.ReadInt16LittleEndian(copy.AsSpan(i))));
            Volatile.Write(ref peak, maximum / 32768f);
            var packet = new AudioPacket(copy, qpc, ClockTime.Qpc100ns, ((int)flags & 4) != 0, ((int)flags & 1) != 0);
            if (!packets.Writer.TryWrite(packet)) ReportFailure($"{label}: Der Schreibpuffer ist voll; die Aufnahme wurde angehalten.");
        };
        recorder.RecordingStopped += (_, args) =>
        {
            packets.Writer.TryComplete(); stopped.TrySetResult();
            if (args.Exception is not null) ReportFailure($"{label}: {args.Exception.Message}");
            else if (!stopping) ReportFailure($"{label}: Das Audiogerät hat die Aufnahme unerwartet beendet.");
        };
        pump = Task.Run(async () =>
        {
            try
            {
                await foreach (var packet in packets.Reader.ReadAllAsync())
                {
                    if (packet.Pcm.Length == 0)
                    {
                        if (origin() > 0)
                        {
                            var safeNow = (ClockTime.Qpc100ns - origin()) * PcmTimeline.SampleRate / 10_000_000 - 8000;
                            timeline.PadTo(Math.Max(0, safeNow)); timeline.Checkpoint();
                        }
                        continue;
                    }
                    var estimated = packet.Qpc <= 0 || packet.TimestampError;
                    if (estimated)
                    {
                        session.NoteEstimatedTiming();
                    }
                    var position = positions.Locate(origin(), packet.Qpc, packet.ReceivedQpc, packet.Pcm.Length / 2, estimated, timeline.WrittenSamples);
                    var elapsed = (ClockTime.Qpc100ns - origin()) * (PcmTimeline.SampleRate / 10_000_000d);
                    if (position > elapsed + PcmTimeline.SampleRate * 2 || position < -PcmTimeline.SampleRate * 2)
                        throw new InvalidDataException("Die Audiozeit liegt außerhalb der Aufnahmezeit.");
                    if (packet.Discontinuity && receivedAudioPacket)
                        session.Warn($"{label}: Der Treiber hat eine Audio-Unterbrechung gemeldet; die betroffenen Zeitbereiche können unvollständig sein.");
                    timeline.Append(packet.Pcm, position);
                    receivedAudioPacket = true;
                }
            }
            catch (Exception e) { ReportFailure($"{label}: Audio konnte nicht gespeichert werden: {e.Message}"); }
        });
        heartbeat = Task.Run(async () =>
        {
            using var timer = new PeriodicTimer(TimeSpan.FromMilliseconds(250));
            try { while (await timer.WaitForNextTickAsync(heartbeatCancellation.Token)) packets.Writer.TryWrite(new([], 0, ClockTime.Qpc100ns, false, false)); }
            catch (OperationCanceledException) when (heartbeatCancellation.IsCancellationRequested) { }
        });
    }
    private void ReportFailure(string message)
    {
        if (Interlocked.Exchange(ref failureReported, 1) == 0) { fail(message); recorder.StopRecording(); }
    }
    public void Start() => recorder.StartRecording();
    public async Task StopAsync(long endSample)
    {
        stopping = true;
        heartbeatCancellation.Cancel(); await heartbeat;
        try
        {
            recorder.StopRecording();
            try { await stopped.Task.WaitAsync(TimeSpan.FromSeconds(8)); }
            catch (TimeoutException) { ReportFailure($"{label}: Das Audiogerät reagiert nicht auf das Beenden."); }
        }
        finally
        {
            try { await recorder.DisposeAsync(); }
            finally
            {
                packets.Writer.TryComplete();
                try { await pump; timeline.Complete(endSample); }
                finally { try { timeline.Dispose(); } finally { device.Dispose(); } }
            }
        }
    }
    public async ValueTask DisposeAsync()
    {
        stopping = true; heartbeatCancellation.Cancel(); await heartbeat;
        await recorder.DisposeAsync(); packets.Writer.TryComplete(); await pump; timeline.Dispose(); device.Dispose();
    }
}

public sealed class CaptureController
{
    private readonly List<CaptureTrack> tracks = [];
    private long started;
    private Task? stoppingTask;
    private readonly object gate = new();
    public SessionStore Session { get; }
    public event Action<string>? Failed;
    public float SystemPeak => tracks.Count > 0 ? tracks[0].Peak : 0;
    public float MicPeak => tracks.Count > 1 ? tracks[1].Peak : 0;
    public long ElapsedMs => started == 0 ? 0 : (ClockTime.Qpc100ns - started) / 10_000;
    public bool MicrophoneMuted
    {
        get => tracks.Count > 1 && tracks[1].Muted;
        set { if (tracks.Count > 1) tracks[1].Muted = value; }
    }
    public CaptureController(SessionStore session) => Session = session;
    public async Task StartAsync(AudioDevice output, AudioDevice? microphone)
    {
        void Fail(string message)
        {
            try { Session.Warn(message); } catch (IOException) { /* A full disk must not prevent stopping capture. */ }
            Failed?.Invoke(message);
        }
        try
        {
            // Construct outside WPF's synchronization context so stop callbacks never depend on the UI.
            tracks.Add(await Task.Run(() => new CaptureTrack(output, true, Session, () => started, Fail)));
            if (microphone is not null) tracks.Add(await Task.Run(() => new CaptureTrack(microphone, false, Session, () => started, Fail)));
            started = ClockTime.Qpc100ns;
            foreach (var track in tracks) track.Start();
        }
        catch
        {
            foreach (var track in tracks) await track.DisposeAsync();
            Session.Data.Status = "recorded"; Session.Warn("Die Audioaufnahme konnte nicht vollständig gestartet werden."); throw;
        }
    }
    public Task StopAsync() { lock (gate) return stoppingTask ??= StopCoreAsync(); }
    private async Task StopCoreAsync()
    {
        var duration = ElapsedMs;
        var endSample = duration * PcmTimeline.SampleRate / 1000;
        foreach (var track in tracks)
        {
            try { await track.StopAsync(endSample); }
            catch (Exception e)
            {
                // Always stop the other source even if the disk is full and saving a warning fails.
                Session.Data.CaptureIncomplete = true;
                Session.Data.Warnings.Add($"Aufnahme konnte nicht vollständig beendet werden: {e.Message}");
            }
        }
        Session.Data.DurationMs = Math.Max(duration, Session.SavedAudioMs); Session.Data.EndedAt = DateTimeOffset.Now; Session.Data.Status = "recorded"; Session.Save();
    }
}
