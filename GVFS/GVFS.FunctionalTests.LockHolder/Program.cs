using System.CommandLine;
using System.CommandLine.Invocation;

namespace GVFS.FunctionalTests.LockHolder
{
    public class Program
    {
        public static int Main(string[] args)
        {
            var skipReleaseLockOption = new Option<bool>(
                "--skip-release-lock",
                () => false,
                "Skip releasing the GVFS lock when exiting the program.");

            var rootCommand = new RootCommand("Acquire and hold the GVFS lock for testing");
            rootCommand.AddOption(skipReleaseLockOption);

            rootCommand.SetHandler((InvocationContext context) =>
            {
                var verb = new AcquireGVFSLockVerb();
                verb.NoReleaseLock = context.ParseResult.GetValueForOption(skipReleaseLockOption);
                verb.Execute();
            });

            return rootCommand.Invoke(args);
        }
    }
}
