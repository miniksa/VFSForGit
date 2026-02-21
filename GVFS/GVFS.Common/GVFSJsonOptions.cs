using System.Text.Json;
using System.Text.Json.Serialization;

namespace GVFS.Common
{
    /// <summary>
    /// Shared JsonSerializerOptions for the GVFS codebase.
    /// Uses PropertyNameCaseInsensitive to match the behavior of
    /// Newtonsoft.Json (which was case-insensitive by default).
    /// </summary>
    public static class GVFSJsonOptions
    {
        public static readonly JsonSerializerOptions Default = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            Converters = { new VersionConverter() },
        };
    }
}
