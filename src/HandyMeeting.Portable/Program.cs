using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Themes.Fluent;

namespace HandyMeeting;

internal static class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        if (args.FirstOrDefault() == "--stream-worker") { StreamingWorkerEntry.Run(); return 0; }
        if (args.FirstOrDefault() == "--synthetic-source") { PortableCaptureChecks.SyntheticSourceAsync(args).GetAwaiter().GetResult(); return 0; }
        if (args.Length == 2 && args[0] == "--portable-capture-host") { PortableCaptureChecks.HostAsync(args[1]).GetAwaiter().GetResult(); return 0; }
        var root = Environment.GetEnvironmentVariable("HANDYMEETING_DATA_DIR") ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "HandyMeeting");
        if (args.Length >= 2 && args[0] == "--portable-checks") root = Path.GetFullPath(args[1]);
        AppStorage.Configure(root);
        if (args.FirstOrDefault() == "--agent") return AgentProtocol.RunAsync(args.Skip(1).ToArray()).GetAwaiter().GetResult();
        if (args.FirstOrDefault() == "--portable-checks") return PortableChecks.RunAsync(root).GetAwaiter().GetResult();
        if (args.Length == 2 && args[0] == "--portable-capture-checks") return PortableCaptureChecks.RunAsync(Path.GetFullPath(args[1])).GetAwaiter().GetResult();
        if (args.Length == 4 && args[0] == "--portable-asr-check") return PortableChecks.NativeAsync(args[1], args[2], args[3]).GetAwaiter().GetResult();
        return AppBuilder.Configure<PortableApp>().UsePlatformDetect().WithInterFont().StartWithClassicDesktopLifetime(args);
    }
}
public sealed class PortableApp : Application
{
    public override void Initialize() => Styles.Add(new FluentTheme());
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop) desktop.MainWindow = new PortableWindow();
        base.OnFrameworkInitializationCompleted();
    }
}
