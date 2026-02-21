using System.Text.Json;

namespace GVFS.Common
{
    public class VersionResponse
    {
        public string Version { get; set; }

        public static VersionResponse FromJsonString(string jsonString)
        {
            return (VersionResponse)JsonSerializer.Deserialize(jsonString, typeof(VersionResponse), GVFSJsonContext.Default);
        }
    }
}
