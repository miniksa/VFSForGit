using GVFS.Common.Tracing;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace GVFS.Common
{
    /// <summary>
    /// Shared JsonSerializerOptions for the GVFS codebase.
    /// Uses source-generated GVFSJsonContext for known types (trim-safe/AOT-safe).
    /// EventMetadata uses a custom converter that handles Dictionary&lt;string, object&gt;
    /// without reflection, making it NativeAOT compatible.
    /// </summary>
    public static class GVFSJsonOptions
    {
        public static readonly JsonSerializerOptions Default = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            Converters = { new VersionConverter(), new EventMetadataConverter() },
            TypeInfoResolverChain = { GVFSJsonContext.Default },
        };
    }
}
