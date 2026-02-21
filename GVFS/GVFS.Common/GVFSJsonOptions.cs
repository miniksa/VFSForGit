using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.Json.Serialization.Metadata;

namespace GVFS.Common
{
    /// <summary>
    /// Shared JsonSerializerOptions for the GVFS codebase.
    /// Uses source-generated GVFSJsonContext for known types (trim-safe),
    /// with DefaultJsonTypeInfoResolver fallback for dynamic types like
    /// EventMetadata (Dictionary&lt;string, object&gt;) which can contain
    /// arbitrary value types at runtime.
    /// </summary>
    public static class GVFSJsonOptions
    {
        public static readonly JsonSerializerOptions Default = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            Converters = { new VersionConverter() },
            TypeInfoResolverChain = { GVFSJsonContext.Default, new DefaultJsonTypeInfoResolver() },
        };
    }
}
