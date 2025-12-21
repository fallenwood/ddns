using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Net.Fallenwood.Ddns.Client;

var services = new ServiceCollection();

var builder = new ConfigurationBuilder()
    .AddJsonFile("appsettings.json", optional: false, reloadOnChange: true);

var ring = Environment.GetEnvironmentVariable("DDNS_RING");
if (!string.IsNullOrWhiteSpace(ring))
{
    builder.AddJsonFile($"appsettings.{ring}.json", optional: true, reloadOnChange: true);
}

var configuration = builder.Build();

var zone = configuration.GetValue<string>("DDns:Zone") ?? throw new InvalidOperationException("DnsProvider:Zone is not configured");

var subDomain = configuration.GetValue<string>("DDns:Hostname") ?? throw new InvalidOperationException("DnsProvider:Hostname is not configured");

services.AddSingleton<IConfiguration>(configuration);
services.AddLogging(configure =>
{
    configure.AddConsole();
});

services.AddSingleton(sp =>
{
    var configuration = sp.GetRequiredService<IConfiguration>();

    SocketsHttpHandler socketsHttpHandler = new()
    {
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
    };

    return new HttpClient(socketsHttpHandler, disposeHandler: true);
});

services.AddHttpClient("IPv4")
    .ConfigurePrimaryHttpMessageHandler(() => new SocketsHttpHandler
    {
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
        ConnectCallback = async (context, token) =>
        {
            var socket = new Socket(SocketType.Stream, ProtocolType.Tcp)
            {
                DualMode = false // IPv4 only
            };

            await socket.ConnectAsync(context.DnsEndPoint, token).ConfigureAwait(false);

            return new NetworkStream(socket, ownsSocket: true);
        }
    });

services.AddHttpClient("IPv6")
    .ConfigurePrimaryHttpMessageHandler(() => new SocketsHttpHandler
    {
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
        ConnectCallback = async (context, token) =>
        {
            var socket = new Socket(SocketType.Stream, ProtocolType.Tcp)
            {
                DualMode = false // IPv6 only
            };
            socket.SetSocketOption(SocketOptionLevel.IPv6, SocketOptionName.IPv6Only, true);

            await socket.ConnectAsync(context.DnsEndPoint, token).ConfigureAwait(false);

            return new NetworkStream(socket, ownsSocket: true);
        }
    });

services.AddHttpClient();

if (configuration.GetValue<bool>("DDns:DnsProvider:Cloudflare:Enabled"))
{
    services.TryAddSingleton<IDnsProvider>(sp =>
    {
        var config = sp.GetRequiredService<IConfiguration>();
        var token = config.GetValue<string>("DDns:DnsProvider:Cloudflare:Token") ?? throw new InvalidOperationException("DnsProvider:Cloudflare:Token is not configured");
        var httpClientFactory = sp.GetRequiredService<IHttpClientFactory>();
        return new CfDnsProvider(httpClientFactory, zoneName: zone, token: token);
    });
}

if (configuration.GetValue<bool>("DDns:IPAddressProvider:Default:Enabled"))
{
    services.AddSingleton<IDnsProvider>(sp =>
    {
        var config = sp.GetRequiredService<IConfiguration>();
        var baseUrl = config.GetValue<string>("DDns:DnsProvider:Default:BaseUrl") ?? throw new InvalidOperationException("DnsProvider:BaseUrl is not configured");
        var httpCliet = sp.GetRequiredService<HttpClient>();
        return new DnsProvider(baseUrl, httpCliet);
    });
}

if (configuration.GetValue<bool>("DDns:IPAddressProvider:Default:Enabled"))
{
    services.AddSingleton<IIPAddressProvider>(sp =>
    {
        var config = sp.GetRequiredService<IConfiguration>();
        var baseUrl = config.GetValue<string>("DDns:IPAddressProvider:Default:BaseUrl") ?? throw new InvalidOperationException("IPAddressProvider:BaseUrl is not configured");
        var httpClientFactory = sp.GetRequiredService<IHttpClientFactory>();
        return new IPAddressProvider(baseUrl, httpClientFactory);
    });
}
else if (configuration.GetValue<bool>("DDns:IPAddressProvider:Cloudflare:Enabled"))
{
    services.AddSingleton<IIPAddressProvider>(sp =>
    {
        var config = sp.GetRequiredService<IConfiguration>();
        var httpClientFactory = sp.GetRequiredService<IHttpClientFactory>();
        return new CfIPAddressProvider(httpClientFactory);
    });
}
else if (configuration.GetValue<bool>("DDns:IPAddressProvider:Echo:Enabled"))
{
    services.AddSingleton<IIPAddressProvider>(sp =>
    {
        var config = sp.GetRequiredService<IConfiguration>();
        var baseUrl = config.GetValue<string>("DDns:IPAddressProvider:Echo:BaseUrl") ?? throw new InvalidOperationException("IPAddressProvider:BaseUrl is not configured");
        var httpClientFactory = sp.GetRequiredService<IHttpClientFactory>();
        return new EchoIPAddressProvider(baseUrl, httpClientFactory);
    });
}

var sp = services.BuildServiceProvider();

var ipAddressProvider = sp.GetRequiredService<IIPAddressProvider>();
var dnsProviders = sp.GetRequiredService<IEnumerable<IDnsProvider>>();

var logger = sp.GetRequiredService<ILogger<Program>>();

var hostname = subDomain + "." + zone;

logger.LogInformation("Starting DDNS client for hostname: {hostname}", hostname);

var initialDelay = 2;
var exp = 2;
var maxDelay = 60;

var delay = initialDelay;

while (true)
{
    await using var _ = Deferrer.DeferAsync(async () => await Task.Delay(TimeSpan.FromMinutes(delay)));

    try
    {
        var ipInfo = await ipAddressProvider.GetIPAddressInfoAsync("AAAA");

        var tasks = dnsProviders.Select(async dnsProvider =>
        {
            var currentRecords = await dnsProvider.GetDnsRecordsAsync(hostname);

            var existingRecord = currentRecords.FirstOrDefault(r => r.Type == ipInfo.Type);

            if (existingRecord?.Content == ipInfo.IPAddress)
            {
                logger.LogInformation("[{ProviderName}] No update needed for {hostname} ({type}): {ipAddress}", dnsProvider.Name, hostname, ipInfo.Type, ipInfo.IPAddress);
                return;
            }

            logger.LogInformation("[{ProviderName}]Creating DNS record for {hostname} ({type}): {ipAddress}", dnsProvider.Name, hostname, ipInfo.Type, ipInfo.IPAddress);
            await dnsProvider.UpsertDnsRecordAsync(existingRecord, hostname, ipInfo.IPAddress, ipInfo.Type, comment: "Created by DDNS client");
            logger.LogInformation("[{ProviderName}]DNS record created successfully.", dnsProvider.Name);
        });

        await Task.WhenAll(tasks);

        delay = Math.Min(maxDelay, delay * exp);
    }
    catch (Exception ex)
    {
        logger.LogError(ex, "An error occurred during DNS update.");
        delay = initialDelay;
    }
}
