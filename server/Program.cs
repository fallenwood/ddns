using System.Net;
using Dapper;
using Microsoft.AspNetCore.Mvc;
using Net.Fallenwood.Ddns;
using Net.Fallenwood.Ddns.Models;

[module: DapperAot]

var builder = WebApplication.CreateSlimBuilder(args);

builder.Services.ConfigureHttpJsonOptions(options =>
{
    options.SerializerOptions.TypeInfoResolverChain.Insert(0, AppJsonSerializerContext.Default);
});

builder.Services.AddHealthChecks();

builder.Services.AddAuthentication();
builder.Services.AddAuthorization();

builder.Services.AddSingleton<DbContext>();

var app = builder.Build();

using (var scope = app.Services.CreateScope())
{
    var dbContext = scope.ServiceProvider.GetRequiredService<DbContext>();
    await dbContext.Init();
}

app.MapHealthChecks("/healthz");

// app.UseAuthentication();
// app.UseAuthorization();

var apiV1 = app.MapGroup("/api/v1");

apiV1.MapGet("ip", (HttpContext context, [FromServices] ILogger<Program> logger) =>
{
    string? ipAddress;

    if (context.Request.Headers.TryGetValue("x-real-ip", out var realIp))
    {
        ipAddress = realIp.ToString();
    }
    else if (context.Request.Headers.TryGetValue("x-forwarded-for", out var forwardedFor))
    {
        ipAddress = forwardedFor.ToString().Split(',')[0];
    }
    else
    {
        ipAddress = context.Connection.RemoteIpAddress?.ToString();
    }

    if (ipAddress == null || !IPAddress.TryParse(ipAddress, out var ip))
    {
        logger.LogWarning("Failed to determine client IP address: {ipAdddress}", ipAddress);
        return Results.BadRequest();
    }

    var info = new IPAddressInfo(ipAddress, ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork ? "A" : "AAAA");

    var response = new GetIPAddressResponse(info);

    return Results.Json(response, AppJsonSerializerContext.Default.GetIPAddressResponse);
});

apiV1.MapGet("/dns-records", async ([FromServices] DbContext dbContext) =>
{
    var records = await dbContext.GetDnsRecordsAsync();
    var response = new CfGetDnsRecordsResponse(
        true,
        [..
            records.Select(r => new CfDnsRecord(
                r.Id.ToString(),
                r.Hostname,
                r.IPType,
                r.IPAddress,
                false,
                -1,
                r.Comment
            ))]);
    return response;
});
// .RequireAuthorization();

apiV1.MapGet("/dns-records/{hostName}", async (
    [FromRoute] string hostName,
    [FromServices] DbContext dbContext) =>
{
    var ipv4Records = await dbContext.GetLatestDnsRecordByHostnameAsync(hostName, "A");
    var ipv6Records = await dbContext.GetLatestDnsRecordByHostnameAsync(hostName, "AAAA");

    var records = ipv4Records.Concat(ipv6Records).ToArray();

    var response = new CfGetDnsRecordsResponse(
        true,
        [..
            records.Select(r => new CfDnsRecord(
                r.Id.ToString(),
                r.Hostname,
                r.IPType,
                r.IPAddress,
                false,
                -1,
                r.Comment
            ))]);
    return response;
});
// .RequireAuthorization();

apiV1.MapPost("/dns-records", async (
    [FromBody] CfPostOrPutDnsRecordRequest request,
    [FromServices] DbContext dbContext) =>
{
    var records = await dbContext.InsertDnsRecordAsync(
        request.Name,
        request.Content,
        request.Type,
        request.Comment);
    var response = new CfPostOrPutDnsRecordResponse(
        true,
        new CfDnsRecord(
            records.Id.ToString(),
            records.Hostname,
            records.IPType,
            records.IPAddress,
            false,
            -1,
            records.Comment
        ));
    return response;
});
// .RequireAuthorization();

await app.RunAsync();
