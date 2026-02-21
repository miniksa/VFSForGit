using System.Text.Json.Serialization;

namespace GVFS.Service
{
    [JsonSerializable(typeof(RepoRegistration))]
    internal partial class ServiceJsonContext : JsonSerializerContext
    {
    }
}
