using GVFS.CommandLine;
using GVFS.Common;
using GVFS.PlatformLoader;
using System;
using System.CommandLine;
using System.CommandLine.Invocation;

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
                int exitCode = rootCommand.Invoke(args);
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
            var opt = new Option<string>("--internal_use_only", "This parameter is reserved for internal use.");
            opt.IsHidden = true;
            return opt;
        }

        private static void ApplyCommonOptions(GVFSVerb verb, InvocationContext context, Option<string> internalOpt)
        {
            string internalParams = context.ParseResult.GetValueForOption(internalOpt);
            if (!string.IsNullOrEmpty(internalParams))
            {
                verb.InternalParameters = internalParams;
            }
        }

        private static Argument<string> CreateEnlistmentRootArgument(bool required = false)
        {
            if (required)
            {
                return new Argument<string>("enlistment-root-path", "Full or relative path to the GVFS enlistment root");
            }

            return new Argument<string>("enlistment-root-path", () => "", "Full or relative path to the GVFS enlistment root");
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

            rootCommand.AddCommand(BuildCacheServerCommand());
            rootCommand.AddCommand(BuildCloneCommand());
            rootCommand.AddCommand(BuildConfigCommand());
            rootCommand.AddCommand(BuildDehydrateCommand());
            rootCommand.AddCommand(BuildDiagnoseCommand());
            rootCommand.AddCommand(BuildHealthCommand());
            rootCommand.AddCommand(BuildLogCommand());
            rootCommand.AddCommand(BuildMountCommand());
            rootCommand.AddCommand(BuildPrefetchCommand());
            rootCommand.AddCommand(BuildRepairCommand());
            rootCommand.AddCommand(BuildServiceCommand());
            rootCommand.AddCommand(BuildSparseCommand());
            rootCommand.AddCommand(BuildStatusCommand());
            rootCommand.AddCommand(BuildUnmountCommand());
            rootCommand.AddCommand(BuildUpgradeCommand());

            return rootCommand;
        }

        #region Clone

        private static Command BuildCloneCommand()
        {
            var repoUrlArg = new Argument<string>("repository-url", "The url of the repo");
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var cacheServerUrlOpt = new Option<string>("--cache-server-url", "The url or friendly name of the cache server");
            var branchOpt = new Option<string>(new[] { "-b", "--branch" }, "Branch to checkout after clone");
            var singleBranchOpt = new Option<bool>("--single-branch", () => false, "Use this option to only download metadata for the branch that will be checked out");
            var noMountOpt = new Option<bool>("--no-mount", () => false, "Use this option to only clone, but not mount the repo");
            var noPrefetchOpt = new Option<bool>("--no-prefetch", () => false, "Use this option to not prefetch commits after clone");
            var localCachePathOpt = new Option<string>("--local-cache-path", "Use this option to override the path for the local GVFS cache.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("clone", "Clone a git repo and mount it as a GVFS virtual repo");
            cmd.AddArgument(repoUrlArg);
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(cacheServerUrlOpt);
            cmd.AddOption(branchOpt);
            cmd.AddOption(singleBranchOpt);
            cmd.AddOption(noMountOpt);
            cmd.AddOption(noPrefetchOpt);
            cmd.AddOption(localCachePathOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new CloneVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.RepositoryURL = context.ParseResult.GetValueForArgument(repoUrlArg);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.CacheServerUrl = context.ParseResult.GetValueForOption(cacheServerUrlOpt);
                verb.Branch = context.ParseResult.GetValueForOption(branchOpt);
                verb.SingleBranch = context.ParseResult.GetValueForOption(singleBranchOpt);
                verb.NoMount = context.ParseResult.GetValueForOption(noMountOpt);
                verb.NoPrefetch = context.ParseResult.GetValueForOption(noPrefetchOpt);
                verb.LocalCacheRoot = context.ParseResult.GetValueForOption(localCachePathOpt);
                // Clone gets special handling: do NOT default EnlistmentRootPathParameter to cwd
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        #endregion

        #region ForExistingEnlistment verbs

        private static Command BuildCacheServerCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var setOpt = new Option<string>("--set", "Sets the cache server to the supplied name or url");
            var getOpt = new Option<bool>("--get", () => false, "Outputs the current cache server information. This is the default.");
            var listOpt = new Option<bool>("--list", () => false, "List available cache servers for the remote repo");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("cache-server", "Manages the cache server configuration for an existing repo.");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(setOpt);
            cmd.AddOption(getOpt);
            cmd.AddOption(listOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new CacheServerVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.CacheToSet = context.ParseResult.GetValueForOption(setOpt);
                verb.OutputCurrentInfo = context.ParseResult.GetValueForOption(getOpt);
                verb.ListCacheServers = context.ParseResult.GetValueForOption(listOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildDehydrateCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var confirmOpt = new Option<bool>("--confirm", () => false, "Pass in this flag to actually do the dehydrate");
            var noStatusOpt = new Option<bool>("--no-status", () => false, "Do not require a clean git status when dehydrating.");
            var foldersOpt = new Option<string>("--folders", () => "", "A semicolon-delimited list of folders to dehydrate.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("dehydrate", "EXPERIMENTAL FEATURE - Fully dehydrate a GVFS repo");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(confirmOpt);
            cmd.AddOption(noStatusOpt);
            cmd.AddOption(foldersOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new DehydrateVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.Confirmed = context.ParseResult.GetValueForOption(confirmOpt);
                verb.NoStatus = context.ParseResult.GetValueForOption(noStatusOpt);
                verb.Folders = context.ParseResult.GetValueForOption(foldersOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildDiagnoseCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("diagnose", "Diagnose issues with a GVFS repo");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new DiagnoseVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildHealthCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var displayCountOpt = new Option<int>("-n", () => 5, "Only display the <n> most hydrated directories in the output");
            var directoryOpt = new Option<string>(new[] { "-d", "--directory" }, "Get the health of a specific directory");
            var statusOpt = new Option<bool>(new[] { "-s", "--status" }, () => false, "Display only the hydration % of the repository");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("health", "EXPERIMENTAL FEATURE - Measure the health of the repository");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(displayCountOpt);
            cmd.AddOption(directoryOpt);
            cmd.AddOption(statusOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new HealthVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.DirectoryDisplayCount = context.ParseResult.GetValueForOption(displayCountOpt);
                verb.Directory = context.ParseResult.GetValueForOption(directoryOpt);
                verb.StatusOnly = context.ParseResult.GetValueForOption(statusOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildMountCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var verbosityOpt = new Option<string>(new[] { "-v", "--verbosity" },
                () => GVFSConstants.VerbParameters.Mount.DefaultVerbosity,
                "Sets the verbosity of console logging. Accepts: Verbose, Informational, Warning, Error");
            var keywordsOpt = new Option<string>(new[] { "-k", "--keywords" },
                () => GVFSConstants.VerbParameters.Mount.DefaultKeywords,
                "A CSV list of logging filter keywords. Accepts: Any, Network");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("mount", "Mount a GVFS virtual repo");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(verbosityOpt);
            cmd.AddOption(keywordsOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new MountVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.Verbosity = context.ParseResult.GetValueForOption(verbosityOpt);
                verb.KeywordsCsv = context.ParseResult.GetValueForOption(keywordsOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildPrefetchCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var filesOpt = new Option<string>("--files", () => "", "A semicolon-delimited list of files to fetch.");
            var foldersOpt = new Option<string>("--folders", () => "", "A semicolon-delimited list of folders to fetch.");
            var foldersListOpt = new Option<string>("--folders-list", () => "", "A file containing line-delimited list of folders to fetch.");
            var stdinFilesOpt = new Option<bool>("--stdin-files-list", () => false, "Load file list from stdin.");
            var stdinFoldersOpt = new Option<bool>("--stdin-folders-list", () => false, "Load folder list from stdin.");
            var filesListOpt = new Option<string>("--files-list", () => "", "A file containing line-delimited list of files to fetch.");
            var hydrateOpt = new Option<bool>("--hydrate", () => false, "Also hydrate files in the working directory.");
            var commitsOpt = new Option<bool>(new[] { "-c", "--commits" }, () => false, "Fetch the latest set of commit and tree packs.");
            var verboseOpt = new Option<bool>("--verbose", () => false, "Show all outputs on the console.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("prefetch", "Prefetch remote objects for the current head");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(filesOpt);
            cmd.AddOption(foldersOpt);
            cmd.AddOption(foldersListOpt);
            cmd.AddOption(stdinFilesOpt);
            cmd.AddOption(stdinFoldersOpt);
            cmd.AddOption(filesListOpt);
            cmd.AddOption(hydrateOpt);
            cmd.AddOption(commitsOpt);
            cmd.AddOption(verboseOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new PrefetchVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.Files = context.ParseResult.GetValueForOption(filesOpt);
                verb.Folders = context.ParseResult.GetValueForOption(foldersOpt);
                verb.FoldersListFile = context.ParseResult.GetValueForOption(foldersListOpt);
                verb.FilesFromStdIn = context.ParseResult.GetValueForOption(stdinFilesOpt);
                verb.FoldersFromStdIn = context.ParseResult.GetValueForOption(stdinFoldersOpt);
                verb.FilesListFile = context.ParseResult.GetValueForOption(filesListOpt);
                verb.HydrateFiles = context.ParseResult.GetValueForOption(hydrateOpt);
                verb.Commits = context.ParseResult.GetValueForOption(commitsOpt);
                verb.Verbose = context.ParseResult.GetValueForOption(verboseOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildSparseCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var setOpt = new Option<string>(new[] { "-s", "--set" }, () => "",
                "A semicolon-delimited list of repo root relative folders to use as the sparse set.");
            var fileOpt = new Option<string>(new[] { "-f", "--file" }, () => "",
                "Path to a file with repo root relative folders to use as the sparse set.");
            var addOpt = new Option<string>(new[] { "-a", "--add" }, () => "",
                "A semicolon-delimited list of repo root relative folders to include in the sparse set.");
            var removeOpt = new Option<string>(new[] { "-r", "--remove" }, () => "",
                "A semicolon-delimited list of repo root relative folders to remove from the sparse set.");
            var listOpt = new Option<bool>(new[] { "-l", "--list" }, () => false, "List of folders in the sparse set.");
            var pruneOpt = new Option<bool>(new[] { "-p", "--prune" }, () => false, "Remove any folders that are not in the list of sparse folders.");
            var disableOpt = new Option<bool>(new[] { "-d", "--disable" }, () => false, "Disable the sparse feature.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("sparse", "EXPERIMENTAL: List, add, or remove from the list of folders included in VFS for Git's projection.");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(setOpt);
            cmd.AddOption(fileOpt);
            cmd.AddOption(addOpt);
            cmd.AddOption(removeOpt);
            cmd.AddOption(listOpt);
            cmd.AddOption(pruneOpt);
            cmd.AddOption(disableOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new SparseVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.Set = context.ParseResult.GetValueForOption(setOpt);
                verb.File = context.ParseResult.GetValueForOption(fileOpt);
                verb.Add = context.ParseResult.GetValueForOption(addOpt);
                verb.Remove = context.ParseResult.GetValueForOption(removeOpt);
                verb.List = context.ParseResult.GetValueForOption(listOpt);
                verb.Prune = context.ParseResult.GetValueForOption(pruneOpt);
                verb.Disable = context.ParseResult.GetValueForOption(disableOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildStatusCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("status", "Get the status of the GVFS virtual repo");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new StatusVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        #endregion

        #region Direct GVFSVerb subclasses (with enlistment root)

        private static Command BuildLogCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var typeOpt = new Option<string>("--type", "The type of log file to display on the console");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("log", "Show the most recent GVFS log files");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(typeOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new LogVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.LogType = context.ParseResult.GetValueForOption(typeOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildRepairCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var confirmOpt = new Option<bool>("--confirm", () => false, "Pass in this flag to actually do repair(s).");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("repair", "EXPERIMENTAL FEATURE - Repair issues that prevent a GVFS repo from mounting");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(confirmOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new RepairVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.Confirmed = context.ParseResult.GetValueForOption(confirmOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildUnmountCommand()
        {
            var enlistmentRootArg = CreateEnlistmentRootArgument();
            var skipLockOpt = new Option<bool>("--skip-wait-for-lock", () => false, "Force unmount even if the lock is not available.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("unmount", "Unmount a GVFS virtual repo");
            cmd.AddArgument(enlistmentRootArg);
            cmd.AddOption(skipLockOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new UnmountVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.EnlistmentRootPathParameter = context.ParseResult.GetValueForArgument(enlistmentRootArg);
                verb.SkipLock = context.ParseResult.GetValueForOption(skipLockOpt);
                ApplyEnlistmentRootDefault(verb);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        #endregion

        #region ForNoEnlistment verbs

        private static Command BuildConfigCommand()
        {
            var keyArg = new Argument<string>("key", () => "", "Name of setting that is to be set or read");
            var valueArg = new Argument<string>("value", () => "", "Value of setting to be set");
            var listOpt = new Option<bool>(new[] { "-l", "--list" }, () => false, "Show all settings");
            var deleteOpt = new Option<string>(new[] { "-d", "--delete" }, "Name of setting to delete");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("config", "Get and set GVFS options.");
            cmd.AddArgument(keyArg);
            cmd.AddArgument(valueArg);
            cmd.AddOption(listOpt);
            cmd.AddOption(deleteOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new ConfigVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.Key = context.ParseResult.GetValueForArgument(keyArg);
                verb.Value = context.ParseResult.GetValueForArgument(valueArg);
                verb.List = context.ParseResult.GetValueForOption(listOpt);
                verb.KeyToDelete = context.ParseResult.GetValueForOption(deleteOpt);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildServiceCommand()
        {
            var mountAllOpt = new Option<bool>("--mount-all", () => false, "Mounts all repos");
            var unmountAllOpt = new Option<bool>("--unmount-all", () => false, "Unmounts all repos");
            var listMountedOpt = new Option<bool>("--list-mounted", () => false, "Prints a list of all mounted repos");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("service", "Runs commands for the GVFS service.");
            cmd.AddOption(mountAllOpt);
            cmd.AddOption(unmountAllOpt);
            cmd.AddOption(listMountedOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new ServiceVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.MountAll = context.ParseResult.GetValueForOption(mountAllOpt);
                verb.UnmountAll = context.ParseResult.GetValueForOption(unmountAllOpt);
                verb.List = context.ParseResult.GetValueForOption(listMountedOpt);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        private static Command BuildUpgradeCommand()
        {
            var confirmOpt = new Option<bool>("--confirm", () => false, "Pass in this flag to actually install the newest release");
            var dryRunOpt = new Option<bool>("--dry-run", () => false, "Display progress and errors, but don't install GVFS");
            var noVerifyOpt = new Option<bool>("--no-verify", () => false, "Do not verify NuGet packages after downloading them.");
            var internalOpt = CreateInternalUseOnlyOption();

            var cmd = new Command("upgrade", "Checks for new GVFS release, downloads and installs it when available.");
            cmd.AddOption(confirmOpt);
            cmd.AddOption(dryRunOpt);
            cmd.AddOption(noVerifyOpt);
            cmd.AddOption(internalOpt);

            cmd.SetHandler((InvocationContext context) =>
            {
                var verb = new UpgradeVerb();
                ApplyCommonOptions(verb, context, internalOpt);
                verb.Confirmed = context.ParseResult.GetValueForOption(confirmOpt);
                verb.DryRun = context.ParseResult.GetValueForOption(dryRunOpt);
                verb.NoVerify = context.ParseResult.GetValueForOption(noVerifyOpt);
                verb.Execute();
                Environment.Exit((int)ReturnCode.Success);
            });

            return cmd;
        }

        #endregion
    }
}
