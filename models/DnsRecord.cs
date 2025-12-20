namespace Net.Fallenwood.Ddns.Models;

public sealed record DnsRecord(
    int Id,
    string Hostname,
    string IPAddress,
    string IPType,
    string CreatedAt,
    string? Comment
);
