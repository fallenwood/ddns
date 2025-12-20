namespace Net.Fallenwood.Ddns.Client;

using System.Net.Http.Json;
using Net.Fallenwood.Ddns.Models;

public interface IDnsProvider
{
    public string Name{get;}
    public Task<IEnumerable<CfDnsRecord>> GetDnsRecordsAsync(string hostname);
    public Task<IEnumerable<CfDnsRecord>> UpsertDnsRecordAsync(CfDnsRecord? record, string hostName, string ipAddress, string ipType, string? comment);
}

public sealed class CfDnsProvider(IHttpClientFactory httpClientFactory, string zoneName, string token) : IDnsProvider
{
    private readonly string baseUrl = "https://api.cloudflare.com/client/v4";

    private string? zoneId = null;

    public string Name => "Cloudflare";

    private async Task<string?> GetZoneIdAsync(string zoneName)
    {
        var httpClient = httpClientFactory.CreateClient();

        var url = $"{baseUrl}/zones";

        var requestMessage = new HttpRequestMessage(HttpMethod.Get, url);
        this.AddAuthorizationHeader(requestMessage);

        var response = await httpClient.SendAsync(requestMessage);
        var zoneResponse = await response.Content.ReadFromJsonAsync(
            AppJsonSerializerContext.Default.CfGetZonesResponse);

        return zoneResponse!.Result
            .FirstOrDefault(e => e.Name == zoneName)
            ?.Id;
    }

    public async Task<IEnumerable<CfDnsRecord>> GetDnsRecordsAsync(string hostname)
    {
        if (string.IsNullOrWhiteSpace(this.zoneId))
        {
            this.zoneId = await this.GetZoneIdAsync(zoneName);
        }

        if (string.IsNullOrWhiteSpace(this.zoneId))
        {
            throw new InvalidOperationException($"Failed to find Zone ID for zone: {zoneName}");
        }

        var httpClient = httpClientFactory.CreateClient();

        var url = $"{baseUrl}/zones/{this.zoneId}/dns_records";
        var requestMessage = new HttpRequestMessage(HttpMethod.Get, url);
        this.AddAuthorizationHeader(requestMessage);

        var response = await httpClient.SendAsync(requestMessage);
        var dnsRecordsResponse = await response.Content.ReadFromJsonAsync(
            AppJsonSerializerContext.Default.CfGetDnsRecordsResponse);

        return dnsRecordsResponse!.Result.Where(e => e.Name == hostname).ToArray();
    }

    public async Task<IEnumerable<CfDnsRecord>> UpsertDnsRecordAsync(CfDnsRecord? record, string hostName, string ipAddress, string ipType, string? comment)
    {        
        if (string.IsNullOrWhiteSpace(this.zoneId))
        {
            this.zoneId = await this.GetZoneIdAsync(zoneName);
        }

        if (string.IsNullOrWhiteSpace(this.zoneId))
        {
            throw new InvalidOperationException($"Failed to find Zone ID for zone: {zoneName}");
        }
        
        var httpClient = httpClientFactory.CreateClient();

        HttpRequestMessage requestMessage;
        if (record is null)
        {
            requestMessage = new HttpRequestMessage(
                HttpMethod.Post,
                $"{baseUrl}/zones/{this.zoneId}/dns_records")
                {
                    Content = JsonContent.Create(
                        new CfPostOrPutDnsRecordRequest(
                            Name: hostName,
                            Type: ipType,
                            Content: ipAddress,
                            Proxied: false,
                            Ttl: 60,
                            Comment: comment),
                        AppJsonSerializerContext.Default.CfPostOrPutDnsRecordRequest)
                };
        }
        else
        {
            requestMessage = new HttpRequestMessage(
                HttpMethod.Put,
                $"{baseUrl}/zones/{this.zoneId}/dns_records/{record.Id}")
                {
                    Content = JsonContent.Create(
                        new CfPostOrPutDnsRecordRequest(
                            Name: hostName,
                            Type: ipType,
                            Content: ipAddress,
                            Proxied: false,
                            Ttl: 60,
                            Comment: comment),
                        AppJsonSerializerContext.Default.CfPostOrPutDnsRecordRequest)
                };
        }
        this.AddAuthorizationHeader(requestMessage);

        var responseMessage = await httpClient.SendAsync(requestMessage);

        var response = await responseMessage.Content.ReadFromJsonAsync(
            AppJsonSerializerContext.Default.CfPostOrPutDnsRecordResponse);

        return [response!.Result];
    }

    private void AddAuthorizationHeader(HttpRequestMessage requestMessage)
    {
        requestMessage.Headers.Add("Authorization", $"Bearer {token}");
    }
}

public sealed class DnsProvider(string baseUrl, HttpClient httpClient) : IDnsProvider
{
    private readonly string baseUrl = baseUrl.TrimEnd('/');

    public string Name => "Default";

    public async Task<IEnumerable<CfDnsRecord>> GetDnsRecordsAsync(string hostname)
    {
        var url = $"{baseUrl}/api/v1/dns-records/{Uri.EscapeDataString(hostname)}";

        var repsonse = await httpClient.GetFromJsonAsync(url, AppJsonSerializerContext.Default.CfGetDnsRecordsResponse);

        return repsonse!.Result;
    }

    public async Task<IEnumerable<CfDnsRecord>> UpsertDnsRecordAsync(CfDnsRecord? record, string hostName, string ipAddress, string ipType, string? comment)
    {
        _ = record;

        var url = $"{baseUrl}/api/v1/dns-records";

        var request = new CfPostOrPutDnsRecordRequest(
            Name: hostName,
            Type: ipType,
            Content: ipAddress,
            Proxied: false,
            Ttl: 60,
            Comment: comment
        );

        var requestMessage = new HttpRequestMessage(HttpMethod.Post, url)
        {
            Content = JsonContent.Create(
                request,
                AppJsonSerializerContext.Default.CfPostOrPutDnsRecordRequest)
        };
        

        var responseMessage = await httpClient.SendAsync(requestMessage);

        var response = await responseMessage.Content.ReadFromJsonAsync(
            AppJsonSerializerContext.Default.CfPostOrPutDnsRecordResponse);

        return [response!.Result];
    }
}
