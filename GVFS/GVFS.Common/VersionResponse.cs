using System.Text.Json;

namespace GVFS.Common
{
    public class VersionResponse
    {
        public string Version { get; set; }

        public static VersionResponse FromJsonString(string jsonString)
        {
            return JsonSerializer.Deserialize<VersionResponse>(jsonString, GVFSJsonOptions.Default);
        }
    }
}
