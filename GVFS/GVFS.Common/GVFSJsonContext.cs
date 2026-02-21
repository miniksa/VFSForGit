using GVFS.Common.Http;
using GVFS.Common.NamedPipes;
using GVFS.Common.Tracing;
using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace GVFS.Common
{
    /// <summary>
    /// Source-generated JSON serializer context for all types used in GVFS serialization.
    /// This enables trim-safe and AOT-compatible JSON serialization without reflection.
    /// </summary>
    [JsonSourceGenerationOptions(
        PropertyNameCaseInsensitive = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = new[] { typeof(VersionConverter), typeof(EventMetadataConverter) })]
    [JsonSerializable(typeof(string))]
    [JsonSerializable(typeof(Dictionary<string, string>))]
    [JsonSerializable(typeof(Dictionary<string, object>))]
    [JsonSerializable(typeof(EventMetadata))]
    [JsonSerializable(typeof(KeyValuePair<string, string>))]
    [JsonSerializable(typeof(List<string>))]
    [JsonSerializable(typeof(List<GitObjectsHttpRequestor.GitObjectSize>))]
    [JsonSerializable(typeof(ServerGVFSConfig))]
    [JsonSerializable(typeof(VersionResponse))]
    [JsonSerializable(typeof(InternalVerbParameters))]
    [JsonSerializable(typeof(CacheServerInfo))]
    [JsonSerializable(typeof(NamedPipeMessages.GetStatus.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.DehydrateFolders.Request))]
    [JsonSerializable(typeof(NamedPipeMessages.DehydrateFolders.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.Notification.Request))]
    [JsonSerializable(typeof(NamedPipeMessages.UnregisterRepoRequest))]
    [JsonSerializable(typeof(NamedPipeMessages.UnregisterRepoRequest.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.RegisterRepoRequest))]
    [JsonSerializable(typeof(NamedPipeMessages.RegisterRepoRequest.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.EnableAndAttachProjFSRequest))]
    [JsonSerializable(typeof(NamedPipeMessages.EnableAndAttachProjFSRequest.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.GetActiveRepoListRequest))]
    [JsonSerializable(typeof(NamedPipeMessages.GetActiveRepoListRequest.Response))]
    [JsonSerializable(typeof(NamedPipeMessages.BaseResponse<string>))]
    [JsonSerializable(typeof(TelemetryDaemonEventListener.PipeMessage))]
    [JsonSerializable(typeof(PrettyConsoleEventListener.ConsoleOutputPayload))]
    internal partial class GVFSJsonContext : JsonSerializerContext
    {
    }
}
