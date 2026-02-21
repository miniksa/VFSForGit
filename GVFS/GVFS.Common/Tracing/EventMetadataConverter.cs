using System;
using System.Collections.Generic;
using System.Net;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace GVFS.Common.Tracing
{
    /// <summary>
    /// AOT-compatible JSON converter for EventMetadata (Dictionary&lt;string, object&gt;).
    /// Handles the known value types stored in EventMetadata without reflection:
    /// string, int, long, bool, double, and enums (serialized as their numeric value).
    /// </summary>
    public class EventMetadataConverter : JsonConverter<EventMetadata>
    {
        public override EventMetadata Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            if (reader.TokenType != JsonTokenType.StartObject)
            {
                throw new JsonException("Expected StartObject");
            }

            var metadata = new EventMetadata();
            while (reader.Read())
            {
                if (reader.TokenType == JsonTokenType.EndObject)
                {
                    return metadata;
                }

                string key = reader.GetString();
                reader.Read();

                object value = reader.TokenType switch
                {
                    JsonTokenType.String => reader.GetString(),
                    JsonTokenType.Number when reader.TryGetInt32(out int i) => i,
                    JsonTokenType.Number when reader.TryGetInt64(out long l) => l,
                    JsonTokenType.Number when reader.TryGetDouble(out double d) => d,
                    JsonTokenType.True => true,
                    JsonTokenType.False => false,
                    JsonTokenType.Null => null,
                    _ => reader.GetString() // fallback: treat as string
                };

                metadata[key] = value;
            }

            throw new JsonException("Unexpected end of JSON");
        }

        public override void Write(Utf8JsonWriter writer, EventMetadata value, JsonSerializerOptions options)
        {
            writer.WriteStartObject();
            foreach (KeyValuePair<string, object> kvp in value)
            {
                writer.WritePropertyName(kvp.Key);
                WriteValue(writer, kvp.Value);
            }
            writer.WriteEndObject();
        }

        private static void WriteValue(Utf8JsonWriter writer, object value)
        {
            switch (value)
            {
                case null:
                    writer.WriteNullValue();
                    break;
                case string s:
                    writer.WriteStringValue(s);
                    break;
                case int i:
                    writer.WriteNumberValue(i);
                    break;
                case long l:
                    writer.WriteNumberValue(l);
                    break;
                case double d:
                    writer.WriteNumberValue(d);
                    break;
                case bool b:
                    writer.WriteBooleanValue(b);
                    break;
                case float f:
                    writer.WriteNumberValue(f);
                    break;
                case HttpStatusCode status:
                    writer.WriteNumberValue((int)status);
                    break;
                case Enum e:
                    writer.WriteStringValue(e.ToString());
                    break;
                default:
                    // For any other type, convert to string
                    writer.WriteStringValue(value.ToString());
                    break;
            }
        }
    }
}
