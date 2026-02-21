using GVFS.Common;
using GVFS.PlatformLoader;
using System;
using System.CommandLine;
using System.CommandLine.Invocation;

namespace GVFS.Mount
{
    public class Program
    {
        public static int Main(string[] args)
        {
            GVFSPlatformLoader.Initialize();

            var enlistmentRootArg = new Argument<string>("enlistment-root-path", "Full or relative path to the GVFS enlistment root");
            var verbosityOpt = new Option<string>(new[] { "-v", "--verbosity" },
                () => GVFSConstants.VerbParameters.Mount.DefaultVerbosity,
                "Sets the verbosity of console logging. Accepts: Verbose, Informational, Warning, Error");
            var keywordsOpt = new Option<string>(new[] { "-k", "--keywords" },
                () => GVFSConstants.VerbParameters.Mount.DefaultKeywords,
                "A CSV list of logging filter keywords. Accepts: Any, Network");
            var debugWindowOpt = new Option<bool>(new[] { "-d", "--debug-window" }, () => false,
                "Show the debug window. By default, all output is written to a log file and no debug window is shown.");
            var startedByServiceOpt = new Option<string>(new[] { "-s", "--StartedByService" }, () => "false",
                "Service initiated mount.");
            var startedByVerbOpt = new Option<bool>(new[] { "-b", "--StartedByVerb" }, () => false,
                "Verb initiated mount.");

            var rootCommand = new RootCommand("Starts the background mount process");
            rootCommand.AddArgument(enlistmentRootArg);
            rootCommand.AddOption(verbosityOpt);
            rootCommand.AddOption(keywordsOpt);
            rootCommand.AddOption(debugWindowOpt);
            rootCommand.AddOption(startedByServiceOpt);
            rootCommand.AddOption(startedByVerbOpt);

            rootCommand.SetHandler((InvocationContext context) =>
            {
                try
                {
                    var mount = new InProcessMountVerb();
                    mount.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                    mount.Verbosity = context.ParseResult.GetValueForOption(verbosityOpt);
                    mount.KeywordsCsv = context.ParseResult.GetValueForOption(keywordsOpt);
                    mount.ShowDebugWindow = context.ParseResult.GetValueForOption(debugWindowOpt);
                    mount.StartedByService = context.ParseResult.GetValueForOption(startedByServiceOpt);
                    mount.StartedByVerb = context.ParseResult.GetValueForOption(startedByVerbOpt);
                    mount.Execute();
                }
                catch (MountAbortedException e)
                {
                    // Calling Environment.Exit() is required, to force all background threads to exit as well
                    Environment.Exit((int)e.Verb.ReturnCode);
                }
            });

            return rootCommand.Invoke(args);
        }
    }
}
