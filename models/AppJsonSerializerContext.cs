namespace Net.Fallenwood.Ddns.Models;

using System.Text.Json.Serialization;

[JsonSerializable(typeof(GetIPAddressResponse))]
[JsonSerializable(typeof(UpdateIPAddressRequest))]
[JsonSerializable(typeof(UpdateIPAddressResponse))]
[JsonSerializable(typeof(CfGetDnsRecordsResponse))]
[JsonSerializable(typeof(CfGetDnsRecordsResultInfo))]
[JsonSerializable(typeof(CfPostOrPutDnsRecordRequest))]
[JsonSerializable(typeof(CfPostOrPutDnsRecordResponse))]
[JsonSerializable(typeof(CfGetZonesResponse))]
[JsonSerializable(typeof(CfDnsRecord))]
[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
public partial class AppJsonSerializerContext : JsonSerializerContext
{

}
