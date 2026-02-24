using System.CommandLine;
using System.CommandLine.Invocation;
using GVFS.PlatformLoader;

namespace FastFetch
{
    public class Program
    {
        public static int Main(string[] args)
        {
            GVFSPlatformLoader.Initialize();

            var commitOpt = new Option<string>("-c") { Description = "Commit to fetch" };
            commitOpt.AddAlias("--commit");
            var branchOpt = new Option<string>("-b") { Description = "Branch to fetch" };
            branchOpt.AddAlias("--branch");
            var cacheServerUrlOpt = new Option<string>("--cache-server-url", "") { Description = "Defines the url of the cache server" };
            var chunkSizeOpt = new Option<int>("--chunk-size", 4000) { Description = "Sets the number of objects to be downloaded in a single pack" };
            var checkoutOpt = new Option<bool>("--checkout", false) { Description = "Checkout the target commit into the working directory after fetching" };
            var forceCheckoutOpt = new Option<bool>("--force-checkout", false) { Description = "Force FastFetch to checkout content as if the current repo had just been initialized." };
            var searchThreadCountOpt = new Option<int>("--search-thread-count", 0) { Description = "Sets the number of threads to use for finding missing blobs. (0 for number of logical cores)" };
            var downloadThreadCountOpt = new Option<int>("--download-thread-count", 0) { Description = "Sets the number of threads to use for downloading. (0 for number of logical cores)" };
            var indexThreadCountOpt = new Option<int>("--index-thread-count", 0) { Description = "Sets the number of threads to use for indexing. (0 for number of logical cores)" };
            var checkoutThreadCountOpt = new Option<int>("--checkout-thread-count", 0) { Description = "Sets the number of threads to use for checkout. (0 for number of logical cores)" };
            var maxRetriesOpt = new Option<int>("-r", 10) { Description = "Sets the maximum number of attempts for downloading a pack" };
            maxRetriesOpt.AddAlias("--max-retries");
            var gitPathOpt = new Option<string>("--git-path", "") { Description = "Sets the path and filename for git.exe if it isn't expected to be on %PATH%." };
            var foldersOpt = new Option<string>("--folders", "") { Description = "A semicolon-delimited list of folders to fetch" };
            var foldersListOpt = new Option<string>("--folders-list", "") { Description = "A file containing line-delimited list of folders to fetch" };
            var allowIndexMetadataOpt = new Option<bool>("--allow-index-metadata-update-from-working-tree", false) { Description =
                "When specified, index metadata (file times and sizes) is updated from disk if not already in the index." };
            var verboseOpt = new Option<bool>("--verbose", false) { Description = "Show all outputs on the console in addition to writing them to a log file" };
            var parentActivityIdOpt = new Option<string>("--parent-activity-id", "") { Description = "The GUID of the caller - used for telemetry purposes." };

            var rootCommand = new RootCommand("Fast-fetch a branch");
            rootCommand.AddOption(commitOpt);
            rootCommand.AddOption(branchOpt);
            rootCommand.AddOption(cacheServerUrlOpt);
            rootCommand.AddOption(chunkSizeOpt);
            rootCommand.AddOption(checkoutOpt);
            rootCommand.AddOption(forceCheckoutOpt);
            rootCommand.AddOption(searchThreadCountOpt);
            rootCommand.AddOption(downloadThreadCountOpt);
            rootCommand.AddOption(indexThreadCountOpt);
            rootCommand.AddOption(checkoutThreadCountOpt);
            rootCommand.AddOption(maxRetriesOpt);
            rootCommand.AddOption(gitPathOpt);
            rootCommand.AddOption(foldersOpt);
            rootCommand.AddOption(foldersListOpt);
            rootCommand.AddOption(allowIndexMetadataOpt);
            rootCommand.AddOption(verboseOpt);
            rootCommand.AddOption(parentActivityIdOpt);

            rootCommand.SetHandler((InvocationContext context) =>
            {
                var verb = new FastFetchVerb();
                verb.Commit = context.ParseResult.GetValueForOption(commitOpt);
                verb.Branch = context.ParseResult.GetValueForOption(branchOpt);
                verb.CacheServerUrl = context.ParseResult.GetValueForOption(cacheServerUrlOpt);
                verb.ChunkSize = context.ParseResult.GetValueForOption(chunkSizeOpt);
                verb.Checkout = context.ParseResult.GetValueForOption(checkoutOpt);
                verb.ForceCheckout = context.ParseResult.GetValueForOption(forceCheckoutOpt);
                verb.SearchThreadCount = context.ParseResult.GetValueForOption(searchThreadCountOpt);
                verb.DownloadThreadCount = context.ParseResult.GetValueForOption(downloadThreadCountOpt);
                verb.IndexThreadCount = context.ParseResult.GetValueForOption(indexThreadCountOpt);
                verb.CheckoutThreadCount = context.ParseResult.GetValueForOption(checkoutThreadCountOpt);
                verb.MaxAttempts = context.ParseResult.GetValueForOption(maxRetriesOpt);
                verb.GitBinPath = context.ParseResult.GetValueForOption(gitPathOpt);
                verb.FolderList = context.ParseResult.GetValueForOption(foldersOpt);
                verb.FolderListFile = context.ParseResult.GetValueForOption(foldersListOpt);
                verb.AllowIndexMetadataUpdateFromWorkingTree = context.ParseResult.GetValueForOption(allowIndexMetadataOpt);
                verb.Verbose = context.ParseResult.GetValueForOption(verboseOpt);
                verb.ParentActivityId = context.ParseResult.GetValueForOption(parentActivityIdOpt);
                verb.Execute();
            });

            return rootCommand.Invoke(args);
        }
    }
}
