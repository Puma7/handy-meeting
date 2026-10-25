using System.IO;

namespace HandyMeeting;

/// <summary>Configured once by the platform entry point, before opening settings or sessions.</summary>
public static class AppStorage
{
    private static string? configured;
    public static string Root => configured ?? Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, ".."));
    public static void Configure(string folder)
    {
        var full = Path.GetFullPath(folder);
        if (configured is not null && configured != full) throw new InvalidOperationException("Storage is already configured.");
        Directory.CreateDirectory(full); configured = full;
    }
}
