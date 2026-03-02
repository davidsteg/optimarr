using System.Reflection;
using System.Text.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Moq;
using Optimarr.Controllers;
using Optimarr.Data;
using Optimarr.Models;
using Optimarr.Services;

namespace Optimarr.Tests.Regression;

public class RequestRegressionTests
{
    [Fact]
    public async Task RQ_E3_GetScan_PermissionDeniedMessageIsSanitized_AndFailedFileTracked()
    {
        await using var dbContext = CreateDbContext();

        var scan = new LibraryScan
        {
            Id = 100,
            LibraryPath = "/media/library",
            StartedAt = DateTime.UtcNow.AddMinutes(-2),
            Status = ScanStatus.Failed,
            TotalFiles = 1,
            ProcessedFiles = 0,
            FailedFiles = 1,
            ErrorMessage = "An internal error occurred. Check logs for details."
        };

        dbContext.LibraryScans.Add(scan);
        dbContext.FailedFiles.Add(new FailedFile
        {
            LibraryScanId = 100,
            FilePath = "/media/library/locked.mkv",
            FileName = "locked.mkv",
            ErrorType = "UnauthorizedAccessException",
            ErrorMessage = "Access denied: C:\\secret\\locked.mkv at Some.Stack.Trace()",
            FailedAt = DateTime.UtcNow
        });
        await dbContext.SaveChangesAsync();

        var controller = CreateLibraryController(dbContext);
        var actionResult = await controller.GetScan(100);
        var ok = Assert.IsType<OkObjectResult>(actionResult.Result);

        var json = JsonSerializer.Serialize(ok.Value);
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        Assert.Equal(1, root.GetProperty("failedFiles").GetInt32());
        var failedList = root.GetProperty("failedFilesList");
        Assert.Equal(1, failedList.GetArrayLength());

        var errorMessage = failedList[0].GetProperty("errorMessage").GetString();
        Assert.Equal("File access is restricted. Check permissions and retry.", errorMessage);
        Assert.DoesNotContain("Access denied", errorMessage!, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("Stack", errorMessage!, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public async Task RQ_E1_MigrationBanner_ActiveAndTerminalStateLogicPresent()
    {
        var repoRoot = FindRepositoryRoot();
        var migrationJsPath = Path.Combine(repoRoot, "wwwroot", "js", "migration.js");
        var content = await File.ReadAllTextAsync(migrationJsPath);

        Assert.Contains("progress.status === 'checking'", content, StringComparison.Ordinal);
        Assert.Contains("progress.status === 'creating'", content, StringComparison.Ordinal);
        Assert.Contains("progress.status === 'migrating'", content, StringComparison.Ordinal);
        Assert.Contains("progress.status === 'error'", content, StringComparison.Ordinal);
        Assert.Contains("hideMigrationBanner();", content, StringComparison.Ordinal);
        Assert.Contains("Migration completed/idle/error (not actively running), stop polling", content, StringComparison.Ordinal);
    }

    [Fact]
    public async Task RQ_E2_GetVideos_BrokenOnlyFilter_RemainsStrictAfterPipeline()
    {
        await using var dbContext = CreateDbContext();
        await SeedVideoRowsAsync(dbContext);

        var controller = CreateLibraryController(dbContext);
        var result = await controller.GetVideos(isBroken: true, score: "Poor");
        var ok = Assert.IsType<OkObjectResult>(result.Result);
        var payload = Assert.IsType<PagedResult<VideoAnalysis>>(ok.Value);

        Assert.NotEmpty(payload.Items);
        Assert.All(payload.Items, v => Assert.True(v.IsBroken));
    }

    [Fact]
    public async Task RQ_E2_GetVideoIds_BrokenOnlyFilter_RemainsStrictAfterPipeline()
    {
        await using var dbContext = CreateDbContext();
        await SeedVideoRowsAsync(dbContext);

        var controller = CreateLibraryController(dbContext);
        var result = await controller.GetVideoIds(isBroken: true, score: "Poor");
        var ok = Assert.IsType<OkObjectResult>(result.Result);
        var ids = Assert.IsType<List<int>>(ok.Value);

        Assert.NotEmpty(ids);
        var rows = await dbContext.VideoAnalyses.Where(v => ids.Contains(v.Id)).ToListAsync();
        Assert.All(rows, v => Assert.True(v.IsBroken));
    }

    [Fact]
    public async Task RQ_E3_PermissionRestrictedAnalysis_RethrowsAndDoesNotCreateBrokenRecord()
    {
        await using var dbContext = CreateDbContext();
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();
        var videoAnalyzer = new VideoAnalyzerService(configuration, null, dbContext);
        var scanner = new LibraryScannerService(
            dbContext,
            videoAnalyzer,
            new Mock<Microsoft.Extensions.Logging.ILogger<LibraryScannerService>>().Object,
            new Mock<IServiceScopeFactory>().Object);

        var tempDir = Path.Combine(Path.GetTempPath(), "optimarr-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        var filePath = Path.Combine(tempDir, "sample.mkv");
        await File.WriteAllTextAsync(filePath, "dummy");

        var method = typeof(LibraryScannerService).GetMethod(
            "AnalyzeAndStoreAsync",
            BindingFlags.Instance | BindingFlags.NonPublic);

        Assert.NotNull(method);

        var ex = await Assert.ThrowsAsync<UnauthorizedAccessException>(async () =>
        {
            var task = (Task)method!.Invoke(scanner, new object[] { filePath, 1, dbContext, videoAnalyzer })!;
            await task;
        });

        Assert.Contains("library roots", ex.Message, StringComparison.OrdinalIgnoreCase);
        Assert.Empty(await dbContext.VideoAnalyses.ToListAsync());
    }

    private static async Task SeedVideoRowsAsync(AppDbContext dbContext)
    {
        dbContext.VideoAnalyses.AddRange(
            new VideoAnalysis
            {
                FilePath = "/media/a/BrokenMovie.mkv",
                FileName = "BrokenMovie.mkv",
                IsBroken = true,
                CompatibilityRating = 10,
                AnalyzedAt = DateTime.UtcNow
            },
            new VideoAnalysis
            {
                FilePath = "/media/b/HealthyMovie.mkv",
                FileName = "HealthyMovie.mkv",
                IsBroken = false,
                CompatibilityRating = 10,
                AnalyzedAt = DateTime.UtcNow
            });

        await dbContext.SaveChangesAsync();
    }

    private static AppDbContext CreateDbContext()
    {
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseInMemoryDatabase(Guid.NewGuid().ToString("N"))
            .Options;
        return new AppDbContext(options);
    }

    private static LibraryController CreateLibraryController(AppDbContext dbContext)
    {
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();
        var logger = new Mock<Microsoft.Extensions.Logging.ILogger<LibraryController>>().Object;
        var videoAnalyzer = new VideoAnalyzerService(configuration, null, dbContext);
        var scannerLogger = new Mock<Microsoft.Extensions.Logging.ILogger<LibraryScannerService>>().Object;
        var serviceScopeFactory = new Mock<IServiceScopeFactory>().Object;
        var scanner = new LibraryScannerService(dbContext, videoAnalyzer, scannerLogger, serviceScopeFactory);

        var environment = new Mock<IWebHostEnvironment>();
        environment.SetupGet(x => x.ContentRootPath).Returns(FindRepositoryRoot());

        return new LibraryController(
            dbContext,
            scanner,
            videoAnalyzer,
            logger,
            configuration,
            environment.Object,
            serviceScopeFactory);
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

        throw new InvalidOperationException("Repository root not found.");
    }
}
