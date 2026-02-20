using GVFS.Common.Http;
using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Linq;

namespace GVFS.Common
{
    public class ServerGVFSConfig
    {
        public IEnumerable<VersionRange> AllowedGVFSClientVersions { get; set; }

        public IEnumerable<CacheServerInfo> CacheServers { get; set; } = Enumerable.Empty<CacheServerInfo>();

        public class VersionRange
        {
            [JsonConverter(typeof(VersionConverter))]
            public Version Min { get; set; }

            [JsonConverter(typeof(VersionConverter))]
            public Version Max { get; set; }
        }
    }
}