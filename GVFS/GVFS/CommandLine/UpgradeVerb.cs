using System;

namespace GVFS.CommandLine
{
    public class UpgradeVerb : GVFSVerb.ForNoEnlistment
    {
        private const string UpgradeVerbName = "upgrade";

        public UpgradeVerb()
        {
            this.Output = Console.Out;
        }

        public bool Confirmed { get; set; }

        public bool DryRun { get; set; }

        public bool NoVerify { get; set; }

        protected override string VerbName
        {
            get { return UpgradeVerbName; }
        }

        public override void Execute()
        {
            Console.Error.WriteLine("'gvfs upgrade' is no longer supported. Visit https://github.com/microsoft/vfsforgit for the latest install/upgrade instructions.");
        }
    }
}
