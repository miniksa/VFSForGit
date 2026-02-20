using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;

namespace GVFS.Common
{
    /// <summary>
    /// Custom JsonConverter for System.Version that handles both string format ("1.0.0.0")
    /// and object format ({"Major":1,"Minor":0,"Build":0,"Revision":0}).
    ///
    /// On .NET Framework, Newtonsoft.Json could deserialize System.Version from either format.
    /// On .NET Core/.NET 10, the default behavior changed and object-format deserialization
    /// fails because System.Version has additional read-only properties (MajorRevision,
    /// MinorRevision) that confuse the deserializer. This converter handles both formats.
    /// </summary>
    public class VersionConverter : JsonConverter<Version>
    {
        public override Version ReadJson(JsonReader reader, Type objectType, Version existingValue, bool hasExistingValue, JsonSerializer serializer)
        {
            if (reader.TokenType == JsonToken.Null)
            {
                return null;
            }

            if (reader.TokenType == JsonToken.String)
            {
                string versionString = reader.Value.ToString();
                return new Version(versionString);
            }

            if (reader.TokenType == JsonToken.StartObject)
            {
                JObject obj = JObject.Load(reader);
                int major = obj.Value<int>("Major");
                int minor = obj.Value<int>("Minor");
                int build = obj.Value<int>("Build");
                int revision = obj.Value<int>("Revision");

                if (build < 0)
                {
                    return new Version(major, minor);
                }

                if (revision < 0)
                {
                    return new Version(major, minor, build);
                }

                return new Version(major, minor, build, revision);
            }

            throw new JsonSerializationException($"Unexpected token type '{reader.TokenType}' when deserializing System.Version.");
        }

        public override void WriteJson(JsonWriter writer, Version value, JsonSerializer serializer)
        {
            if (value == null)
            {
                writer.WriteNull();
            }
            else
            {
                writer.WriteValue(value.ToString());
            }
        }
    }
}
