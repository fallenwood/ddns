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
