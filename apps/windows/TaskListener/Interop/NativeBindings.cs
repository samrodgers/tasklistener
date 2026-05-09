using System.Runtime.InteropServices;

namespace TaskListener.Interop;

/// <summary>
/// P/Invoke bindings for the Rust core (see crates/ffi/include/tasklistener.h).
/// Strings returned from the core must be freed with <see cref="tl_string_free"/>.
/// All strings cross the boundary as UTF-8 — we manage that explicitly because
/// the default marshaller would use UTF-16 on Windows.
/// </summary>
internal static partial class NativeBindings
{
    private const string Lib = "tasklistener";

    public delegate void TlEventCallback(nint json, nint ctx);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_start(string? db_path);

    [LibraryImport(Lib)]
    public static partial int tl_subscribe(TlEventCallback cb, nint ctx);

    [LibraryImport(Lib)]
    public static partial void tl_string_free(nint s);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial nint tl_capture_manual(string text);

    [LibraryImport(Lib)]
    public static partial nint tl_list_tasks(int include_done, long limit);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial nint tl_get_task(string id);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_update_task_text(string id, string text);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_set_task_status(string id, string status);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_delete_task(string id);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_set_provider(string config_json, string? token);

    [LibraryImport(Lib)]
    public static partial nint tl_list_providers();

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_delete_provider(string id);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial nint tl_list_targets(string provider_id);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_push_now(string task_id, string provider_id);

    [LibraryImport(Lib, StringMarshalling = StringMarshalling.Utf8)]
    public static partial int tl_record_external_push(
        string task_id,
        string provider_id,
        string? external_id,
        string? external_url,
        string? error);

    [LibraryImport(Lib)]
    public static partial int tl_audio_is_real();

    public static string? TakeString(nint ptr)
    {
        if (ptr == 0) return null;
        try
        {
            return Marshal.PtrToStringUTF8(ptr);
        }
        finally
        {
            tl_string_free(ptr);
        }
    }
}
