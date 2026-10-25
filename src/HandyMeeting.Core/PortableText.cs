using System.Diagnostics;

namespace HandyMeeting;

public static class Ui
{
    public static bool English { get; set; }
    public static string T(string text) => !English ? text : text switch
    {
        "KI-Entwurf · Inhalt und Belege prüfen." => "AI draft · Review content and evidence.", "Keine belegten Einträge." => "No supported entries.",
        "Zuständig" => "Owner", "Termin" => "Due", "nicht genannt" => "Not specified", _ => text
    };
}
public sealed record MemorySample(long PrivateBytes, long WorkingBytes, long ManagedBytes, long AllocatedBytes, int Gen0, int Gen1, int Gen2)
{
    public static MemorySample Read()
    {
        using var process = Process.GetCurrentProcess();
        return new(process.PrivateMemorySize64, process.WorkingSet64, GC.GetTotalMemory(false), GC.GetTotalAllocatedBytes(), GC.CollectionCount(0), GC.CollectionCount(1), GC.CollectionCount(2));
    }
}
