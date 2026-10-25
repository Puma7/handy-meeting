using System.Diagnostics;
using System.IO;

namespace HandyMeeting;

public sealed class StreamingCoordinator : IAsyncDisposable
{
    private static readonly string[] Sources = ["system", "microphone"];
    private readonly SessionStore session;
    private readonly string modelId, modelPath;
    private readonly CancellationTokenSource cancellation = new();
    private readonly Dictionary<string, (string File, long Cursor)> positions = [];
    private StreamingProcess? worker;
    private Task? running;
    private volatile bool recording;
    private bool allowRestart = true;
    private int failures;
    private long lastProgressUpdate;
    public string Message { get; private set; } = "Streaming-Modell wird geladen. Audio wird bereits lokal gesichert.";
    public string Backend { get; private set; } = "";
    public int RestartCount { get; private set; }
    public int ModelLoadCount { get; private set; }
    public bool HasFailed { get; private set; }
    public event Action? Updated;
    public StreamingCoordinator(SessionStore session, string modelId, string modelPath, bool recording)
    {
        this.session = session; this.modelId = modelId; this.modelPath = modelPath; this.recording = recording;
        session.Data.ModelId = modelId; session.Data.StreamingModelPath = modelPath;
    }
    public void Start() => running ??= Task.Run(RunAsync);
    public async Task FinishAsync()
    {
        recording = false; session.SetStatus("catching-up");
        if (running is not null) await running;
    }
    public async Task PauseAsync()
    {
        cancellation.Cancel();
        if (running is not null) await running;
    }
    public void Retry()
    {
        if (!HasFailed) return;
        HasFailed = false; failures = 0; allowRestart = true; Message = "Streaming wird aus dem gesicherten Audio wieder aufgenommen.";
    }
    public void KillWorkerForTest() => worker?.KillForTest();
    public int? WorkerProcessIdForTest { get { try { return worker?.ProcessId; } catch (InvalidOperationException) { return null; } } }
    private async Task RunAsync()
    {
        var token = cancellation.Token;
        try
        {
            while (!token.IsCancellationRequested)
            {
                if (!allowRestart)
                {
                    if (!recording) break;
                    await Task.Delay(250, token); continue;
                }
                try
                {
                    if (worker is null)
                    {
                        Message = failures == 0 ? "Streaming-Modell wird einmalig geladen. Audio ist gesichert." : "Streaming-Prozess startet neu; gesicherte Abschnitte werden nachgeholt.";
                        Updated?.Invoke();
                        worker = await StreamingProcess.StartAsync(modelPath, failures >= 2, token);
                        ModelLoadCount++;
                        Backend = worker.Backend; session.SetBackend(Backend); positions.Clear();
                        // Every incomplete 30 s window is replayed from disk after a process loss.
                        // Completed windows and all previous text revisions are left intact.
                    }
                    var didWork = false;
                    foreach (var source in Sources)
                    {
                        var next = session.NextPendingPart(source, modelId);
                        if (next is null) continue;
                        if (!next.HasSignal && next.AudioClosed)
                        {
                            session.PersistText(next, modelId, new(), next.SampleCount, true); didWork = true; continue;
                        }
                        if (!next.HasSignal) continue;
                        if (!positions.TryGetValue(source, out var state) || state.File != next.File)
                        {
                            await worker.CallAsync(new() { Operation = "begin", Source = source }, token);
                            state = (next.File, 0); positions[source] = state;
                        }
                        var available = next.SampleCount - state.Cursor;
                        if (available >= 8000 || (next.AudioClosed && available > 0))
                        {
                            var count = (int)Math.Min(8000, available);
                            var reply = await worker.CallAsync(new() { Operation = "feed", Source = source, AudioPath = session.AudioPath(next), OffsetSamples = state.Cursor, SampleCount = count }, token);
                            state.Cursor += count; positions[source] = state;
                            if (reply.Full != next.Text || reply.Committed != next.CommittedText || reply.Tentative != next.TentativeText)
                                session.PersistText(next, modelId, reply, state.Cursor, false);
                            else session.NoteProcessed(next.File, state.Cursor);
                            didWork = true;
                        }
                        if (next.AudioClosed && state.Cursor == next.SampleCount)
                        {
                            var reply = await worker.CallAsync(new() { Operation = "finalize", Source = source }, token);
                            session.PersistText(next, modelId, reply, state.Cursor, true);
                            session.ExportSnapshot(); positions.Remove(source); didWork = true;
                        }
                    }
                    if (Stopwatch.GetElapsedTime(lastProgressUpdate).TotalMilliseconds >= 250)
                    {
                        var compute = Backend.StartsWith("Vulkan", StringComparison.OrdinalIgnoreCase) ? "Grafikkarte" : Backend.StartsWith("CPU", StringComparison.OrdinalIgnoreCase) ? "Prozessor" : Backend;
                        Message = $"Streaming · {compute} · Audio gesichert bis {ClockTime.Format(session.SavedAudioMs)} · Rückstand {ClockTime.Format(session.PendingAudioMs(modelId))}";
                        lastProgressUpdate = Stopwatch.GetTimestamp();
                    }
                    if (!recording && session.AllPartsComplete(modelId))
                    {
                        session.SetStatus("complete"); session.ExportSnapshot(); Message = "Fertig. Audio, Textstände und Transkript sind lokal gespeichert.";
                        Updated?.Invoke(); break;
                    }
                    if (!didWork) await Task.Delay(100, token);
                }
                catch (OperationCanceledException) when (token.IsCancellationRequested) { throw; }
                catch (Exception error)
                {
                    if (worker is not null) { await worker.DisposeAsync(); worker = null; }
                    positions.Clear(); failures++; RestartCount++;
                    Message = "Streaming unterbrochen: " + error.Message + " Audio wird weiter lokal gesichert.";
                    session.Data.StreamingError = Message; session.Save(); Updated?.Invoke();
                    if (failures >= 3)
                    {
                        HasFailed = true; allowRestart = false;
                        if (!recording) break;
                    }
                    else await Task.Delay(TimeSpan.FromSeconds(failures), token);
                }
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        { Message = "Streaming pausiert. Audio und gespeicherte Textstände bleiben erhalten."; }
        catch (Exception error)
        { HasFailed = true; Message = "Streaming konnte nicht fortgesetzt werden: " + error.Message + " Die gesicherten Dateien bleiben erhalten."; }
        finally
        {
            if (worker is not null) { await worker.DisposeAsync(); worker = null; }
            try
            {
                if (session.Data.Status != "complete") session.SetStatus(recording && session.Data.EndedAt is null ? "recording" : HasFailed ? "error" : "recorded", session.Data.StreamingError);
                session.ExportSnapshot();
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            { HasFailed = true; Message = "Datenträgerfehler: " + error.Message + " Aufnahme und Sicherungsstatus prüfen."; }
            Updated?.Invoke();
        }
    }
    public async ValueTask DisposeAsync() { await PauseAsync(); cancellation.Dispose(); }
}
