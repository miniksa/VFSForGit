// GVFS.NativeHooks — C# NativeAOT replacement for the 4 C++ hook executables.
//
// This single binary dispatches based on its executable name (argv[0]):
//   GVFS.ReadObjectHook.exe      → ReadObjectHook
//   GVFS.PostIndexChangedHook.exe → PostIndexChangedHook
//   GVFS.VirtualFileSystemHook.exe → VirtualFileSystemHook
//   GitHooksLoader.exe            → GitHooksLoader
//
// After NativeAOT publish, copy the exe with the 4 different names.

using System;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;

namespace GVFS.NativeHooks;

internal static class Program
{
    static int Main(string[] args)
    {
        string exeName = Path.GetFileNameWithoutExtension(Environment.ProcessPath ?? "unknown");

        return exeName.ToUpperInvariant() switch
        {
            "GVFS.READOBJECTHOOK" => ReadObjectHook.Run(args),
            "GVFS.POSTINDEXCHANGEDHOOK" => PostIndexChangedHook.Run(args),
            "GVFS.VIRTUALFILESYSTEMHOOK" => VirtualFileSystemHook.Run(args),
            "GITHOOKSLOADER" => GitHooksLoader.Run(args),
            "GVFS.NATIVEHOOKS" => RunFromArg(args), // for testing: pass hook name as first arg
            _ => RunFromArg(args), // fallback: try first arg as hook name
        };
    }

    static int RunFromArg(string[] args)
    {
        if (args.Length == 0)
        {
            Console.Error.WriteLine("Usage: <exe> <hook-name> [args...]");
            Console.Error.WriteLine("  hook-name: read-object | post-index-changed | virtual-filesystem | hooks-loader");
            return 1;
        }

        string hookName = args[0].ToLowerInvariant();
        string[] remainingArgs = args.Length > 1 ? args[1..] : Array.Empty<string>();

        return hookName switch
        {
            "read-object" => ReadObjectHook.Run(remainingArgs),
            "post-index-changed" => PostIndexChangedHook.Run(remainingArgs),
            "virtual-filesystem" => VirtualFileSystemHook.Run(remainingArgs),
            "hooks-loader" => GitHooksLoader.Run(remainingArgs),
            _ => Die($"Unknown hook: {hookName}"),
        };
    }

    internal static int Die(string message, int code = 1)
    {
        Console.Error.WriteLine(message);
        return code;
    }
}
