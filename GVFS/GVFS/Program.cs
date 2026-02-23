using GVFS.CommandLine;
using GVFS.Common;
using GVFS.PlatformLoader;
using System;
using System.CommandLine;
using System.CommandLine.Parsing;

namespace GVFS
{
    public class Program
    {
        public static void Main(string[] args)
        {
            GVFSPlatformLoader.Initialize();
            if (!GVFSPlatform.Instance.KernelDriver.RegisterForOfflineIO())
            {
                Console.WriteLine("Unable to register with the kernel for offline I/O. Ensure that VFS for Git installed successfully and try again");
                Environment.Exit((int)ReturnCode.UnableToRegisterForOfflineIO);
            }

            try
            {
                var rootCommand = BuildRootCommand();
                int exitCode = rootCommand.Parse(args).Invoke();
                Environment.Exit(exitCode);
            }
            catch (GVFSVerb.VerbAbortedException e)
            {
                // Calling Environment.Exit() is required, to force all background threads to exit as well
                Environment.Exit((int)e.Verb.ReturnCode);
            }
            finally
            {
                if (!GVFSPlatform.Instance.KernelDriver.UnregisterForOfflineIO())
                {
                    Console.WriteLine("Unable to unregister with the kernel for offline I/O.");
                }
            }
        }

        private static Option<string> CreateInternalUseOnlyOption()
        {
            var opt = new Option<string>("--internal_use_only") { Description = "This parameter is reserved for internal use." };
            opt.Hidden = true;
            return opt;
        }

        private static void ApplyCommonOptions(GVFSVerb verb, string internalParams)
        {
            if (!string.IsNullOrEmpty(internalParams))
            {
                verb.InternalParameters = internalParams;
            }
        }

        /// <summary>
        /// Execute a verb, catching VerbAbortedException and converting to exit code.
        /// GVFS verbs use VerbAbortedException for flow control (exit with specific code).
        /// </summary>
        private static void ExecuteVerb(GVFSVerb verb)
        {
            try
            {
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            }
            catch (GVFSVerb.VerbAbortedException e)
            {
                Environment.Exit((int)e.Verb.ReturnCode);
            }
        }

        private static Argument<string> CreateEnlistmentRootArgument(bool required = false)
        {
            var arg = new Argument<string>("enlistment-root-path");
            arg.Description = "Full or relative path to the GVFS enlistment root";
            if (!required)
            {
                // Make optional: zero or one values accepted. When omitted,
                // ApplyEnlistmentRootDefault fills in the current directory.
                arg.Arity = ArgumentArity.ZeroOrOne;
            }

            return arg;
        }

        /// <summary>
        /// For verbs that are NOT CloneVerb and NOT ForNoEnlistment, default
        /// EnlistmentRootPathParameter to the current directory if not specified.
        /// </summary>
        private static void ApplyEnlistmentRootDefault(GVFSVerb verb)
        {
            if (string.IsNullOrEmpty(verb.EnlistmentRootPathParameter))
            {
                verb.EnlistmentRootPathParameter = Environment.CurrentDirectory;
            }
        }

        private static RootCommand BuildRootCommand()
        {
            var rootCommand = new RootCommand("GVFS: Virtual File System for Git");

            rootCommand.Subcommands.Add(BuildCacheServerCommand());
            rootCommand.Subcommands.Add(BuildCloneCommand());
            rootCommand.Subcommands.Add(BuildConfigCommand());
            rootCommand.Subcommands.Add(BuildDehydrateCommand());
            rootCommand.Subcommands.Add(BuildDiagnoseCommand());
            rootCommand.Subcommands.Add(BuildHealthCommand());
            rootCommand.Subcommands.Add(BuildLogCommand());
            rootCommand.Subcommands.Add(BuildMountCommand());
            rootCommand.Subcommands.Add(BuildPrefetchCommand());
            rootCommand.Subcommands.Add(BuildRepairCommand());
            rootCommand.Subcommands.Add(BuildServiceCommand());
            rootCommand.Subcommands.Add(BuildSparseCommand());
            rootCommand.Subcommands.Add(BuildStatusCommand());
            rootCommand.Subcommands.Add(BuildUnmountCommand());
            rootCommand.Subcommands.Add(BuildUpgradeCommand());

            // Backward-compatible 'version' subcommand (old CLI used 'gvfs version')
            var versionCmd = new Command("version", "Display version information");
            versionCmd.SetAction((ParseResult _) =>
            {
                Console.WriteLine("GVFS " + ProcessHelper.GetCurrentProcessVersion());
            });
            rootCommand.Subcommands.Add(versionCmd);

            return rootCommand;
        }

        #region Clone

        private static Command BuildCloneCommand()
        {
            var repoUrlArg = new Argument<string>("repository-url");
            repoUrlArg.Description = "The url of the repo";

            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var cacheServerUrlOpt = new Option<string>("--cache-server-url") { Description = "The url or friendly name of the cache server" };

            var branchOpt = new Option<string>("--branch") { Description = "Branch to checkout after clone" };
            branchOpt.Aliases.Add("-b");

            var singleBranchOpt = new Option<bool>("--single-branch") { Description = "Use this option to only download metadata for the branch that will be checked out" };
            // Default: false (set via constructor)

            var noMountOpt = new Option<bool>("--no-mount") { Description = "Use this option to only clone, but not mount the repo" };
            // Default: false (set via constructor)

            var noPrefetchOpt = new Option<bool>("--no-prefetch") { Description = "Use this option to not prefetch commits after clone" };
            // Default: false (set via constructor)

            var localCachePathOpt = new Option<string>("--local-cache-path") { Description = "Use this option to override the path for the local GVFS cache." };
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("clone", "Clone a git repo and mount it as a GVFS virtual repo");
            cmd.Arguments.Add(repoUrlArg);
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(cacheServerUrlOpt);
            cmd.Options.Add(branchOpt);
            cmd.Options.Add(singleBranchOpt);
            cmd.Options.Add(noMountOpt);
            cmd.Options.Add(noPrefetchOpt);
            cmd.Options.Add(localCachePathOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new CloneVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.RepositoryURL = parseResult.GetValue(repoUrlArg);
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.CacheServerUrl = parseResult.GetValue(cacheServerUrlOpt);
                verb.Branch = parseResult.GetValue(branchOpt);
                verb.SingleBranch = parseResult.GetValue(singleBranchOpt);
                verb.NoMount = parseResult.GetValue(noMountOpt);
                verb.NoPrefetch = parseResult.GetValue(noPrefetchOpt);
                verb.LocalCacheRoot = parseResult.GetValue(localCachePathOpt);
                // Clone gets special handling: do NOT default EnlistmentRootPathParameter to cwd
                ExecuteVerb(verb);
            });

            return cmd;
        }

        #endregion

        #region ForExistingEnlistment verbs

        private static Command BuildCacheServerCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var setOpt = new Option<string>("--set") { Description = "Sets the cache server to the supplied name or url" };

            var getOpt = new Option<bool>("--get") { Description = "Outputs the current cache server information. This is the default." };
            // Default: false (set via constructor)

            var listOpt = new Option<bool>("--list") { Description = "List available cache servers for the remote repo" };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("cache-server", "Manages the cache server configuration for an existing repo.");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(setOpt);
            cmd.Options.Add(getOpt);
            cmd.Options.Add(listOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new CacheServerVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.CacheToSet = parseResult.GetValue(setOpt);
                verb.OutputCurrentInfo = parseResult.GetValue(getOpt);
                verb.ListCacheServers = parseResult.GetValue(listOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildDehydrateCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var confirmOpt = new Option<bool>("--confirm") { Description = "Pass in this flag to actually do the dehydrate" };
            // Default: false (set via constructor)

            var noStatusOpt = new Option<bool>("--no-status") { Description = "Do not require a clean git status when dehydrating." };
            // Default: false (set via constructor)

            var foldersOpt = new Option<string>("--folders") { Description = "A semicolon-delimited list of folders to dehydrate." };
            // Default: "" (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("dehydrate", "EXPERIMENTAL FEATURE - Fully dehydrate a GVFS repo");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(confirmOpt);
            cmd.Options.Add(noStatusOpt);
            cmd.Options.Add(foldersOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new DehydrateVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.Confirmed = parseResult.GetValue(confirmOpt);
                verb.NoStatus = parseResult.GetValue(noStatusOpt);
                verb.Folders = parseResult.GetValue(foldersOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildDiagnoseCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("diagnose", "Diagnose issues with a GVFS repo");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new DiagnoseVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildHealthCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var displayCountOpt = new Option<int>("-n") { Description = "Only display the <n> most hydrated directories in the output" };
            // Default: 5 (set via constructor)

            var directoryOpt = new Option<string>("--directory") { Description = "Get the health of a specific directory" };
            directoryOpt.Aliases.Add("-d");

            var statusOpt = new Option<bool>("--status") { Description = "Display only the hydration % of the repository" };
            statusOpt.Aliases.Add("-s");
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("health", "EXPERIMENTAL FEATURE - Measure the health of the repository");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(displayCountOpt);
            cmd.Options.Add(directoryOpt);
            cmd.Options.Add(statusOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new HealthVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.DirectoryDisplayCount = parseResult.GetValue(displayCountOpt);
                verb.Directory = parseResult.GetValue(directoryOpt);
                verb.StatusOnly = parseResult.GetValue(statusOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildMountCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var verbosityOpt = new Option<string>("--verbosity") { Description = "Sets the verbosity of console logging. Accepts: Verbose, Informational, Warning, Error" };
            verbosityOpt.Aliases.Add("-v");
            // Default: GVFSConstants.VerbParameters.Mount.DefaultVerbosity (set via constructor)

            var keywordsOpt = new Option<string>("--keywords") { Description = "A CSV list of logging filter keywords. Accepts: Any, Network" };
            keywordsOpt.Aliases.Add("-k");
            // Default: GVFSConstants.VerbParameters.Mount.DefaultKeywords (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("mount", "Mount a GVFS virtual repo");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(verbosityOpt);
            cmd.Options.Add(keywordsOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new MountVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.Verbosity = parseResult.GetValue(verbosityOpt);
                verb.KeywordsCsv = parseResult.GetValue(keywordsOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildPrefetchCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var filesOpt = new Option<string>("--files") { Description = "A semicolon-delimited list of files to fetch." };
            // Default: "" (set via constructor)

            var foldersOpt = new Option<string>("--folders") { Description = "A semicolon-delimited list of folders to fetch." };
            // Default: "" (set via constructor)

            var foldersListOpt = new Option<string>("--folders-list") { Description = "A file containing line-delimited list of folders to fetch." };
            // Default: "" (set via constructor)

            var stdinFilesOpt = new Option<bool>("--stdin-files-list") { Description = "Load file list from stdin." };
            // Default: false (set via constructor)

            var stdinFoldersOpt = new Option<bool>("--stdin-folders-list") { Description = "Load folder list from stdin." };
            // Default: false (set via constructor)

            var filesListOpt = new Option<string>("--files-list") { Description = "A file containing line-delimited list of files to fetch." };
            // Default: "" (set via constructor)

            var hydrateOpt = new Option<bool>("--hydrate") { Description = "Also hydrate files in the working directory." };
            // Default: false (set via constructor)

            var commitsOpt = new Option<bool>("--commits") { Description = "Fetch the latest set of commit and tree packs." };
            commitsOpt.Aliases.Add("-c");
            // Default: false (set via constructor)

            var verboseOpt = new Option<bool>("--verbose") { Description = "Show all outputs on the console." };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("prefetch", "Prefetch remote objects for the current head");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(filesOpt);
            cmd.Options.Add(foldersOpt);
            cmd.Options.Add(foldersListOpt);
            cmd.Options.Add(stdinFilesOpt);
            cmd.Options.Add(stdinFoldersOpt);
            cmd.Options.Add(filesListOpt);
            cmd.Options.Add(hydrateOpt);
            cmd.Options.Add(commitsOpt);
            cmd.Options.Add(verboseOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new PrefetchVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.Files = parseResult.GetValue(filesOpt);
                verb.Folders = parseResult.GetValue(foldersOpt);
                verb.FoldersListFile = parseResult.GetValue(foldersListOpt);
                verb.FilesFromStdIn = parseResult.GetValue(stdinFilesOpt);
                verb.FoldersFromStdIn = parseResult.GetValue(stdinFoldersOpt);
                verb.FilesListFile = parseResult.GetValue(filesListOpt);
                verb.HydrateFiles = parseResult.GetValue(hydrateOpt);
                verb.Commits = parseResult.GetValue(commitsOpt);
                verb.Verbose = parseResult.GetValue(verboseOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildSparseCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var setOpt = new Option<string>("--set") { Description = "A semicolon-delimited list of repo root relative folders to use as the sparse set." };
            setOpt.Aliases.Add("-s");
            // Default: "" (set via constructor)

            var fileOpt = new Option<string>("--file") { Description = "Path to a file with repo root relative folders to use as the sparse set." };
            fileOpt.Aliases.Add("-f");
            // Default: "" (set via constructor)

            var addOpt = new Option<string>("--add") { Description = "A semicolon-delimited list of repo root relative folders to include in the sparse set." };
            addOpt.Aliases.Add("-a");
            // Default: "" (set via constructor)

            var removeOpt = new Option<string>("--remove") { Description = "A semicolon-delimited list of repo root relative folders to remove from the sparse set." };
            removeOpt.Aliases.Add("-r");
            // Default: "" (set via constructor)

            var listOpt = new Option<bool>("--list") { Description = "List of folders in the sparse set." };
            listOpt.Aliases.Add("-l");
            // Default: false (set via constructor)

            var pruneOpt = new Option<bool>("--prune") { Description = "Remove any folders that are not in the list of sparse folders." };
            pruneOpt.Aliases.Add("-p");
            // Default: false (set via constructor)

            var disableOpt = new Option<bool>("--disable") { Description = "Disable the sparse feature." };
            disableOpt.Aliases.Add("-d");
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("sparse", "EXPERIMENTAL: List, add, or remove from the list of folders included in VFS for Git's projection.");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(setOpt);
            cmd.Options.Add(fileOpt);
            cmd.Options.Add(addOpt);
            cmd.Options.Add(removeOpt);
            cmd.Options.Add(listOpt);
            cmd.Options.Add(pruneOpt);
            cmd.Options.Add(disableOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new SparseVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.Set = parseResult.GetValue(setOpt);
                verb.File = parseResult.GetValue(fileOpt);
                verb.Add = parseResult.GetValue(addOpt);
                verb.Remove = parseResult.GetValue(removeOpt);
                verb.List = parseResult.GetValue(listOpt);
                verb.Prune = parseResult.GetValue(pruneOpt);
                verb.Disable = parseResult.GetValue(disableOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildStatusCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("status", "Get the status of the GVFS virtual repo");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new StatusVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        #endregion

        #region Direct GVFSVerb subclasses (with enlistment root)

        private static Command BuildLogCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var typeOpt = new Option<string>("--type") { Description = "The type of log file to display on the console" };
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("log", "Show the most recent GVFS log files");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(typeOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new LogVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.LogType = parseResult.GetValue(typeOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildRepairCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var confirmOpt = new Option<bool>("--confirm") { Description = "Pass in this flag to actually do repair(s)." };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("repair", "EXPERIMENTAL FEATURE - Repair issues that prevent a GVFS repo from mounting");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(confirmOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new RepairVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.Confirmed = parseResult.GetValue(confirmOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildUnmountCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();

            var skipLockOpt = new Option<bool>("--skip-wait-for-lock") { Description = "Force unmount even if the lock is not available." };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("unmount", "Unmount a GVFS virtual repo");
            cmd.Arguments.Add(enlistmentRootArg);
            cmd.Options.Add(skipLockOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new UnmountVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.EnlistmentRootPathParameter = parseResult.GetValue(enlistmentRootArg);
                verb.SkipLock = parseResult.GetValue(skipLockOpt);
                ApplyEnlistmentRootDefault(verb);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        #endregion

        #region ForNoEnlistment verbs

        private static Command BuildConfigCommand()
        {
            var keyArg = new Argument<string>("key");
            keyArg.Description = "Name of setting that is to be set or read";
            // Default: "" (set via constructor)

            var valueArg = new Argument<string>("value");
            valueArg.Description = "Value of setting to be set";
            // Default: "" (set via constructor)

            var listOpt = new Option<bool>("--list") { Description = "Show all settings" };
            listOpt.Aliases.Add("-l");
            // Default: false (set via constructor)

            var deleteOpt = new Option<string>("--delete") { Description = "Name of setting to delete" };
            deleteOpt.Aliases.Add("-d");

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("config", "Get and set GVFS options.");
            cmd.Arguments.Add(keyArg);
            cmd.Arguments.Add(valueArg);
            cmd.Options.Add(listOpt);
            cmd.Options.Add(deleteOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new ConfigVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.Key = parseResult.GetValue(keyArg);
                verb.Value = parseResult.GetValue(valueArg);
                verb.List = parseResult.GetValue(listOpt);
                verb.KeyToDelete = parseResult.GetValue(deleteOpt);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildServiceCommand()
        {
            var mountAllOpt = new Option<bool>("--mount-all") { Description = "Mounts all repos" };
            // Default: false (set via constructor)

            var unmountAllOpt = new Option<bool>("--unmount-all") { Description = "Unmounts all repos" };
            // Default: false (set via constructor)

            var listMountedOpt = new Option<bool>("--list-mounted") { Description = "Prints a list of all mounted repos" };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("service", "Runs commands for the GVFS service.");
            cmd.Options.Add(mountAllOpt);
            cmd.Options.Add(unmountAllOpt);
            cmd.Options.Add(listMountedOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new ServiceVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.MountAll = parseResult.GetValue(mountAllOpt);
                verb.UnmountAll = parseResult.GetValue(unmountAllOpt);
                verb.List = parseResult.GetValue(listMountedOpt);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        private static Command BuildUpgradeCommand()
        {
            var confirmOpt = new Option<bool>("--confirm") { Description = "Pass in this flag to actually install the newest release" };
            // Default: false (set via constructor)

            var dryRunOpt = new Option<bool>("--dry-run") { Description = "Display progress and errors, but don't install GVFS" };
            // Default: false (set via constructor)

            var noVerifyOpt = new Option<bool>("--no-verify") { Description = "Do not verify NuGet packages after downloading them." };
            // Default: false (set via constructor)

            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("upgrade", "Checks for new GVFS release, downloads and installs it when available.");
            cmd.Options.Add(confirmOpt);
            cmd.Options.Add(dryRunOpt);
            cmd.Options.Add(noVerifyOpt);
            cmd.Options.Add(internalOpt);

            cmd.SetAction((ParseResult parseResult) =>
            {
                var verb = new UpgradeVerb();
                ApplyCommonOptions(verb, parseResult.GetValue(internalOpt));
                verb.Confirmed = parseResult.GetValue(confirmOpt);
                verb.DryRun = parseResult.GetValue(dryRunOpt);
                verb.NoVerify = parseResult.GetValue(noVerifyOpt);
                ExecuteVerb(verb);
            });

            return cmd;
        }

        #endregion
    }
}

