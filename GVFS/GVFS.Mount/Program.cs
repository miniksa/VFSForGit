using GVFS.Common;
using GVFS.PlatformLoader;
using System;
using System.CommandLine;
using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("GVFS.CommandLine.Tests")]

namespace GVFS.Mount
{
    public class Program
    {
        public static int Main(string[] args)
        {
            GVFSPlatformLoader.Initialize();

            var rootCommand = BuildRootCommand();
            return rootCommand.Parse(args).Invoke();
        }

        internal static RootCommand BuildRootCommand()
        {
            var enlistmentRootArg = new Argument<string>("enlistment-root-path") { Description = "Full or relative path to the GVFS enlistment root" };
            var verbosityOpt = new Option<string>("--verbosity") { Description = "Sets the verbosity of console logging. Accepts: Verbose, Informational, Warning, Error", DefaultValueFactory = _ => GVFSConstants.VerbParameters.Mount.DefaultVerbosity };
            var keywordsOpt = new Option<string>("--keywords") { Description = "A CSV list of logging filter keywords. Accepts: Any, Network", DefaultValueFactory = _ => GVFSConstants.VerbParameters.Mount.DefaultKeywords };
            var debugWindowOpt = new Option<bool>("--debug-window") { Description = "Show the debug window. By default, all output is written to a log file and no debug window is shown." };
            var startedByServiceOpt = new Option<string>("--StartedByService") { Description = "Service initiated mount.", DefaultValueFactory = _ => "false" };
            var startedByVerbOpt = new Option<bool>("--StartedByVerb") { Description = "Verb initiated mount." };

            var rootCommand = new RootCommand("Starts the background mount process");
            rootCommand.Arguments.Add(enlistmentRootArg);
            rootCommand.Options.Add(verbosityOpt);
            rootCommand.Options.Add(keywordsOpt);
            rootCommand.Options.Add(debugWindowOpt);
            rootCommand.Options.Add(startedByServiceOpt);
            rootCommand.Options.Add(startedByVerbOpt);

            rootCommand.SetAction((parseResult) =>
            {
                try
                {
                    var mount = new InProcessMountVerb();
                    mount.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                    mount.Verbosity = parseResult.GetValue(verbosityOpt);
                    mount.KeywordsCsv = parseResult.GetValue(keywordsOpt);
                    mount.ShowDebugWindow = parseResult.GetValue(debugWindowOpt);
                    mount.StartedByService = parseResult.GetValue(startedByServiceOpt);
                    mount.StartedByVerb = parseResult.GetValue(startedByVerbOpt);
                    mount.Execute();
                }
                catch (MountAbortedException e)
                {
                    // Calling Environment.Exit() is required, to force all background threads to exit as well
                    Environment.Exit((int)e.Verb.ReturnCode);
                }
            });

            return rootCommand;
        }
    }
}
