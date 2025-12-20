namespace Net.Fallenwood.Ddns.Models;

public sealed record CfPostOrPutDnsRecordRequest(
    string Name,
    string Type,
    string Content,
    bool Proxied,
    int Ttl,
    string? Comment
);
