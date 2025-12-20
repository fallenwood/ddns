namespace Net.Fallenwood.Ddns.Models;

public sealed record CfPostOrPutDnsRecordResponse(
    bool Success,
    CfDnsRecord Result
);
