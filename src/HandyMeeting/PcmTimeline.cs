using System.Buffers.Binary;
using System.IO;
using System.Text;

namespace HandyMeeting;

/// <summary>Disk-backed, bounded-memory PCM timeline. Empty loopback intervals stay as silence.</summary>
public sealed class PcmTimeline : IDisposable
{
    public const int SampleRate = 16_000;
    private readonly string folder, source;
    private readonly long chunkSamples;
    private readonly Action<AudioPart> completed;
    private readonly Action<AudioPart>? checkpoint;
    private readonly byte[] zeros = new byte[32_000];
    private FileStream? file;
    private long chunkStart, chunkCount, written;
    private bool hasSignal;
    private DateTime lastFlush = DateTime.UtcNow;
    public long WrittenSamples => written;
    public PcmTimeline(string folder, string source, int chunkSeconds, Action<AudioPart> completed, Action<AudioPart>? checkpoint = null)
    {
        if (source is not ("system" or "microphone") || chunkSeconds is < 1 or > 300) throw new ArgumentException("Ungültige Spurkonfiguration.");
        this.folder = folder; this.source = source; chunkSamples = SampleRate * chunkSeconds; this.completed = completed; this.checkpoint = checkpoint;
        Directory.CreateDirectory(folder);
    }
    public void Append(ReadOnlySpan<byte> pcm, long startSample)
    {
        if (pcm.Length % 2 != 0) throw new InvalidDataException("Unvollständiges PCM-Sample.");
        if (startSample < 0)
        {
            var skip = Math.Min(pcm.Length / 2L, -startSample);
            pcm = pcm[(int)(skip * 2)..]; startSample += skip;
        }
        // Ignore sub-millisecond rounding jitter, but retain actual silent intervals.
        if (Math.Abs(startSample - written) <= 16) startSample = written;
        if (startSample > written) PadTo(startSample);
        else if (startSample < written)
        {
            var skip = Math.Min(pcm.Length / 2L, written - startSample);
            pcm = pcm[(int)(skip * 2)..];
        }
        Write(pcm);
    }
    public void PadTo(long targetSample)
    {
        while (written < targetSample) Write(zeros.AsSpan(0, (int)Math.Min(zeros.Length, (targetSample - written) * 2)));
    }
    private void Write(ReadOnlySpan<byte> data)
    {
        while (!data.IsEmpty)
        {
            if (file is null)
            {
                chunkStart = written; chunkCount = 0; hasSignal = false;
                file = new FileStream(Path.Combine(folder, $"{source}-{chunkStart:D12}.wav"), FileMode.CreateNew, FileAccess.ReadWrite, FileShare.Read);
                WriteHeader(file, 0);
            }
            var bytes = (int)Math.Min(data.Length, (chunkSamples - chunkCount) * 2);
            var part = data[..bytes];
            if (!hasSignal)
                for (var i = 0; i < part.Length; i += 2)
                    if (BinaryPrimitives.ReadInt16LittleEndian(part[i..]) != 0) { hasSignal = true; break; }
            file.Write(part); chunkCount += bytes / 2; written += bytes / 2; data = data[bytes..];
            if ((DateTime.UtcNow - lastFlush).TotalSeconds >= 0.5) FlushDurable();
            if (chunkCount == chunkSamples) CloseChunk();
        }
    }
    private void FlushDurable()
    {
        if (file is null) return;
        var position = file.Position; WriteHeader(file, chunkCount * 2); file.Position = position; file.Flush(true); lastFlush = DateTime.UtcNow;
        checkpoint?.Invoke(new() { File = Path.GetFileName(file.Name), Source = source, StartSample = chunkStart, SampleCount = chunkCount, HasSignal = hasSignal, AudioClosed = false });
    }
    public void Checkpoint() { if ((DateTime.UtcNow - lastFlush).TotalSeconds >= 0.5) FlushDurable(); }
    private void CloseChunk()
    {
        if (file is null) return;
        var path = file.Name;
        try { FlushDurable(); }
        finally { file.Dispose(); file = null; }
        completed(new() { File = Path.GetFileName(path), Source = source, StartSample = chunkStart, SampleCount = chunkCount, HasSignal = hasSignal });
    }
    public void Complete(long endSample) { PadTo(endSample); CloseChunk(); }
    public void Dispose() => CloseChunk();
    internal static void WriteHeader(Stream stream, long dataBytes)
    {
        stream.Position = 0;
        using var writer = new BinaryWriter(stream, Encoding.ASCII, true);
        writer.Write(Encoding.ASCII.GetBytes("RIFF")); writer.Write(checked((uint)(36 + dataBytes))); writer.Write(Encoding.ASCII.GetBytes("WAVEfmt "));
        writer.Write(16); writer.Write((short)1); writer.Write((short)1); writer.Write(SampleRate); writer.Write(SampleRate * 2); writer.Write((short)2); writer.Write((short)16);
        writer.Write(Encoding.ASCII.GetBytes("data")); writer.Write(checked((uint)dataBytes));
    }
    public static (long Samples, bool HasSignal) InspectAndRepair(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.ReadWrite, FileShare.Read);
        Span<byte> header = stackalloc byte[44]; stream.ReadExactly(header);
        if (!header[..4].SequenceEqual("RIFF"u8) || !header.Slice(8, 8).SequenceEqual("WAVEfmt "u8) || !header.Slice(36, 4).SequenceEqual("data"u8)
            || BinaryPrimitives.ReadInt16LittleEndian(header[20..]) != 1 || BinaryPrimitives.ReadInt16LittleEndian(header[22..]) != 1
            || BinaryPrimitives.ReadInt32LittleEndian(header[24..]) != SampleRate || BinaryPrimitives.ReadInt16LittleEndian(header[34..]) != 16)
            throw new InvalidDataException("Die Aufnahme ist kein unterstütztes 16-kHz-PCM-WAV.");
        var dataBytes = (stream.Length - 44) / 2 * 2;
        if (dataBytes > SampleRate * 2L * 300) throw new InvalidDataException("Audioabschnitt unerwartet groß.");
        var signal = false; var buffer = new byte[32_000]; int read;
        while ((read = stream.Read(buffer)) > 0)
            for (var i = 0; i + 1 < read; i += 2)
                if (BinaryPrimitives.ReadInt16LittleEndian(buffer.AsSpan(i)) != 0) { signal = true; break; }
        if (BinaryPrimitives.ReadUInt32LittleEndian(header[40..]) != dataBytes || stream.Length != dataBytes + 44)
        {
            stream.SetLength(dataBytes + 44); WriteHeader(stream, dataBytes); stream.Flush(true);
        }
        return (dataBytes / 2, signal);
    }
}
