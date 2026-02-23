// PostIndexChangedHook — notifies GVFS when the git index changes.
// Replaces GVFS.PostIndexChangedHook (C++).
//
// Args: <updated-working-dir: 0|1> <updated-skipworktree: 0|1>
// Pipe: PICN|<flag1><flag2>\x03 → S...\x03

using System;
using System.IO.Pipes;

namespace GVFS.NativeHooks;

internal static class PostIndexChangedHook
{
    internal static int Run(string[] args)
    {
        if (args.Length != 2)
            return Program.Die("Invalid arguments", 1);

        string flag1 = args[0];
        string flag2 = args[1];

        if (flag1 != "0" && flag1 != "1")
            return Program.Die("Invalid value passed for first argument", 11);
        if (flag2 != "0" && flag2 != "1")
            return Program.Die("Invalid value passed for second argument", 11);

        using var pipe = GvfsPipe.ConnectToGvfs("GVFS.PostIndexChangedHook");

        string response = GvfsPipe.SendMessage(pipe, $"PICN|{flag1}{flag2}");

        if (response.Length == 0 || response[0] != 'S')
            return Program.Die($"Read response from pipe failed ({response})", 8);

        return 0;
    }
}
