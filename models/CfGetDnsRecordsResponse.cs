namespace Net.Fallenwood.Ddns.Models;

public sealed record CfGetDnsRecordsResponse(
    bool Success,
    CfDnsRecord[] Result);
