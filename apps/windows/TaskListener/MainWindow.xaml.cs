using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;

namespace TaskListener;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        this.InitializeComponent();
        Refresh();
        Core.Shared.OnEvent += (_, _) => DispatcherQueue.TryEnqueue(Refresh);
        FooterStatus.Text = Core.Shared.AudioIsReal
            ? "Listening"
            : "Audio: stubbed (build with --features audio)";
    }

    private void AddClick(object sender, RoutedEventArgs e)
    {
        Submit();
    }

    private void DraftBox_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter)
        {
            Submit();
            e.Handled = true;
        }
    }

    private void Submit()
    {
        var text = DraftBox.Text?.Trim();
        if (string.IsNullOrEmpty(text)) return;
        Core.Shared.CaptureManual(text);
        DraftBox.Text = string.Empty;
    }

    private void Refresh()
    {
        TaskList.ItemsSource = Core.Shared.ListTasks();
    }
}
