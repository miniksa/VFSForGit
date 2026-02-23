// pkt-line protocol helpers — matches git's pkt-line format.
// 4-hex-digit length prefix, content, '\n'. Flush = "0000".

using System;
using System.IO;
using System.Text;

namespace GVFS.NativeHooks;

internal static class PktLine
{
    /// <summary>Read a pkt-line from the stream. Returns content without trailing newline.</summary>
    internal static string ReadLine(Stream stream)
    {
        Span<byte> lenBuf = stackalloc byte[4];
        int read = ReadExact(stream, lenBuf);
        if (read == 0) throw new EndOfStreamException();
        if (read != 4) throw new InvalidDataException("Invalid packet length");

        int len = ParseHexLength(lenBuf);
        if (len == 0) return string.Empty; // flush packet

        if (len < 4)
            throw new InvalidDataException($"Bad line length: {len}");

        int dataLen = len - 4;
        byte[] data = new byte[dataLen];
        read = ReadExact(stream, data);
        if (read != dataLen)
            throw new InvalidDataException($"Short read: expected {dataLen}, got {read}");

        // Strip trailing newline
        int end = dataLen;
        if (end > 0 && data[end - 1] == '\n') end--;

        return Encoding.ASCII.GetString(data, 0, end);
    }

    /// <summary>Read a flush packet ("0000").</summary>
    internal static void ReadFlush(Stream stream)
    {
        string line = ReadLine(stream);
        // A flush packet returns empty string from ReadLine
        // (ParseHexLength returns 0 for "0000")
    }

    /// <summary>Write a pkt-line to the stream.</summary>
    internal static void WriteLine(Stream stream, string text)
    {
        int len = text.Length + 5; // 4 byte header + content + newline
        Span<byte> header = stackalloc byte[4];
        FormatHexLength(header, len);
        stream.Write(header);

        byte[] data = Encoding.ASCII.GetBytes(text);
        stream.Write(data);
        stream.WriteByte((byte)'\n');
        stream.Flush();
    }

    /// <summary>Write a flush packet.</summary>
    internal static void WriteFlush(Stream stream)
    {
        stream.Write("0000"u8);
        stream.Flush();
    }

    private static int ReadExact(Stream stream, Span<byte> buffer)
    {
        int total = 0;
        while (total < buffer.Length)
        {
            int read = stream.Read(buffer[total..]);
            if (read == 0) break;
            total += read;
        }
        return total;
    }

    private static int ParseHexLength(ReadOnlySpan<byte> hex)
    {
        int val = 0;
        for (int i = 0; i < 4; i++)
        {
            int digit = HexVal(hex[i]);
            if (digit < 0) throw new InvalidDataException($"Bad hex char: {(char)hex[i]}");
            val = (val << 4) | digit;
        }
        return val;
    }

    private static void FormatHexLength(Span<byte> buf, int len)
    {
        const string hex = "0123456789abcdef";
        buf[0] = (byte)hex[(len >> 12) & 0xF];
        buf[1] = (byte)hex[(len >> 8) & 0xF];
        buf[2] = (byte)hex[(len >> 4) & 0xF];
        buf[3] = (byte)hex[len & 0xF];
    }

    private static int HexVal(byte c) => c switch
    {
        >= (byte)'0' and <= (byte)'9' => c - '0',
        >= (byte)'a' and <= (byte)'f' => c - 'a' + 10,
        >= (byte)'A' and <= (byte)'F' => c - 'A' + 10,
        _ => -1,
    };
}
