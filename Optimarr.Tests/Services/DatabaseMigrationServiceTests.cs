using Microsoft.Data.Sqlite;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Moq;
using Optimarr.Data;
using Optimarr.Services;

namespace Optimarr.Tests.Services;

public class DatabaseMigrationServiceTests
{
    [Fact]
    public async Task StartAsync_WhenDatabaseDoesNotExist_CreatesSchemaAndSetsCompletedProgress()
    {
        var dbFile = Path.Combine(Path.GetTempPath(), "optimarr-tests", $"{Guid.NewGuid():N}.db");
        Directory.CreateDirectory(Path.GetDirectoryName(dbFile)!);
        if (File.Exists(dbFile))
        {
            File.Delete(dbFile);
        }

        var serviceProvider = BuildServiceProvider(dbFile);
        var service = CreateService(serviceProvider);

        await service.StartAsync(CancellationToken.None);

        Assert.True(File.Exists(dbFile));
        var progress = DatabaseMigrationService.GetMigrationProgress();
        Assert.NotNull(progress);
        Assert.Equal("completed", progress!.Status);
    }

    [Fact]
    public async Task StartAsync_WhenProcessingStatusColumnsMissing_AppliesSqlMigration()
    {
        var dbFile = Path.Combine(Path.GetTempPath(), "optimarr-tests", $"{Guid.NewGuid():N}.db");
        Directory.CreateDirectory(Path.GetDirectoryName(dbFile)!);
        if (File.Exists(dbFile))
        {
            File.Delete(dbFile);
        }

        await SeedLegacyServarrSchemaWithoutProcessingColumnsAsync(dbFile);

        var serviceProvider = BuildServiceProvider(dbFile);
        var service = CreateService(serviceProvider);
        await service.StartAsync(CancellationToken.None);

        await using var connection = new SqliteConnection($"Data Source={dbFile}");
        await connection.OpenAsync();
        await using var command = connection.CreateCommand();
        command.CommandText = "SELECT COUNT(*) FROM pragma_table_info('VideoAnalyses') WHERE name='ProcessingStatus'";
        var columnExists = Convert.ToInt32(await command.ExecuteScalarAsync()) > 0;

        Assert.True(columnExists);

        var progress = DatabaseMigrationService.GetMigrationProgress();
        Assert.NotNull(progress);
        Assert.Equal("completed", progress!.Status);
    }

    private static IServiceProvider BuildServiceProvider(string dbFile)
    {
        var services = new ServiceCollection();
        services.AddLogging();
        services.AddDbContext<AppDbContext>(options => options.UseSqlite($"Data Source={dbFile}"));
        return services.BuildServiceProvider();
    }

    private static DatabaseMigrationService CreateService(IServiceProvider serviceProvider)
    {
        var hostEnv = new Mock<IHostEnvironment>();
        hostEnv.SetupGet(x => x.ContentRootPath).Returns(FindRepositoryRoot());

        return new DatabaseMigrationService(
            serviceProvider,
            new LoggerFactory().CreateLogger<DatabaseMigrationService>(),
            hostEnv.Object,
            new ConfigurationBuilder().AddInMemoryCollection().Build());
    }

    private static async Task SeedLegacyServarrSchemaWithoutProcessingColumnsAsync(string dbFile)
    {
        await using var connection = new SqliteConnection($"Data Source={dbFile}");
        await connection.OpenAsync();

        var sql = """
                  CREATE TABLE LibraryScans (
                    Id INTEGER PRIMARY KEY AUTOINCREMENT,
                    LibraryPath TEXT NOT NULL,
                    StartedAt TEXT NOT NULL,
                    Status TEXT NOT NULL,
                    CurrentProcessingFile TEXT
                  );

                  CREATE TABLE LibraryPaths (
                    Id INTEGER PRIMARY KEY AUTOINCREMENT,
                    Path TEXT NOT NULL,
                    ServarrType TEXT,
                    ServarrRootFolderId INTEGER,
                    ServarrRootFolderPath TEXT,
                    LastSyncedAt TEXT
                  );

                  CREATE TABLE VideoAnalyses (
                    Id INTEGER PRIMARY KEY AUTOINCREMENT,
                    FilePath TEXT NOT NULL,
                    FileName TEXT NOT NULL DEFAULT '',
                    FileSize INTEGER NOT NULL DEFAULT 0,
                    Duration REAL NOT NULL DEFAULT 0,
                    Container TEXT NOT NULL DEFAULT '',
                    VideoCodec TEXT NOT NULL DEFAULT '',
                    VideoCodecTag TEXT NOT NULL DEFAULT '',
                    IsCodecTagCorrect INTEGER NOT NULL DEFAULT 1,
                    BitDepth INTEGER NOT NULL DEFAULT 8,
                    Width INTEGER NOT NULL DEFAULT 0,
                    Height INTEGER NOT NULL DEFAULT 0,
                    FrameRate REAL NOT NULL DEFAULT 0,
                    IsHDR INTEGER NOT NULL DEFAULT 0,
                    HDRType TEXT NOT NULL DEFAULT '',
                    IsFastStart INTEGER NOT NULL DEFAULT 0,
                    AudioCodecs TEXT NOT NULL DEFAULT '',
                    AudioTrackCount INTEGER NOT NULL DEFAULT 0,
                    AudioTracksJson TEXT NOT NULL DEFAULT '',
                    SubtitleFormats TEXT NOT NULL DEFAULT '',
                    SubtitleTrackCount INTEGER NOT NULL DEFAULT 0,
                    SubtitleTracksJson TEXT NOT NULL DEFAULT '',
                    OverallScore TEXT NOT NULL DEFAULT 'Unknown',
                    CompatibilityRating INTEGER NOT NULL DEFAULT 0,
                    DirectPlayClients INTEGER NOT NULL DEFAULT 0,
                    RemuxClients INTEGER NOT NULL DEFAULT 0,
                    TranscodeClients INTEGER NOT NULL DEFAULT 0,
                    Issues TEXT NOT NULL DEFAULT '',
                    Recommendations TEXT NOT NULL DEFAULT '',
                    ClientResults TEXT NOT NULL DEFAULT '',
                    FullReport TEXT NOT NULL DEFAULT '',
                    IsBroken INTEGER NOT NULL DEFAULT 0,
                    BrokenReason TEXT,
                    AnalyzedAt TEXT NOT NULL,
                    LibraryScanId INTEGER,
                    ServarrType TEXT,
                    SonarrSeriesId INTEGER,
                    SonarrSeriesTitle TEXT,
                    SonarrEpisodeId INTEGER,
                    SonarrEpisodeNumber INTEGER,
                    SonarrSeasonNumber INTEGER,
                    RadarrMovieId INTEGER,
                    RadarrMovieTitle TEXT,
                    RadarrYear INTEGER,
                    ServarrMatchedAt TEXT
                  );

                  INSERT INTO VideoAnalyses (
                    FilePath, FileName, AnalyzedAt, ServarrType
                  ) VALUES (
                    '/tmp/sample.mkv', 'sample.mkv', '2026-03-01T00:00:00Z', 'Radarr'
                  );
                  """;

        await using var command = connection.CreateCommand();
        command.CommandText = sql;
        await command.ExecuteNonQueryAsync();
    }

    private static string FindRepositoryRoot()
    {
        var current = new DirectoryInfo(AppContext.BaseDirectory);
        while (current != null)
        {
            if (File.Exists(Path.Combine(current.FullName, "optimarr.csproj")))
            {
                return current.FullName;
            }

            current = current.Parent;
        }

        throw new InvalidOperationException("Could not locate repository root containing optimarr.csproj");
    }
}
