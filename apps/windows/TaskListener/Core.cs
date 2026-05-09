using System.Text.Json;
using TaskListener.Interop;

namespace TaskListener;

/// <summary>Managed wrapper over the Rust core. Singleton; lifetime = app.</summary>
public sealed class Core
{
    public static Core Shared { get; } = new();
    private bool _started;
    private readonly NativeBindings.TlEventCallback _callbackKeepAlive;

    public event EventHandler<CoreEvent>? OnEvent;

    public bool AudioIsReal { get; private set; }

    private Core()
    {
        _callbackKeepAlive = HandleEvent;
    }

    public void Start(string? dbPath = null)
    {
        if (_started) return;
        var rc = NativeBindings.tl_start(dbPath);
        if (rc != 0) throw new InvalidOperationException($"tl_start failed: {rc}");
        NativeBindings.tl_subscribe(_callbackKeepAlive, 0);
        AudioIsReal = NativeBindings.tl_audio_is_real() != 0;
        _started = true;
    }

    private void HandleEvent(nint jsonPtr, nint ctx)
    {
        var json = System.Runtime.InteropServices.Marshal.PtrToStringUTF8(jsonPtr);
        if (json == null) return;
        try
        {
            using var doc = JsonDocument.Parse(json);
            var kind = doc.RootElement.GetProperty("kind").GetString();
            OnEvent?.Invoke(this, new CoreEvent(kind ?? "unknown", json));
        }
        catch { /* ignore malformed events */ }
    }

    public string? CaptureManual(string text)
        => NativeBindings.TakeString(NativeBindings.tl_capture_manual(text));

    public IReadOnlyList<TaskItem> ListTasks(bool includeDone = false, long limit = 200)
    {
        var s = NativeBindings.TakeString(NativeBindings.tl_list_tasks(includeDone ? 1 : 0, limit));
        if (s == null) return Array.Empty<TaskItem>();
        using var doc = JsonDocument.Parse(s);
        var arr = doc.RootElement.GetProperty("tasks");
        return JsonSerializer.Deserialize<List<TaskItem>>(arr.GetRawText()) ?? new();
    }

    public IReadOnlyList<ProviderView> ListProviders()
    {
        var s = NativeBindings.TakeString(NativeBindings.tl_list_providers());
        if (s == null) return Array.Empty<ProviderView>();
        using var doc = JsonDocument.Parse(s);
        return JsonSerializer.Deserialize<List<ProviderView>>(
            doc.RootElement.GetProperty("providers").GetRawText()) ?? new();
    }

    public bool SetProvider(object cfg, string? token)
    {
        var json = JsonSerializer.Serialize(cfg);
        return NativeBindings.tl_set_provider(json, token) == 0;
    }

    public IReadOnlyList<ProviderTarget> ListTargets(string providerId)
    {
        var s = NativeBindings.TakeString(NativeBindings.tl_list_targets(providerId));
        if (s == null) return Array.Empty<ProviderTarget>();
        using var doc = JsonDocument.Parse(s);
        return JsonSerializer.Deserialize<List<ProviderTarget>>(
            doc.RootElement.GetProperty("targets").GetRawText()) ?? new();
    }

    public bool SetStatus(string id, TaskStatus status)
        => NativeBindings.tl_set_task_status(id, status.ToString().ToLowerInvariant()) == 0;

    public bool Delete(string id) => NativeBindings.tl_delete_task(id) == 0;
    public bool DeleteProvider(string id) => NativeBindings.tl_delete_provider(id) == 0;
    public bool PushNow(string taskId, string providerId) => NativeBindings.tl_push_now(taskId, providerId) == 0;
}

public record CoreEvent(string Kind, string RawJson);
