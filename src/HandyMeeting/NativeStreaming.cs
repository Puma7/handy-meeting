using System.Buffers.Binary;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace HandyMeeting;

internal static class NativeAsr
{
    private const string Library = "transcribe.dll";
    internal static string DirectoryPath => Path.Combine(AppContext.BaseDirectory, "native");
    static NativeAsr()
    {
        if (!Environment.Is64BitProcess) throw new PlatformNotSupportedException("Streaming benötigt einen 64-Bit-Prozess.");
        if (OperatingSystem.IsWindows()) SetDllDirectory(DirectoryPath);
        var file = OperatingSystem.IsWindows() ? Library : OperatingSystem.IsMacOS() ? "libtranscribe.dylib" : "libtranscribe.so";
        NativeLibrary.SetDllImportResolver(typeof(NativeAsr).Assembly, (name, _, _) => name == Library ? NativeLibrary.Load(Path.Combine(DirectoryPath, file)) : IntPtr.Zero);
    }
    [DllImport("kernel32", CharSet = CharSet.Unicode)] private static extern bool SetDllDirectory(string path);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr transcribe_version();
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr transcribe_status_string(int status);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_log_set(IntPtr callback, IntPtr userData);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_init_backends([MarshalAs(UnmanagedType.LPUTF8Str)] string directory);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_backend_init_params_init(ref BackendParams options);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_init_backends_ex(ref BackendParams options);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_model_load_params_init(ref LoadParams options);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_model_load_file([MarshalAs(UnmanagedType.LPUTF8Str)] string path, ref LoadParams options, out IntPtr model);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_model_free(IntPtr model);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr transcribe_model_backend(IntPtr model);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_capabilities_init(ref Capabilities capabilities);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_model_get_capabilities(IntPtr model, ref Capabilities capabilities);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_session_init(IntPtr model, IntPtr options, out IntPtr session);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_session_free(IntPtr session);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_stream_begin(IntPtr session, IntPtr runOptions, IntPtr streamOptions);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_stream_feed(IntPtr session, float[] samples, int count, IntPtr update);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_stream_finalize(IntPtr session, IntPtr update);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern void transcribe_stream_text_init(ref StreamText text);
    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)] internal static extern int transcribe_stream_get_text(IntPtr session, ref StreamText text);
    [StructLayout(LayoutKind.Sequential)] internal struct LoadParams { public ulong Size; public int Backend; public IntPtr Device; public byte NoMmap; }
    [StructLayout(LayoutKind.Sequential)] internal struct BackendParams { public ulong Size; public IntPtr ArtifactDirectory; public uint Allowed; }
    [StructLayout(LayoutKind.Sequential)] internal struct Capabilities
    {
        public ulong Size; public int SampleRate, LanguageCount; public IntPtr Languages; public int Timestamps;
        public byte LanguageDetect, Translate, Streaming, SpecDecode;
        public long MaxAudioMs; public int TranslationLanguageCount; public IntPtr TranslationLanguages;
    }
    [StructLayout(LayoutKind.Sequential)] internal struct StreamText
    {
        public ulong Size; public IntPtr Full; public ulong FullBytes; public IntPtr Committed; public ulong CommittedBytes;
        public IntPtr Tentative; public ulong TentativeBytes, RawStart;
    }
    internal static string Text(IntPtr value) => Marshal.PtrToStringUTF8(value) ?? "";
    internal static void Check(int code)
    {
        if (code != 0) throw new InvalidOperationException($"Lokale Streaming-Engine: {Text(transcribe_status_string(code))} ({code}).");
    }
}

public sealed class StreamCommand
{
    public string Operation { get; set; } = "";
    public string? ModelPath { get; set; }
    public bool CpuOnly { get; set; }
    public string Source { get; set; } = "system";
    public string? AudioPath { get; set; }
    public long OffsetSamples { get; set; }
    public int SampleCount { get; set; }
}
public sealed class StreamReply
{
    [System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    public MemorySample? Memory { get; set; }
    public bool Ok { get; set; } = true;
    public string? Error { get; set; }
    public string Full { get; set; } = "";
    public string Committed { get; set; } = "";
    public string Tentative { get; set; } = "";
    public string Backend { get; set; } = "";
}

internal static class StreamingWorker
{
    public static void Run()
    {
        Console.InputEncoding = new UTF8Encoding(false); Console.OutputEncoding = new UTF8Encoding(false);
        var model = IntPtr.Zero; var sessions = new Dictionary<string, IntPtr>();
        // Commands execute serially. Reuse bounded buffers; native feed consumes them synchronously.
        var pcmBytes = new byte[16_000 * 2]; var pcmSamples = new float[16_000];
        try
        {
            string? line;
            while ((line = Console.ReadLine()) is not null)
            {
                StreamReply reply;
                try
                {
                    var command = JsonSerializer.Deserialize<StreamCommand>(line, JsonData.Options) ?? throw new InvalidDataException("Leerer Streaming-Auftrag.");
                    reply = Execute(command);
                }
                catch (Exception error) { reply = new() { Ok = false, Error = error.Message }; }
                Console.WriteLine(JsonSerializer.Serialize(reply)); Console.Out.Flush();
            }
        }
        finally
        {
            foreach (var session in sessions.Values) NativeAsr.transcribe_session_free(session);
            if (model != IntPtr.Zero) NativeAsr.transcribe_model_free(model);
        }
        StreamReply Execute(StreamCommand command)
        {
            if (command.Operation == "metrics") return new() { Memory = MemorySample.Read() };
            if (command.Operation == "load")
            {
                if (model != IntPtr.Zero) throw new InvalidOperationException("Modell ist bereits geladen.");
                if (NativeAsr.Text(NativeAsr.transcribe_version()) != "0.4.1") throw new InvalidOperationException("Die Streaming-Laufzeit hat eine nicht unterstützte Version.");
                NativeAsr.transcribe_log_set(IntPtr.Zero, IntPtr.Zero);
                if (command.CpuOnly)
                {
                    var backend = new NativeAsr.BackendParams(); NativeAsr.transcribe_backend_init_params_init(ref backend);
                    backend.ArtifactDirectory = Marshal.StringToCoTaskMemUTF8(NativeAsr.DirectoryPath); backend.Allowed = 1;
                    try { NativeAsr.Check(NativeAsr.transcribe_init_backends_ex(ref backend)); }
                    finally { Marshal.FreeCoTaskMem(backend.ArtifactDirectory); }
                }
                else NativeAsr.Check(NativeAsr.transcribe_init_backends(NativeAsr.DirectoryPath));
                var options = new NativeAsr.LoadParams(); NativeAsr.transcribe_model_load_params_init(ref options);
                if (options.Size != (ulong)Marshal.SizeOf<NativeAsr.LoadParams>()) throw new InvalidDataException("Modell-ABI stimmt nicht überein.");
                if (command.CpuOnly) options.Backend = 1;
                NativeAsr.Check(NativeAsr.transcribe_model_load_file(command.ModelPath ?? "", ref options, out model));
                var capabilities = new NativeAsr.Capabilities(); NativeAsr.transcribe_capabilities_init(ref capabilities);
                if (capabilities.Size != (ulong)Marshal.SizeOf<NativeAsr.Capabilities>()) throw new InvalidDataException("Modellfähigkeiten-ABI stimmt nicht überein.");
                NativeAsr.Check(NativeAsr.transcribe_model_get_capabilities(model, ref capabilities));
                if (capabilities.Streaming == 0) throw new InvalidOperationException("Dieses Modell unterstützt keinen echten Audiostream.");
                return new() { Backend = NativeAsr.Text(NativeAsr.transcribe_model_backend(model)) };
            }
            if (model == IntPtr.Zero) throw new InvalidOperationException("Das Streaming-Modell ist noch nicht geladen.");
            if (command.Source is not ("system" or "microphone")) throw new InvalidDataException("Unbekannte Audiospur.");
            if (command.Operation == "begin")
            {
                if (sessions.Remove(command.Source, out var previous)) NativeAsr.transcribe_session_free(previous);
                NativeAsr.Check(NativeAsr.transcribe_session_init(model, IntPtr.Zero, out var created));
                sessions.Add(command.Source, created);
                NativeAsr.Check(NativeAsr.transcribe_stream_begin(created, IntPtr.Zero, IntPtr.Zero));
                return new();
            }
            if (!sessions.TryGetValue(command.Source, out var current)) throw new InvalidOperationException("Kein Stream für diese Spur.");
            if (command.Operation == "feed")
            {
                if (command.SampleCount is < 1 or > 16_000 || command.OffsetSamples < 0) throw new InvalidDataException("Ungültiges Streaming-Fenster.");
                using (var audio = new FileStream(command.AudioPath!, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
                { audio.Position = checked(44 + command.OffsetSamples * 2); audio.ReadExactly(pcmBytes.AsSpan(0, command.SampleCount * 2)); }
                for (var i = 0; i < command.SampleCount; i++) pcmSamples[i] = BinaryPrimitives.ReadInt16LittleEndian(pcmBytes.AsSpan(i * 2)) / 32768f;
                NativeAsr.Check(NativeAsr.transcribe_stream_feed(current, pcmSamples, command.SampleCount, IntPtr.Zero));
            }
            else if (command.Operation == "finalize") NativeAsr.Check(NativeAsr.transcribe_stream_finalize(current, IntPtr.Zero));
            else throw new InvalidDataException("Unbekannter Streaming-Auftrag.");
            var result = new NativeAsr.StreamText(); NativeAsr.transcribe_stream_text_init(ref result);
            if (result.Size != (ulong)Marshal.SizeOf<NativeAsr.StreamText>()) throw new InvalidDataException("Text-ABI stimmt nicht überein.");
            NativeAsr.Check(NativeAsr.transcribe_stream_get_text(current, ref result));
            return new() { Full = NativeAsr.Text(result.Full), Committed = NativeAsr.Text(result.Committed), Tentative = NativeAsr.Text(result.Tentative) };
        }
    }
}

public sealed class StreamingProcess : IAsyncDisposable
{
    private readonly Process process;
    private readonly Task stderr;
    private readonly WorkerJob job;
    public string Backend { get; private set; } = "";
    public int ProcessId => process.Id;
    private StreamingProcess()
    {
        var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardInput = true, RedirectStandardOutput = true, RedirectStandardError = true, StandardInputEncoding = new UTF8Encoding(false), StandardOutputEncoding = Encoding.UTF8 };
        info.ArgumentList.Add("--stream-worker");
        // Match Handy's Vulkan posture and keep third-party graphics hooks out of this process.
        if (!info.Environment.ContainsKey("VK_LOADER_LAYERS_DISABLE")) info.Environment["VK_LOADER_LAYERS_DISABLE"] = "~implicit~";
        process = Process.Start(info) ?? throw new InvalidOperationException("Streaming-Prozess konnte nicht starten.");
        job = new WorkerJob(process);
        stderr = Task.Run(async () => { while (await process.StandardError.ReadLineAsync() is not null) { } });
    }
    public static async Task<StreamingProcess> StartAsync(string modelPath, bool cpuOnly, CancellationToken cancellation)
    {
        var worker = new StreamingProcess();
        try
        {
            var reply = await worker.CallAsync(new() { Operation = "load", ModelPath = modelPath, CpuOnly = cpuOnly }, cancellation, TimeSpan.FromMinutes(2));
            worker.Backend = reply.Backend; return worker;
        }
        catch { await worker.DisposeAsync(); throw; }
    }
    public async Task<StreamReply> CallAsync(StreamCommand command, CancellationToken cancellation, TimeSpan? timeout = null)
    {
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellation); deadline.CancelAfter(timeout ?? TimeSpan.FromSeconds(30));
        try
        {
            await process.StandardInput.WriteLineAsync(JsonSerializer.Serialize(command).AsMemory(), deadline.Token);
            await process.StandardInput.FlushAsync(deadline.Token);
            var line = await process.StandardOutput.ReadLineAsync(deadline.Token) ?? throw new IOException("Der lokale Streaming-Prozess wurde beendet.");
            var reply = JsonSerializer.Deserialize<StreamReply>(line, JsonData.Options) ?? throw new InvalidDataException("Leere Streaming-Antwort.");
            if (!reply.Ok) throw new InvalidOperationException(reply.Error); return reply;
        }
        catch (OperationCanceledException) when (!cancellation.IsCancellationRequested)
        { throw new TimeoutException("Die lokale Streaming-Engine reagiert nicht. Das Audio wird weiter gesichert."); }
    }
    public void KillForTest() { if (!process.HasExited) process.Kill(true); }
    public async ValueTask DisposeAsync()
    {
        try
        {
            if (!process.HasExited)
            {
                process.StandardInput.Close();
                try { await process.WaitForExitAsync().WaitAsync(TimeSpan.FromSeconds(3)); }
                catch (TimeoutException) { process.Kill(true); await process.WaitForExitAsync(); }
            }
            await stderr;
        }
        finally { job.Dispose(); process.Dispose(); }
    }
}

internal sealed class WorkerJob : IDisposable
{
    private IntPtr handle;
    public WorkerJob(Process child)
    {
        if (!OperatingSystem.IsWindows()) return; // Unix worker exits on pipe EOF; parent owns and awaits it during normal shutdown.
        handle = CreateJobObject(IntPtr.Zero, null);
        var info = new ExtendedInfo(); info.Basic.LimitFlags = 0x2000; // KILL_ON_JOB_CLOSE
        if (handle == IntPtr.Zero || !SetInformationJobObject(handle, 9, ref info, (uint)Marshal.SizeOf<ExtendedInfo>()) || !AssignProcessToJobObject(handle, child.Handle))
        {
            var error = Marshal.GetLastWin32Error(); Dispose();
            try { child.Kill(true); } catch (InvalidOperationException) { }
            throw new System.ComponentModel.Win32Exception(error, "Streaming-Prozess konnte nicht an die App gebunden werden.");
        }
    }
    public void Dispose() { if (handle != IntPtr.Zero) { CloseHandle(handle); handle = IntPtr.Zero; } }
    [StructLayout(LayoutKind.Sequential)] private struct BasicInfo { public long ProcessTime, JobTime; public uint LimitFlags; public UIntPtr MinWorkingSet, MaxWorkingSet; public uint ActiveProcessLimit; public UIntPtr Affinity; public uint PriorityClass, SchedulingClass; }
    [StructLayout(LayoutKind.Sequential)] private struct IoInfo { public ulong ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes; }
    [StructLayout(LayoutKind.Sequential)] private struct ExtendedInfo { public BasicInfo Basic; public IoInfo Io; public UIntPtr ProcessMemory, JobMemory, PeakProcessMemory, PeakJobMemory; }
    [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)] private static extern IntPtr CreateJobObject(IntPtr attributes, string? name);
    [DllImport("kernel32", SetLastError = true)] private static extern bool SetInformationJobObject(IntPtr job, int infoClass, ref ExtendedInfo info, uint length);
    [DllImport("kernel32", SetLastError = true)] private static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32")] private static extern bool CloseHandle(IntPtr handle);
}
