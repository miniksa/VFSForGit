using System.Text.Json;
using System.Text.Json.Serialization;

namespace GVFS.Common
{
    /// <summary>
    /// Shared JsonSerializerOptions for the GVFS codebase.
    /// Uses the source-generated GVFSJsonContext for trim-safe and AOT-compatible
    /// JSON serialization. PropertyNameCaseInsensitive matches the legacy
    /// Newtonsoft.Json behavior.
    /// </summary>
    public static class GVFSJsonOptions
    {
        public static readonly JsonSerializerOptions Default = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            Converters = { new VersionConverter() },
            TypeInfoResolverChain = { GVFSJsonContext.Default },
        };
    }
}
