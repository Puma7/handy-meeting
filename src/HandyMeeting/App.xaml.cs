using System.IO;
using System.Windows;

namespace HandyMeeting;

public partial class App : Application
{
    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        if (e.Args.FirstOrDefault() == "--agent")
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            if (Environment.GetEnvironmentVariable("HANDYMEETING_DATA_DIR") is { Length: > 0 } root) AppStorage.Configure(root);
            Shutdown(await AgentProtocol.RunAsync(e.Args.Skip(1).ToArray())); return;
        }
        if (e.Args.FirstOrDefault() == "--stream-worker")
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            try { await Task.Run(StreamingWorker.Run); Shutdown(0); }
            catch { Shutdown(1); }
            return;
        }
        if (e.Args.Length >= 2 && e.Args[0].StartsWith("--"))
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
            var code = 0;
            try { await Verification.RunAsync(e.Args); }
            catch (Exception error)
            {
                code = 1;
                Directory.CreateDirectory(e.Args[1]);
                JsonData.Save(Path.Combine(e.Args[1], "failure.json"), new { error = error.ToString() });
            }
            Shutdown(code); return;
        }
        new MainWindow().Show();
    }
}
