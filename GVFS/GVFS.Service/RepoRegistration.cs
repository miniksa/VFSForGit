using GVFS.Common;
using System.Text.Json;

namespace GVFS.Service
{
    public class RepoRegistration
    {
        public RepoRegistration()
        {
        }

        public RepoRegistration(string enlistmentRoot, string ownerSID)
        {
            this.EnlistmentRoot = enlistmentRoot;
            this.OwnerSID = ownerSID;
            this.IsActive = true;
        }

        public string EnlistmentRoot { get; set; }
        public string OwnerSID { get; set; }
        public bool IsActive { get; set; }

        public static RepoRegistration FromJson(string json)
        {
            // System.Text.Json ignores missing members by default,
            // matching the previous MissingMemberHandling.Ignore behavior.
            return JsonSerializer.Deserialize<RepoRegistration>(json, GVFSJsonOptions.Default);
        }

        public override string ToString()
        {
            return
                string.Format(
                    "({0} - {1}) {2}",
                    this.IsActive ? "Active" : "Inactive",
                    this.OwnerSID,
                    this.EnlistmentRoot);
        }

        public string ToJson()
        {
            return JsonSerializer.Serialize(this, GVFSJsonOptions.Default);
        }
    }
}