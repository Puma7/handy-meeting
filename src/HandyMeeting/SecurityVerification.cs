using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text.Json;

namespace HandyMeeting;

/// <summary>Isolated synthetic filesystem/output regressions; never opens a device or service.</summary>
internal static class SecurityVerification
{
    [DllImport("kernel32.dll", EntryPoint = "CreateHardLinkW", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool WindowsHardLink(string name, string target, nint reserved);
    [DllImport("libc", EntryPoint = "link", SetLastError = true)]
    private static extern int UnixHardLink(string target, string name);
    private static void HardLink(string name, string target)
    {
        string Extended(string value) => value.StartsWith("\\\\", StringComparison.Ordinal) ? "\\\\?\\UNC\\" + value[2..] : "\\\\?\\" + Path.GetFullPath(value);
        var success = OperatingSystem.IsWindows() ? WindowsHardLink(Extended(name), Extended(target), 0) : UnixHardLink(target, name) == 0;
        if (!success) throw new IOException("Synthetic hard-link fixture could not be created: " + Marshal.GetLastPInvokeError());
    }
    /// <summary>Exercise real filesystem links, atomic replacement and inert exports using only owned synthetic sessions.</summary>
    internal static void Run(string parent, Action<bool, string> check)
    {
        var root = Path.Combine(parent, "security-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        string Hash(string file) => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(file)));
        void Rejected(Action action, string label)
        {
            var rejected = false;
            try { action(); } catch (InvalidDataException) { rejected = true; }
            check(rejected, label);
        }
        var ordinary = Path.Combine(root, "ordinary", "state.json");
        JsonData.SaveText(ordinary, "first"); JsonData.SaveText(ordinary, "second");
        check(File.ReadAllText(ordinary) == "second" && File.ReadAllText(ordinary + ".bak") == "first", "Atomic replacement preserves the previous ordinary file as backup");

        var outside = Path.Combine(root, "external-sentinel.txt");
        File.WriteAllText(outside, "Synthetic unrelated bytes must survive"); var original = Hash(outside);
        foreach (var hard in new[] { false, true })
        {
            var folder = Path.Combine(root, hard ? "hard-temp" : "symbolic-temp"); Directory.CreateDirectory(folder);
            var manifest = Path.Combine(folder, "session.json");
            File.WriteAllText(manifest, JsonSerializer.Serialize(new SessionData { Status = "complete", Title = "External ordinary session", ModelId = "fixture" }, JsonData.Options));
            if (hard) HardLink(manifest + ".tmp", outside); else File.CreateSymbolicLink(manifest + ".tmp", outside);
            _ = SessionStore.Open(manifest);
            check(Hash(outside) == original && File.Exists(manifest + ".tmp"), "Opening supplied session ignores preplanted " + (hard ? "hard" : "symbolic") + " temporary link and retains unrelated bytes");
        }
        foreach (var suffix in new[] { "", ".bak" })
        foreach (var dangling in new[] { false, true })
        {
            var folder = Path.Combine(root, "linked-destination-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(folder);
            var path = Path.Combine(folder, "state.json");
            if (suffix.Length != 0) File.WriteAllText(path, "original destination");
            var target = dangling ? Path.Combine(root, "missing-" + Guid.NewGuid().ToString("N")) : outside;
            File.CreateSymbolicLink(path + suffix, target);
            Rejected(() => JsonData.SaveText(path, "replacement"), "Reject " + (dangling ? "dangling " : "") + "destination/backup link before writing");
            check(Hash(outside) == original && (!dangling || !File.Exists(target)), "Linked destination/backup cannot create or change its external target");
        }
        var externalFolder = Path.Combine(root, "outside-folder"); Directory.CreateDirectory(externalFolder);
        var linkedFolder = Path.Combine(root, "linked-parent"); Directory.CreateSymbolicLink(linkedFolder, externalFolder);
        Rejected(() => JsonData.SaveText(Path.Combine(linkedFolder, "nested", "state.json"), "never"), "Linked ancestor is rejected before creating missing child directories");
        check(!Directory.EnumerateFileSystemEntries(externalFolder).Any(), "Rejected directory link leaves external directory empty");

        var finalHard = Path.Combine(root, "final-hardlink.txt"); HardLink(finalHard, outside);
        JsonData.SaveText(finalHard, "new directory-entry bytes");
        check(Hash(outside) == original && File.ReadAllText(finalHard) == "new directory-entry bytes", "Atomic replacement does not truncate a final hardlink's other name");
        var failed = Path.Combine(root, "blocked-placement.txt"); File.WriteAllText(failed, "original"); Directory.CreateDirectory(failed + ".bak");
        Rejected(() => JsonData.SaveText(failed, "never"), "Unexpected backup directory fails closed");
        check(File.ReadAllText(failed) == "original" && Directory.Exists(failed + ".bak"), "Failed replacement preserves original destination and backup entry");

        var imported = Path.Combine(root, "linked-audio-session"); Directory.CreateDirectory(imported);
        var importedManifest = Path.Combine(imported, "session.json");
        File.WriteAllText(importedManifest, JsonSerializer.Serialize(new SessionData { Status = "complete" }, JsonData.Options));
        Directory.CreateSymbolicLink(Path.Combine(imported, "audio"), externalFolder);
        var beforeManifest = Hash(importedManifest);
        Rejected(() => SessionStore.Open(importedManifest), "Imported linked audio directory is rejected before recovery writes");
        check(Hash(importedManifest) == beforeManifest && !File.Exists(Path.Combine(imported, "session.lock")), "Rejected supplied session remains unchanged before acquiring its lock");

        var outsideWav = Path.Combine(root, "external-recording.wav");
        using (var wav = File.Create(outsideWav)) { PcmTimeline.WriteHeader(wav, 0); wav.Write(new byte[3200]); wav.WriteByte(17); }
        var originalWav = Hash(outsideWav);
        var linkedAudio = SessionStore.Create(Path.Combine(root, "hard-audio"), "Supplied hardlinked WAV", "fixture", new("synthetic", "Synthetic"), null);
        var linkedWav = Path.Combine(linkedAudio.Folder, "audio", "system-000000000000.wav"); HardLink(linkedWav, outsideWav);
        var repaired = SessionStore.Open(linkedAudio.ManifestPath);
        check(Hash(outsideWav) == originalWav && Hash(linkedWav + ".bak") == originalWav && new FileInfo(linkedWav).Length == 3244 && repaired.Data.Parts.Single().SampleCount == 1600, "Recovery repairs its own WAV copy while preserving a hardlinked external original and exact backup");
        var outsideJournal = Path.Combine(root, "external-journal.txt"); File.WriteAllText(outsideJournal, "Synthetic unrelated journal prefix without newline");
        var originalJournal = Hash(outsideJournal); HardLink(repaired.JournalPath, outsideJournal);
        var reopened = SessionStore.Open(repaired.ManifestPath); var audio = reopened.PartsSnapshot().Single();
        reopened.PersistText(audio, "fixture", new() { Full = "New confirmed text", Committed = "New confirmed text" }, audio.SampleCount, true);
        check(Hash(outsideJournal) == originalJournal && Hash(reopened.JournalPath + ".bak") == originalJournal && File.ReadAllText(reopened.JournalPath).Contains("New confirmed text"), "First journal append detaches supplied hardlink without changing its external sibling or previous bytes");

        const string hostile = "![remote](https://example.invalid/pixel) <img src='https://example.invalid/raw'> &copy;\n[ref]: https://example.invalid/ref\n`code` \\literal";
        var sessionFolder = Path.Combine(root, "markdown"); Directory.CreateDirectory(sessionFolder);
        var data = new SessionData { Status = "complete", Title = hostile, ModelId = hostile, Warnings = [hostile] };
        data.Parts.Add(new() { File = "system-000000000000.wav", ModelId = hostile, Status = "complete", Text = hostile, SampleCount = 16000 });
        var store = new SessionStore(sessionFolder, data); var snapshot = JsonSerializer.Serialize(data, JsonData.Options);
        Transcription.Export(store);
        var md = File.ReadAllText(Path.Combine(sessionFolder, "transcript.md"));
        var txt = File.ReadAllText(Path.Combine(sessionFolder, "transcript.txt"));
        check(!md.Contains("![remote](") && !md.Contains("<img") && !md.Contains("\n[ref]:") && !md.Contains("`code`"), "Transcript Markdown cannot activate supplied image, HTML, reference-link or code delimiters");
        check(md.Split("\\!\\[remote\\]", StringSplitOptions.None).Length == 5 && md.Contains("&lt;img") && md.Contains("&amp;copy;") && md.Contains("\\\\literal"), "Markdown escaping covers title, model, warnings and multiline transcript body");
        check(txt.Contains(hostile) && JsonSerializer.Serialize(JsonData.Read<SessionData>(Path.Combine(sessionFolder, "transcript.json")), JsonData.Options) == snapshot && JsonSerializer.Serialize(data, JsonData.Options) == snapshot, "TXT, JSON and original session values remain literal and unchanged");
        ReviewRegressions(root, check);
    }

    /// <summary>Reproduce nullable imports and write/placement failures while checking preservation and bounded temporary-file cleanup.</summary>
    private static void ReviewRegressions(string parent, Action<bool, string> check)
    {
        var root = Path.Combine(parent, "review-regressions"); Directory.CreateDirectory(root);
        var results = new List<(bool Passed, string Label)>();
        void Record(bool passed, string label) => results.Add((passed, label));
        string[] Pending(string folder) => Directory.GetFiles(folder, ".*.pending");

        var imported = Path.Combine(root, "nullable-session"); Directory.CreateDirectory(imported);
        var manifest = Path.Combine(imported, "session.json");
        File.WriteAllText(manifest, "{\"SchemaVersion\":1,\"Title\":\"Nullable imported metadata\",\"Status\":\"complete\",\"ModelId\":null,\"Warnings\":[null],\"Parts\":[]}");
        var opened = false;
        try { _ = SessionStore.Open(manifest); opened = true; }
        catch (NullReferenceException) { }
        Record(opened && new[] { "transcript.md", "transcript.txt", "transcript.json" }.All(name => File.Exists(Path.Combine(imported, name))), "Imported null model and warning values still allow reopening and every transcript export");
        var exported = Path.Combine(imported, "transcript.json");
        var nullsRetained = false;
        if (File.Exists(exported))
        {
            using var json = JsonDocument.Parse(File.ReadAllText(exported));
            nullsRetained = json.RootElement.GetProperty("ModelId").ValueKind == JsonValueKind.Null
                && json.RootElement.GetProperty("Warnings")[0].ValueKind == JsonValueKind.Null;
        }
        Record(nullsRetained, "Nullable imported metadata is preserved in JSON rather than silently rewritten");

        foreach (var exists in new[] { false, true })
        {
            var folder = Path.Combine(root, exists ? "partial-existing" : "partial-new"); Directory.CreateDirectory(folder);
            var path = Path.Combine(folder, "state.json"); if (exists) File.WriteAllText(path, "original");
            File.WriteAllText(path + ".tmp", "unrelated supplied temporary file");
            for (var attempt = 0; attempt < 3; attempt++)
            {
                try { JsonData.WriteAtomic(path, stream => { stream.Write("partial"u8); throw new IOException("Synthetic mid-write failure"); }); }
                catch (IOException) { }
            }
            Record(Pending(folder).Length == 0 && (exists ? File.ReadAllText(path) == "original" : !File.Exists(path))
                && File.ReadAllText(path + ".tmp") == "unrelated supplied temporary file", "Failed owned partial writes do not accumulate files or change original/supplied bytes: " + exists);
        }
        foreach (var exists in new[] { false, true })
        {
            var folder = Path.Combine(root, exists ? "placement-existing" : "placement-new"); Directory.CreateDirectory(folder);
            var path = Path.Combine(folder, "state.json"); if (exists) File.WriteAllText(path, "original");
            for (var attempt = 0; attempt < 3; attempt++)
            {
                var backup = path + ".bak";
                try { JsonData.WriteAtomic(path, stream => { stream.Write("complete new bytes"u8); Directory.CreateDirectory(backup); }); }
                catch (InvalidDataException) { }
                Directory.Delete(backup); // Only the empty directory created by this fixture.
            }
            var pending = Pending(folder);
            Record(exists ? pending.Length == 0 && File.ReadAllText(path) == "original"
                : pending.Length == 3 && !File.Exists(path) && pending.All(file => File.ReadAllText(file) == "complete new bytes"),
                exists ? "Failed placement keeps the canonical original without accumulating owned pending files"
                    : "Failed placement without a canonical destination retains every complete pending copy");
        }
        // Preserve the independently exercised outcomes even when an old implementation
        // causes the first reported assertion to fail.
        JsonData.Save(Path.Combine(root, "outcomes.json"), results.Select(item => new { item.Passed, item.Label }));
        foreach (var result in results) check(result.Passed, result.Label);
    }
}
