// Shared GVFS pipe utilities — equivalent to GVFS.NativeHooks.Common in C++.
// Finds the GVFS named pipe and provides read/write helpers.

using System;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;

namespace GVFS.NativeHooks;

internal static partial class GvfsPipe
{
    private const byte ETX = 0x03;

    /// <summary>
    /// Find the GVFS enlistment root by walking up from cwd looking for .gvfs directory.
    /// </summary>
    internal static string FindEnlistmentRoot()
    {
        string dir = GetFinalPath(Directory.GetCurrentDirectory());

        while (!string.IsNullOrEmpty(dir))
        {
            if (Directory.Exists(Path.Combine(dir, ".gvfs")))
                return dir;

            dir = Path.GetDirectoryName(dir);
        }

        return null!;
    }

    /// <summary>
    /// Build the named pipe name from the enlistment root.
    /// Matches C++: uppercase, replace ':' with '_', prefix with "GVFS_".
    /// Example: D:\os → \\.\pipe\GVFS_D_\OS
    /// </summary>
    internal static string GetPipeName(string enlistmentRoot)
    {
        string normalized = enlistmentRoot.ToUpperInvariant().Replace(':', '_');
        if (!normalized.EndsWith('\\'))
            normalized += '\\';
        // Remove trailing backslash to match C++ behavior
        normalized = normalized.TrimEnd('\\');
        return @"\\.\pipe\GVFS_" + normalized;
    }

    /// <summary>
    /// Resolve the final path name (follows junctions/symlinks).
    /// </summary>
    internal static string GetFinalPath(string path)
    {
        using var handle = File.OpenHandle(path, FileMode.Open, FileAccess.Read,
            FileShare.ReadWrite | FileShare.Delete,
            FileOptions.Asynchronous,
            preallocationSize: 0);

        // Use GetFinalPathNameByHandle to resolve junctions
        Span<char> buffer = stackalloc char[512];
        uint len = GetFinalPathNameByHandle(handle.DangerousGetHandle(), buffer);
        if (len == 0 || len > buffer.Length)
            return path; // fallback to original

        string result = buffer[..(int)len].ToString();

        // Strip \\?\ prefix
        if (result.StartsWith(@"\\?\UNC\"))
            return @"\\" + result[@"\\?\UNC\".Length..];
        if (result.StartsWith(@"\\?\"))
            return result[@"\\?\".Length..];

        return result;
    }

    [LibraryImport("kernel32.dll", EntryPoint = "GetFinalPathNameByHandleW", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
    private static partial uint GetFinalPathNameByHandle(IntPtr hFile, Span<char> lpszFilePath, uint cchFilePath = 512, uint dwFlags = 0);

    /// <summary>
    /// Connect to the GVFS named pipe for the current enlistment.
    /// Dies on failure.
    /// </summary>
    internal static NamedPipeClientStream ConnectToGvfs(string appName)
    {
        string root = FindEnlistmentRoot();
        if (root == null)
        {
            Console.Error.WriteLine($"{appName} must be run from inside a GVFS enlistment");
            Environment.Exit(3);
        }

        string pipeName = GetPipeName(root);
        // Extract just the pipe name (after \\.\pipe\)
        string serverPipeName = pipeName[@"\\.\pipe\".Length..];

        var pipe = new NamedPipeClientStream(".", serverPipeName,
            PipeDirection.InOut, PipeOptions.None);

        try
        {
            pipe.Connect(3000); // 3 second timeout, same as C++
        }
        catch (TimeoutException)
        {
            Console.Error.WriteLine($"Could not open pipe: {pipeName}, Timed out.");
            Environment.Exit(5);
        }
        catch (IOException ex)
        {
            Console.Error.WriteLine($"Could not open pipe: {pipeName}, Error: {ex.HResult & 0xFFFF}");
            Environment.Exit(4);
        }

        return pipe;
    }

    /// <summary>
    /// Send ETX-terminated message and read response.
    /// </summary>
    internal static string SendMessage(NamedPipeClientStream pipe, string message)
    {
        // Write message + ETX
        byte[] msgBytes = Encoding.UTF8.GetBytes(message);
        pipe.Write(msgBytes);
        pipe.WriteByte(ETX);
        pipe.Flush();

        // Read until ETX
        var response = new MemoryStream(256);
        int b;
        while ((b = pipe.ReadByte()) != -1 && b != ETX)
        {
            response.WriteByte((byte)b);
        }

        return Encoding.UTF8.GetString(response.GetBuffer(), 0, (int)response.Length);
    }

    /// <summary>
    /// Send ETX-terminated message and read streaming response (for MPL).
    /// Returns the full response bytes after the initial "S" prefix.
    /// </summary>
    internal static byte[] SendMessageStreaming(NamedPipeClientStream pipe, string message)
    {
        byte[] msgBytes = Encoding.UTF8.GetBytes(message);
        pipe.Write(msgBytes);
        pipe.WriteByte(ETX);
        pipe.Flush();

        var response = new MemoryStream(4096);
        var buffer = new byte[1024];
        int bytesRead;
        bool firstRead = true;

        while ((bytesRead = pipe.Read(buffer, 0, buffer.Length)) > 0)
        {
            int offset = 0;
            int count = bytesRead;

            if (firstRead)
            {
                firstRead = false;
                if (buffer[0] != (byte)'S')
                {
                    string err = Encoding.UTF8.GetString(buffer, 0, bytesRead);
                    Console.Error.WriteLine($"Read response from pipe failed ({err})");
                    Environment.Exit(8);
                }
                // Skip "S\x03" prefix (2 bytes)
                offset = 2;
                count -= 2;
            }

            // Check for trailing ETX
            if (count > 0 && buffer[offset + count - 1] == ETX)
            {
                count--;
                response.Write(buffer, offset, count);
                break;
            }

            response.Write(buffer, offset, count);
        }

        return response.ToArray();
    }
}
