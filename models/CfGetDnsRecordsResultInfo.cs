namespace Net.Fallenwood.Ddns.Models;

public sealed record CfGetDnsRecordsResultInfo(
    int Page,
    int PerPage,
    int Count,
    int TotalCount,
    int TotalPages
);
