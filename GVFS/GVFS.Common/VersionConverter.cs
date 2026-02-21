using System;
using System.Text.Json;
using System.Text.Json.Serialization;

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
        public override Version Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            if (reader.TokenType == JsonTokenType.Null)
            {
                return null;
            }

            if (reader.TokenType == JsonTokenType.String)
            {
                string versionString = reader.GetString();
                return new Version(versionString);
            }

            if (reader.TokenType == JsonTokenType.StartObject)
            {
                int major = 0, minor = 0, build = -1, revision = -1;

                while (reader.Read())
                {
                    if (reader.TokenType == JsonTokenType.EndObject)
                    {
                        break;
                    }

                    if (reader.TokenType == JsonTokenType.PropertyName)
                    {
                        string propertyName = reader.GetString();
                        reader.Read();

                        switch (propertyName)
                        {
                            case "Major":
                                major = reader.GetInt32();
                                break;
                            case "Minor":
                                minor = reader.GetInt32();
                                break;
                            case "Build":
                                build = reader.GetInt32();
                                break;
                            case "Revision":
                                revision = reader.GetInt32();
                                break;
                            default:
                                reader.Skip();
                                break;
                        }
                    }
                }

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

            throw new JsonException($"Unexpected token type '{reader.TokenType}' when deserializing System.Version.");
        }

        public override void Write(Utf8JsonWriter writer, Version value, JsonSerializerOptions options)
        {
            if (value == null)
            {
                writer.WriteNullValue();
            }
            else
            {
                writer.WriteStringValue(value.ToString());
            }
        }
    }
}
