namespace Net.Fallenwood.Ddns.Models;

public sealed record CfDnsRecord(
    string Id,
    string Name,
    string Type,
    string Content,
    bool Proxied,
    int Ttl,
    string? Comment
);
