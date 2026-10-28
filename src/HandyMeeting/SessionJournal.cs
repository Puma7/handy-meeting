using System.IO;
using System.Text;
using System.Text.Json;

namespace HandyMeeting;

public sealed class TextCheckpoint
{
    public long Sequence { get; set; }
    public DateTimeOffset At { get; set; } = DateTimeOffset.Now;
    public string File { get; set; } = "";
    public string ModelId { get; set; } = "";
    public string Full { get; set; } = "";
    public string Committed { get; set; } = "";
    public string Tentative { get; set; } = "";
    public long ProcessedSamples { get; set; }
    public bool Final { get; set; }
}

public sealed partial class SessionStore
{
    private DateTime lastManifestSave = DateTime.MinValue;
    private bool journalPreparedForAppend;
    public string JournalPath => Path.Combine(Folder, "transcript-journal.jsonl");
    public IDisposable AcquireLease() => new FileStream(Path.Combine(Folder, "session.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
    public List<AudioPart> PartsSnapshot() { lock (gate) return Data.Parts.Select(p => p.Copy()).ToList(); }
    public string CopyTranscriptSnapshot() { lock (gate) return Transcription.Render(this, false); }
    public AudioPart? NextPendingPart(string source, string modelId)
    {
        lock (gate)
        {
            AudioPart? next = null;
            foreach (var part in Data.Parts)
                if (part.Source == source && !(part.Status == "complete" && part.ModelId == modelId) && (next is null || part.StartSample < next.StartSample)) next = part;
            return next?.Copy();
        }
    }
    public bool AllPartsComplete(string modelId)
    {
        lock (gate)
        {
            foreach (var part in Data.Parts) if (part.Status != "complete" || part.ModelId != modelId) return false;
            return true;
        }
    }
    private static readonly IComparer<AudioPart> PartOrder = Comparer<AudioPart>.Create((a, b) =>
    {
        var time = a.StartSample.CompareTo(b.StartSample);
        return time != 0 ? time : string.Compare(a.Source, b.Source, StringComparison.CurrentCulture);
    });
    public List<AudioPart> RecentTextPartsSnapshot()
    {
        lock (gate)
        {
            // Bound temporary storage even when a recovered session contains many hours.
            var recent = new List<AudioPart>(17);
            foreach (var part in Data.Parts)
            {
                if (string.IsNullOrWhiteSpace(part.Text)) continue;
                var index = recent.BinarySearch(part, PartOrder);
                index = index < 0 ? ~index : index + 1;
                if (recent.Count == 16 && index == 0) continue;
                recent.Insert(index, part);
                if (recent.Count > 16) recent.RemoveAt(0);
            }
            for (var i = 0; i < recent.Count; i++) recent[i] = recent[i].Copy();
            return recent;
        }
    }
    public void SetStatus(string status, string? error = null)
    {
        lock (gate) { Data.Status = status; Data.StreamingError = error; Save(); }
    }
    public void SetBackend(string backend) { lock (gate) { Data.StreamingBackend = backend; Data.StreamingError = null; Save(); } }
    public void AudioCheckpoint(AudioPart description, bool closed)
    {
        lock (gate)
        {
            var part = Data.Parts.FirstOrDefault(p => p.File == description.File);
            var added = part is null;
            if (part is null) { part = description.Copy(); Data.Parts.Add(part); }
            part.SampleCount = description.SampleCount; part.HasSignal = description.HasSignal; part.AudioClosed = closed;
            Data.DurationMs = Math.Max(Data.DurationMs, part.EndMs);
            if (added || closed || (DateTime.UtcNow - lastManifestSave).TotalSeconds >= 15)
            { Save(); lastManifestSave = DateTime.UtcNow; }
        }
    }
    public void PersistText(AudioPart audio, string modelId, StreamReply reply, long processedSamples, bool final)
    {
        lock (gate)
        {
            var part = Data.Parts.First(p => p.File == audio.File);
            JsonData.CheckDirectory(Folder); JsonData.CheckEntry(JournalPath);
            if (!journalPreparedForAppend)
            {
                if (File.Exists(JournalPath))
                    JsonData.WriteAtomic(JournalPath, replacement =>
                    {
                        using var original = new FileStream(JournalPath, FileMode.Open, FileAccess.Read, FileShare.Read);
                        original.CopyTo(replacement, 32_000);
                    });
                // One private directory entry per writer object, before the first append.
                // Existing journal bytes (including a torn tail) survive exactly in .bak.
                journalPreparedForAppend = true;
            }
            if (!string.IsNullOrEmpty(part.Text) && part.ModelId != modelId)
            {
                var archive = Path.Combine(Folder, "text-history", $"{DateTimeOffset.UtcNow:yyyyMMdd-HHmmss-fffffff}-{Guid.NewGuid():N}.json");
                JsonData.Save(archive, part.Copy());
            }
            var checkpoint = new TextCheckpoint { Sequence = ++Data.SnapshotSequence, File = part.File, ModelId = modelId, Full = reply.Full, Committed = reply.Committed, Tentative = reply.Tentative, ProcessedSamples = processedSamples, Final = final };
            // Append and force to disk BEFORE publishing/acknowledging the new text in memory/UI.
            var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(checkpoint) + "\n");
            using (var output = new FileStream(JournalPath, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.Read, 4096, FileOptions.WriteThrough))
            {
                if (output.Length > 0)
                {
                    output.Position = output.Length - 1;
                    if (output.ReadByte() != '\n') { output.Position = output.Length; output.WriteByte((byte)'\n'); }
                }
                output.Position = output.Length; output.Write(bytes); output.Flush(true);
            }
            ApplyText(part, checkpoint);
            if (final || (DateTime.UtcNow - lastManifestSave).TotalSeconds >= 15) { Save(); lastManifestSave = DateTime.UtcNow; }
        }
    }
    public void NoteProcessed(string file, long samples)
    {
        lock (gate) { var part = Data.Parts.First(p => p.File == file); part.ProcessedSamples = samples; }
    }
    private static void ApplyText(AudioPart part, TextCheckpoint checkpoint)
    {
        part.Text = checkpoint.Full; part.CommittedText = checkpoint.Committed; part.TentativeText = checkpoint.Tentative;
        part.ProcessedSamples = checkpoint.ProcessedSamples; part.ModelId = checkpoint.ModelId;
        part.Status = checkpoint.Final && checkpoint.ProcessedSamples == part.SampleCount ? "complete" : "pending"; part.Error = null;
    }
    private void ReplayTranscriptJournal()
    {
        if (!File.Exists(JournalPath)) return;
        var ignored = 0;
        foreach (var line in File.ReadLines(JournalPath))
        {
            if (string.IsNullOrWhiteSpace(line)) continue;
            try
            {
                var checkpoint = JsonSerializer.Deserialize<TextCheckpoint>(line, JsonData.Options);
                if (checkpoint is null) continue;
                var part = Data.Parts.FirstOrDefault(p => p.File == checkpoint.File);
                if (part is null || checkpoint.ProcessedSamples < 0 || checkpoint.ProcessedSamples > part.SampleCount) continue;
                ApplyText(part, checkpoint); Data.SnapshotSequence = Math.Max(Data.SnapshotSequence, checkpoint.Sequence);
            }
            catch (JsonException) { ignored++; }
        }
        if (ignored > 0) Data.Warnings.Add($"{ignored} unvollständige Journalzeile(n) wurden übersprungen; ältere gespeicherte Textstände bleiben erhalten.");
    }
    public long SavedAudioMs
    {
        get
        {
            lock (gate)
            {
                long end = 0;
                foreach (var part in Data.Parts) end = Math.Max(end, part.EndMs);
                return end;
            }
        }
    }
    public long PendingAudioMs(string modelId)
    {
        lock (gate)
        {
            long system = 0, microphone = 0;
            foreach (var part in Data.Parts)
            {
                var samples = part.ModelId != modelId ? part.SampleCount : part.Status == "complete" ? 0 : Math.Max(0, part.SampleCount - part.ProcessedSamples);
                if (part.Source == "system") system += samples;
                else if (part.Source == "microphone") microphone += samples;
            }
            return Math.Max(system, microphone) * 1000 / PcmTimeline.SampleRate;
        }
    }
    public void ExportSnapshot()
    {
        lock (gate) Transcription.Export(this);
    }
}

public sealed record SessionHistoryItem(string Manifest, string Title, DateTimeOffset StartedAt, long DurationMs, string Status)
{
    public override string ToString() => $"{StartedAt:dd.MM. HH:mm} · {Title} · {ClockTime.Format(DurationMs)} · {Status}";
    public static List<SessionHistoryItem> Read(string root)
    {
        var result = new List<SessionHistoryItem>();
        if (!Directory.Exists(root)) return result;
        foreach (var folder in Directory.EnumerateDirectories(root))
        {
            var path = Path.Combine(folder, "session.json");
            try
            {
                var data = JsonData.Read<SessionData>(path);
                if (data is null) continue;
                var status = data.Status switch { "complete" => "Fertig", "recording" => "Aufnahme / Wiederherstellung", "catching-up" => "Nachlauf / Wiederaufnahme", "error" => "Fortsetzen möglich", _ => "Lokal gespeichert" };
                result.Add(new(path, data.Title, data.StartedAt, data.DurationMs, status));
            }
            catch (Exception error) when (error is IOException or JsonException or UnauthorizedAccessException)
            { result.Add(new(path, "Wiederherstellung prüfen", Directory.GetCreationTime(folder), 0, "Sicherung vorhanden?")); }
        }
        return result.OrderByDescending(s => s.StartedAt).ToList();
    }
}
