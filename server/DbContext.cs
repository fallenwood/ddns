namespace Net.Fallenwood.Ddns;

using Dapper;
using Microsoft.Data.Sqlite;
using Net.Fallenwood.Ddns.Models;
using System.Data;

public class DbContext(IConfiguration configuration)
{
    private readonly string _connectionString = configuration.GetConnectionString("DefaultConnection") ?? throw new InvalidOperationException("Connection string 'DefaultConnection' not found.");

    private IDbConnection CreateConnection()
    {
        return new SqliteConnection(_connectionString);
    }

    public async Task Init()
    {
        using var connection = CreateConnection();
        // Ensure the database file is created
        if (connection is SqliteConnection sqliteConnection)
        {
            // This will create the file if it doesn't exist when we open it
        }

        connection.Open();

        // Enable WAL mode
        await connection.ExecuteAsync("PRAGMA journal_mode=WAL;");

        var sql = """
            CREATE TABLE IF NOT EXISTS DnsRecords (
                Id INTEGER PRIMARY KEY AUTOINCREMENT,
                Hostname TEXT NOT NULL,
                IPAddress TEXT NOT NULL,
                IPType TEXT NOT NULL,
                Comment TEXT,
                CreatedAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS IX_DnsRecords_HostName_IPType ON DnsRecords(Hostname, IPType);
        """;
        await connection.ExecuteAsync(sql);
    }

    public async Task<IEnumerable<DnsRecord>> GetDnsRecordsAsync()
    {
        using var connection = CreateConnection();
        var sql = "SELECT Id, Hostname, IPAddress, IPType, Comment, CreatedAt FROM DnsRecords;";
        return await connection.QueryAsync<DnsRecord>(sql);
    }

    public async Task<IEnumerable<DnsRecord>> GetDnsRecordByHostNameAsync(string hostName)
    {
        using var connection = CreateConnection();
        var sql = "SELECT Id, Hostname, IPAddress, IPType, Comment, CreatedAt FROM DnsRecords WHERE Hostname = @Hostname;";
        return await connection.QueryAsync<DnsRecord>(sql, new { Hostname = hostName });
    }

    public async Task<IEnumerable<DnsRecord>> GetLatestDnsRecordByHostnameAsync(string hostName, string ipType)
    {
        using var connection = CreateConnection();
        var sql = "SELECT Id, Hostname, IPAddress, IPType, Comment, CreatedAt FROM DnsRecords WHERE Hostname = @Hostname AND IPType = @IPType;";
        var records = await connection.QueryAsync<DnsRecord>(sql, new { Hostname = hostName, IPType = ipType });

        if (records is null || !records.Any())
        {
            return [];
        }

        return [records.OrderByDescending(r => r.CreatedAt).First()];
    }

    public async Task<DnsRecord> InsertDnsRecordAsync(string hostName, string ipAddress, string ipType, string? comment)
    {
        using var connection = CreateConnection();
        var sql = """
            INSERT INTO DnsRecords (Hostname, IPAddress, IPType, Comment)
            VALUES (@Hostname, @IPAddress, @IPType, @Comment)
            RETURNING Id, Hostname, IPAddress, IPType, Comment, CreatedAt;
        """;
        return await connection.QuerySingleAsync<DnsRecord>(
            sql,
            new
            {
                Hostname = hostName,
                IPAddress = ipAddress,
                IPType = ipType,
                Comment = comment
            });
    }
}
