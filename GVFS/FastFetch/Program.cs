using System.CommandLine;
using GVFS.PlatformLoader;
using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("GVFS.CommandLine.Tests")]

namespace FastFetch
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
            var commitOpt = new Option<string>("--commit") { Description = "Commit to fetch" };
            commitOpt.Aliases.Add("-c");
            var branchOpt = new Option<string>("--branch") { Description = "Branch to fetch" };
            branchOpt.Aliases.Add("-b");
            var cacheServerUrlOpt = new Option<string>("--cache-server-url") { Description = "Defines the url of the cache server" };
            var chunkSizeOpt = new Option<int>("--chunk-size") { Description = "Sets the number of objects to be downloaded in a single pack", DefaultValueFactory = _ => 4000 };
            var checkoutOpt = new Option<bool>("--checkout") { Description = "Checkout the target commit into the working directory after fetching" };
            var forceCheckoutOpt = new Option<bool>("--force-checkout") { Description = "Force FastFetch to checkout content as if the current repo had just been initialized." };
            var searchThreadCountOpt = new Option<int>("--search-thread-count") { Description = "Sets the number of threads to use for finding missing blobs. (0 for number of logical cores)" };
            var downloadThreadCountOpt = new Option<int>("--download-thread-count") { Description = "Sets the number of threads to use for downloading. (0 for number of logical cores)" };
            var indexThreadCountOpt = new Option<int>("--index-thread-count") { Description = "Sets the number of threads to use for indexing. (0 for number of logical cores)" };
            var checkoutThreadCountOpt = new Option<int>("--checkout-thread-count") { Description = "Sets the number of threads to use for checkout. (0 for number of logical cores)" };
            var maxRetriesOpt = new Option<int>("--max-retries") { Description = "Sets the maximum number of attempts for downloading a pack", DefaultValueFactory = _ => 10 };
            maxRetriesOpt.Aliases.Add("-r");
            var gitPathOpt = new Option<string>("--git-path") { Description = "Sets the path and filename for git.exe if it isn't expected to be on %PATH%." };
            var foldersOpt = new Option<string>("--folders") { Description = "A semicolon-delimited list of folders to fetch", DefaultValueFactory = _ => "" };
            var foldersListOpt = new Option<string>("--folders-list") { Description = "A file containing line-delimited list of folders to fetch", DefaultValueFactory = _ => "" };
            var allowIndexMetadataOpt = new Option<bool>("--allow-index-metadata-update-from-working-tree") { Description =
                "When specified, index metadata (file times and sizes) is updated from disk if not already in the index." };
            var verboseOpt = new Option<bool>("--verbose") { Description = "Show all outputs on the console in addition to writing them to a log file" };
            var parentActivityIdOpt = new Option<string>("--parent-activity-id") { Description = "The GUID of the caller - used for telemetry purposes." };

            var rootCommand = new RootCommand("Fast-fetch a branch");
            rootCommand.Options.Add(commitOpt);
            rootCommand.Options.Add(branchOpt);
            rootCommand.Options.Add(cacheServerUrlOpt);
            rootCommand.Options.Add(chunkSizeOpt);
            rootCommand.Options.Add(checkoutOpt);
            rootCommand.Options.Add(forceCheckoutOpt);
            rootCommand.Options.Add(searchThreadCountOpt);
            rootCommand.Options.Add(downloadThreadCountOpt);
            rootCommand.Options.Add(indexThreadCountOpt);
            rootCommand.Options.Add(checkoutThreadCountOpt);
            rootCommand.Options.Add(maxRetriesOpt);
            rootCommand.Options.Add(gitPathOpt);
            rootCommand.Options.Add(foldersOpt);
            rootCommand.Options.Add(foldersListOpt);
            rootCommand.Options.Add(allowIndexMetadataOpt);
            rootCommand.Options.Add(verboseOpt);
            rootCommand.Options.Add(parentActivityIdOpt);

            rootCommand.SetAction((parseResult) =>
            {
                var verb = new FastFetchVerb();
                verb.Commit = parseResult.GetValue(commitOpt);
                verb.Branch = parseResult.GetValue(branchOpt);
                verb.CacheServerUrl = parseResult.GetValue(cacheServerUrlOpt);
                verb.ChunkSize = parseResult.GetValue(chunkSizeOpt);
                verb.Checkout = parseResult.GetValue(checkoutOpt);
                verb.ForceCheckout = parseResult.GetValue(forceCheckoutOpt);
                verb.SearchThreadCount = parseResult.GetValue(searchThreadCountOpt);
                verb.DownloadThreadCount = parseResult.GetValue(downloadThreadCountOpt);
                verb.IndexThreadCount = parseResult.GetValue(indexThreadCountOpt);
                verb.CheckoutThreadCount = parseResult.GetValue(checkoutThreadCountOpt);
                verb.MaxAttempts = parseResult.GetValue(maxRetriesOpt);
                verb.GitBinPath = parseResult.GetValue(gitPathOpt);
                verb.FolderList = parseResult.GetValue(foldersOpt);
                verb.FolderListFile = parseResult.GetValue(foldersListOpt);
                verb.AllowIndexMetadataUpdateFromWorkingTree = parseResult.GetValue(allowIndexMetadataOpt);
                verb.Verbose = parseResult.GetValue(verboseOpt);
                verb.ParentActivityId = parseResult.GetValue(parentActivityIdOpt);
                verb.Execute();
            });

            return rootCommand;
        }
    }
}
