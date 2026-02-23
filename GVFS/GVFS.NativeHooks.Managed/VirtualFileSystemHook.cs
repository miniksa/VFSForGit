// VirtualFileSystemHook — provides the modified paths list to git.
// Replaces GVFS.VirtualFileSystemHook (C++).
//
// Args: <version: "1">
// Pipe: MPL|1\x03 → S\x03<streaming data>\x03 (written to stdout)

using System;
using System.IO;

namespace GVFS.NativeHooks;

internal static class VirtualFileSystemHook
{
    internal static int Run(string[] args)
    {
        if (args.Length != 1)
            return Program.Die("Invalid arguments", 11);
        if (args[0] != "1")
            return Program.Die("Bad version", 11);

        using var pipe = GvfsPipe.ConnectToGvfs("GVFS.VirtualFileSystemHook");
        using var stdout = Console.OpenStandardOutput();

        byte[] data = GvfsPipe.SendMessageStreaming(pipe, "MPL|1");
        stdout.Write(data);
        stdout.Flush();

        return 0;
    }
}
