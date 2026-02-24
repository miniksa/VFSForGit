// HTTP Handler Benchmark — simulates the exact GVFS HttpRequestor pattern.
//
// Tests three configurations:
//   A) WinHttpHandler + NTLM + Basic header  (previous "fix")
//   B) SocketsHttpHandler + NTLM + Basic header  (the slow path)
//   C) SocketsHttpHandler WITHOUT NTLM + Basic header  (proposed fix)
//
// Usage:
//   dotnet run -c Release -- <gvfs-endpoint-url> <pat-or-oauth-token>
//
// Examples:
//   # Against a GVFS cache server (run on a machine with OS enlistment):
//   dotnet run -c Release -- https://gitoc-reddog.corp.microsoft.com/<org>/<repo>/gvfs/config <PAT>
//
//   # Against Azure DevOps (works from any corpnet machine):
//   $token = az account get-access-token --resource 499b84ac-1321-427f-aa17-267ca6975798 --query accessToken -o tsv
//   dotnet run -c Release -- https://dev.azure.com/microsoft/_apis/connectionData $token
//
// The benchmark sends "Authorization: Basic <base64(:token)>" on every request,
// exactly like GVFS HttpRequestor.SendRequest does. The question is whether the
// handler ALSO does an NTLM handshake underneath.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Threading;
using System.Threading.Tasks;

namespace GVFS.HttpBenchmark;

internal static class Program
{
    private const int WarmupRequests = 5;
    private const int MeasuredRequests = 30;

    static async Task<int> Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: dotnet run -c Release -- <url> <token>");
            Console.Error.WriteLine();
            Console.Error.WriteLine("  url    GVFS endpoint or any authenticated HTTPS endpoint");
            Console.Error.WriteLine("  token  PAT or OAuth token (sent as Basic auth, same as GVFS)");
            return 1;
        }

        string url = args[0];
        // GVFS sends ":token" base64-encoded as Basic auth (username is empty)
        string basicAuth = Convert.ToBase64String(
            System.Text.Encoding.ASCII.GetBytes(":" + args[1]));

        Console.WriteLine("HTTP Handler Benchmark (simulates GVFS HttpRequestor)");
        Console.WriteLine($"  URL:       {url}");
        Console.WriteLine($"  Warmup:    {WarmupRequests}");
        Console.WriteLine($"  Measured:  {MeasuredRequests}");
        Console.WriteLine($"  CPUs:      {Environment.ProcessorCount}");
        Console.WriteLine();

        // The three configs that matter:
        var configs = new (string Name, Func<HttpMessageHandler> Factory)[]
        {
            // A: What we had before (WinHttpHandler + NTLM creds).
            //    Fast (~11ms) because WinHTTP does NTLM natively, but NTLM is redundant.
            ("A) WinHttp + NTLM creds", () => new WinHttpHandler
            {
                ServerCredentials = CredentialCache.DefaultCredentials,
                MaxConnectionsPerServer = Environment.ProcessorCount,
            }),

            // B: SocketsHttpHandler WITH NTLM creds.
            //    Slow (~400ms) because managed NTLM handshake is expensive.
            ("B) Sockets + NTLM creds", () => new SocketsHttpHandler
            {
                Credentials = CredentialCache.DefaultCredentials,
            }),

            // C: SocketsHttpHandler WITHOUT any transport-level creds.
            //    Should be fast — no NTLM handshake, auth is purely via header.
            ("C) Sockets, NO NTLM (token only)", () => new SocketsHttpHandler
            {
                MaxConnectionsPerServer = Environment.ProcessorCount,
                PooledConnectionLifetime = Timeout.InfiniteTimeSpan,
                PooledConnectionIdleTimeout = TimeSpan.FromMinutes(5),
            }),
        };

        Console.WriteLine($"{"Config",-34} {"Cold",8} {"Avg",8} {"Min",8} {"Max",8} {"P50",8} {"P95",8}");
        Console.WriteLine(new string('-', 82));

        foreach (var (name, factory) in configs)
        {
            try
            {
                var r = await Benchmark(url, basicAuth, factory);
                Console.WriteLine(
                    $"{name,-34} {r.ColdMs,7:F1}ms {r.AvgMs,7:F1}ms {r.MinMs,7:F1}ms " +
                    $"{r.MaxMs,7:F1}ms {r.P50Ms,7:F1}ms {r.P95Ms,7:F1}ms");
            }
            catch (Exception ex)
            {
                Console.WriteLine($"{name,-34} FAILED: {ex.GetType().Name}: {ex.Message}");
            }

            await Task.Delay(500); // let connections drain
        }

        Console.WriteLine();
        return 0;
    }

    static async Task<Result> Benchmark(string url, string basicAuth, Func<HttpMessageHandler> factory)
    {
        using var handler = factory();
        using var client = new HttpClient(handler)
        {
            Timeout = TimeSpan.FromSeconds(30),
            DefaultRequestVersion = HttpVersion.Version11,
            DefaultVersionPolicy = HttpVersionPolicy.RequestVersionExact,
        };
        // Set the Authorization header exactly like GVFS does
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Basic", basicAuth);
        client.DefaultRequestHeaders.Add("X-TFS-FedAuthRedirect", "Suppress");

        var uri = new Uri(url);

        // Cold request
        var sw = Stopwatch.StartNew();
        using (var resp = await client.GetAsync(uri))
        {
            resp.EnsureSuccessStatusCode();
            await resp.Content.ReadAsStringAsync();
        }
        sw.Stop();
        double coldMs = sw.Elapsed.TotalMilliseconds;

        // Warmup
        for (int i = 0; i < WarmupRequests; i++)
        {
            using var resp = await client.GetAsync(uri);
            await resp.Content.ReadAsStringAsync();
        }

        // Measured
        var times = new List<double>(MeasuredRequests);
        for (int i = 0; i < MeasuredRequests; i++)
        {
            sw.Restart();
            using (var resp = await client.GetAsync(uri))
            {
                sw.Stop();
                times.Add(sw.Elapsed.TotalMilliseconds);
                await resp.Content.ReadAsStringAsync();
            }
        }

        times.Sort();
        return new Result
        {
            ColdMs = coldMs,
            AvgMs = times.Average(),
            MinMs = times.Min(),
            MaxMs = times.Max(),
            P50Ms = Pct(times, 0.50),
            P95Ms = Pct(times, 0.95),
        };
    }

    static double Pct(List<double> sorted, double p) =>
        sorted[Math.Max(0, Math.Min((int)Math.Ceiling(p * sorted.Count) - 1, sorted.Count - 1))];

    record struct Result
    {
        public double ColdMs, AvgMs, MinMs, MaxMs, P50Ms, P95Ms;
    }
}
