using H.NotifyIcon;
using Microsoft.UI.Xaml;

namespace TaskListener;

public partial class App : Application
{
    private TaskbarIcon? _tray;
    private MainWindow? _window;

    public App()
    {
        this.InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Core.Shared.Start();

        _window = new MainWindow();
        _tray = (TaskbarIcon)Resources["TrayIcon"]!; // optional — define in Resources if needed
        // For a minimal skeleton, just show the window.
        _window.Activate();
    }
}
