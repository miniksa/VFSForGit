using System.Text.Json.Serialization;

namespace GVFS.GVFlt
{
    [JsonSerializable(typeof(GVFltCallbacks.BackgroundGitUpdate))]
    internal partial class GVFltJsonContext : JsonSerializerContext
    {
    }
}
