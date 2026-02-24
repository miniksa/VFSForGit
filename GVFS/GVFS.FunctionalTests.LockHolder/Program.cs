using System.CommandLine;

namespace GVFS.FunctionalTests.LockHolder
{
    public class Program
    {
        public static int Main(string[] args)
        {
            var skipReleaseLockOption = new Option<bool>(
                "--skip-release-lock", false)
            { Description = "Skip releasing the GVFS lock when exiting the program." };

            var rootCommand = new RootCommand("Acquire and hold the GVFS lock for testing");
            rootCommand.Options.Add(skipReleaseLockOption);

            rootCommand.SetAction((parseResult) =>
            {
                var verb = new AcquireGVFSLockVerb();
                verb.NoReleaseLock = parseResult.GetValue(skipReleaseLockOption);
                verb.Execute();
            });

            return rootCommand.Parse(args).Invoke();
        }
    }
}
