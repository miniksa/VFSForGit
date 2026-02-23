// GitHooksLoader — reads a .hooks sidecar file and runs each listed exe.
// Replaces GitHooksLoader.exe (C++).
//
// Usage: GitHooksLoader.exe <git-verb> [args...]
// Reads <exename>.hooks file for the list of hook executables to run.

using System;
using System.Diagnostics;
using System.IO;

namespace GVFS.NativeHooks;

internal static class GitHooksLoader
{
    internal static int Run(string[] args)
    {
        if (args.Length < 1)
        {
            Console.Error.WriteLine($"Usage: GitHooksLoader <git verb> [<other arguments>]");
            return 1;
        }

        string exePath = Environment.ProcessPath ?? "unknown";
        string hookName = Path.GetFileNameWithoutExtension(exePath);

        // The .hooks file is next to the exe, same name with .hooks extension
        string basePath = Path.ChangeExtension(exePath, null); // strip .exe
        string hooksFile = basePath + ".hooks";

        if (!File.Exists(hooksFile))
        {
            Console.Error.WriteLine($"No hooks file found: {hooksFile}");
            return 5;
        }

        bool perfTrace = Environment.GetEnvironmentVariable("GITHOOKSLOADER_PERFTRACE") != null;
        int numHooksExecuted = 0;

        foreach (string line in File.ReadLines(hooksFile))
        {
            string hookApp = line.Trim();
            if (string.IsNullOrEmpty(hookApp) || hookApp[0] == '#')
                continue;

            numHooksExecuted++;

            // Expand environment variables in the path
            hookApp = Environment.ExpandEnvironmentVariables(hookApp);

            var sw = perfTrace ? Stopwatch.StartNew() : null;

            int exitCode = ExecuteHook(hookApp, hookName, args);
            if (exitCode != 0)
                return exitCode;

            if (sw != null)
            {
                sw.Stop();
                Console.WriteLine($"{basePath}: {hookApp} = {sw.Elapsed.TotalMilliseconds:F2} milliseconds");
            }
        }

        if (numHooksExecuted == 0)
        {
            Console.Error.WriteLine("No hooks found to execute");
            return 5;
        }

        return 0;
    }

    private static int ExecuteHook(string applicationName, string hookName, string[] args)
    {
        // Build command line: <app> <hookName> <arg1> <arg2> ...
        var cmdLineBuilder = new System.Text.StringBuilder();
        cmdLineBuilder.Append('"').Append(applicationName).Append("\" ");
        cmdLineBuilder.Append(hookName);
        foreach (string arg in args)
        {
            cmdLineBuilder.Append(' ').Append(arg);
        }

        try
        {
            using var process = new Process();
            process.StartInfo.FileName = applicationName;
            process.StartInfo.Arguments = $"{hookName} {string.Join(' ', args)}";
            process.StartInfo.UseShellExecute = false;
            process.StartInfo.CreateNoWindow = true;

            // Inherit stdio
            process.StartInfo.RedirectStandardInput = false;
            process.StartInfo.RedirectStandardOutput = false;
            process.StartInfo.RedirectStandardError = false;

            process.Start();
            process.WaitForExit();
            return process.ExitCode;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"Could not execute '{applicationName}'. Error: {ex.Message}");
            return 3;
        }
    }
}
