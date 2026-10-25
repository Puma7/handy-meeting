using Avalonia;
using Avalonia.Headless;
using Avalonia.Threading;
using HandyMeeting;

internal static class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        var root = Path.GetFullPath(args[0]); Directory.CreateDirectory(root); AppStorage.Configure(root);
        AppBuilder.Configure<PortableApp>().UseSkia().UseHeadless(new AvaloniaHeadlessPlatformOptions { UseHeadlessDrawing = false }).SetupWithoutStarting();
        var window = new PortableWindow(); window.Show();
        for (var i = 0; i < 30; i++) { Dispatcher.UIThread.RunJobs(); Thread.Sleep(20); }
        using var frame = window.CaptureRenderedFrame() ?? throw new InvalidOperationException("Portable UI did not render.");
        frame.Save(Path.Combine(root, "portable-preview.png")); window.Close();
        JsonData.Save(Path.Combine(root, "ui-checks.json"), new { Passed = true, Headless = true, RealAudioCaptured = false, NativeWindowTested = false }); return 0;
    }
}
