namespace Net.Fallenwood.Ddns.Client;

using System.Net.Http.Json;
using Net.Fallenwood.Ddns.Models;

public interface IIPAddressProvider
{
    Task<IPAddressInfo> GetIPAddressInfoAsync(string type);
}

public sealed class IPAddressProvider(string baseUrl, IHttpClientFactory httpClientFactory) : IIPAddressProvider
{
    private readonly string baseUrl = baseUrl.TrimEnd('/');

    public async Task<IPAddressInfo> GetIPAddressInfoAsync(string type)
    {
        var url = $"{baseUrl}/api/v1/ip";

        var httpClient = type switch
        {
            "A" => httpClientFactory.CreateClient("IPv4"),
            "AAAA" => httpClientFactory.CreateClient("IPv6"),
            _ => httpClientFactory.CreateClient(),
        };

        var response = await httpClient.GetFromJsonAsync(url, AppJsonSerializerContext.Default.GetIPAddressResponse);

        return response!.Result;
    }
}

public sealed class CfIPAddressProvider(IHttpClientFactory httpClientFactory) : IIPAddressProvider
{
    private readonly string baseUrl = "https://1.1.1.1/cdn-cgi/trace";

    public async Task<IPAddressInfo> GetIPAddressInfoAsync(string type)
    {
        var httpClient = httpClientFactory.CreateClient();

        var response = await httpClient.GetStringAsync(baseUrl);

        var lines = response.Split('\n', StringSplitOptions.RemoveEmptyEntries);

        var ipLine = lines.FirstOrDefault(line => line.StartsWith("ip="));

        if (ipLine == null)
        {
            throw new InvalidOperationException("Failed to retrieve IP address from Cloudflare trace response");
        }

        var ipAddress = ipLine[3..].Trim();

        return new IPAddressInfo(ipAddress, type);
    }
}

public sealed class EchoIPAddressProvider(string baseUrl, IHttpClientFactory httpClientFactory) : IIPAddressProvider
{
    public async Task<IPAddressInfo> GetIPAddressInfoAsync(string type)
    {
        var httpClient = type switch
        {
            "A" => httpClientFactory.CreateClient("IPv4"),
            "AAAA" => httpClientFactory.CreateClient("IPv6"),
            _ => httpClientFactory.CreateClient(),
        };

        var response = await httpClient.GetAsync(baseUrl);

        var ipAddress = response.Headers.GetValues("X-Client-IP").FirstOrDefault();

        return new IPAddressInfo(ipAddress, type);
    }
}