// ReadObjectHook — git's read-object hook for downloading missing objects.
// Replaces GVFS.ReadObjectHook (C++).
//
// Protocol: pkt-line on stdin/stdout with git.
// Pipe protocol: DLO|<40-char-SHA>\x03 → S\x03 or F\x03

using System;
using System.IO;
using System.IO.Pipes;
using System.Text;

namespace GVFS.NativeHooks;

internal static class ReadObjectHook
{
    private const int ShaLength = 40;

    internal static int Run(string[] args)
    {
        using var stdin = Console.OpenStandardInput();
        using var stdout = Console.OpenStandardOutput();

        // === pkt-line handshake ===
        string welcome = PktLine.ReadLine(stdin);
        if (welcome != "git-read-object-client")
            return Program.Die("Bad welcome message");

        string version = PktLine.ReadLine(stdin);
        if (version != "version=1")
            return Program.Die("Bad version");

        PktLine.ReadFlush(stdin);

        PktLine.WriteLine(stdout, "git-read-object-server");
        PktLine.WriteLine(stdout, "version=1");
        PktLine.WriteFlush(stdout);

        string capability = PktLine.ReadLine(stdin);
        if (capability != "capability=get")
            return Program.Die("Bad capability");

        PktLine.ReadFlush(stdin);

        PktLine.WriteLine(stdout, "capability=get");
        PktLine.WriteFlush(stdout);

        // === Connect to GVFS named pipe ===
        using var pipe = GvfsPipe.ConnectToGvfs("GVFS.ReadObjectHook");

        // === Command loop ===
        while (true)
        {
            string command;
            try
            {
                command = PktLine.ReadLine(stdin);
            }
            catch (EndOfStreamException)
            {
                break; // git closed stdin
            }

            if (command != "command=get")
                return Program.Die("Bad command");

            string shaLine = PktLine.ReadLine(stdin);
            if (shaLine.Length != ShaLength + 5 || !shaLine.StartsWith("sha1="))
                return Program.Die("Bad sha1 in get command");

            PktLine.ReadFlush(stdin);

            string sha = shaLine[5..];
            bool success = DownloadObject(pipe, sha);

            PktLine.WriteLine(stdout, success ? "status=success" : "status=error");
            PktLine.WriteFlush(stdout);
        }

        return 0;
    }

    private static bool DownloadObject(NamedPipeClientStream pipe, string sha)
    {
        string response = GvfsPipe.SendMessage(pipe, $"DLO|{sha}");
        return response.Length > 0 && response[0] == 'S';
    }
}
