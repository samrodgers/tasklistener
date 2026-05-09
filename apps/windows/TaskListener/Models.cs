using System.Text.Json.Serialization;

namespace TaskListener;

public enum TaskStatus { Open, Done, Dismissed }
public enum DestinationState { Pending, Pushing, Pushed, Failed, DeadLetter }

public record TaskItem(
    string Id,
    string Text,
    [property: JsonPropertyName("due_hint")] string? DueHint,
    [property: JsonPropertyName("source_snippet")] string? SourceSnippet,
    [property: JsonPropertyName("captured_at")] long CapturedAt,
    string Status,
    float Confidence
);

public record TaskDestination(
    string Id,
    [property: JsonPropertyName("task_id")] string TaskId,
    string Provider,
    [property: JsonPropertyName("external_id")] string? ExternalId,
    [property: JsonPropertyName("external_url")] string? ExternalUrl,
    [property: JsonPropertyName("pushed_at")] long? PushedAt,
    [property: JsonPropertyName("last_error")] string? LastError,
    string State,
    int Attempts
);

public record ProviderView(
    string Id,
    string Kind,
    [property: JsonPropertyName("display_name")] string DisplayName,
    bool Enabled,
    [property: JsonPropertyName("config_json")] string ConfigJson,
    [property: JsonPropertyName("min_confidence")] float MinConfidence,
    [property: JsonPropertyName("auto_push")] bool AutoPush,
    [property: JsonPropertyName("target_id")] string? TargetId,
    [property: JsonPropertyName("target_label")] string? TargetLabel,
    [property: JsonPropertyName("token_masked")] string? TokenMasked
);

public record ProviderTarget(string Id, string Label);
