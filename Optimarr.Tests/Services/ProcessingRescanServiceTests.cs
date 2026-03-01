using System.Reflection;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Moq;
using Optimarr.Data;
using Optimarr.Models;
using Optimarr.Services;

namespace Optimarr.Tests.Services;

public class ProcessingRescanServiceTests
{
    [Fact]
    public async Task CheckAndRescanProcessingVideos_RescansOnlyItemsOlderThan24Hours()
    {
        var services = new ServiceCollection();
        services.AddDbContext<AppDbContext>(options => options.UseInMemoryDatabase(Guid.NewGuid().ToString("N")));
        services.AddSingleton(new VideoAnalyzerService());

        var provider = services.BuildServiceProvider();
        await SeedVideosAsync(provider);

        var service = new ProcessingRescanService(
            provider,
            new Mock<Microsoft.Extensions.Logging.ILogger<ProcessingRescanService>>().Object,
            new ConfigurationBuilder().AddInMemoryCollection().Build());

        var method = typeof(ProcessingRescanService)
            .GetMethod("CheckAndRescanProcessingVideos", BindingFlags.Instance | BindingFlags.NonPublic);
        Assert.NotNull(method);

        var task = (Task)method!.Invoke(service, new object[] { CancellationToken.None })!;
        await task;

        using var scope = provider.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<AppDbContext>();
        var videos = await db.VideoAnalyses.AsNoTracking().OrderBy(v => v.FilePath).ToListAsync();

        // Old "processing" item should be removed because file is missing and it is eligible for rescan.
        Assert.DoesNotContain(videos, v => v.FilePath == "/tmp/old-processing.mkv");

        // Recent item should be untouched because it is below the 24-hour threshold.
        Assert.Contains(videos, v => v.FilePath == "/tmp/recent-processing.mkv");
    }

    private static async Task SeedVideosAsync(IServiceProvider provider)
    {
        using var scope = provider.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<AppDbContext>();

        db.VideoAnalyses.AddRange(
            new VideoAnalysis
            {
                FilePath = "/tmp/old-processing.mkv",
                FileName = "old-processing.mkv",
                ProcessingStatus = ProcessingStatus.Processing,
                ProcessingStartedAt = DateTime.UtcNow.AddHours(-25),
                AnalyzedAt = DateTime.UtcNow
            },
            new VideoAnalysis
            {
                FilePath = "/tmp/recent-processing.mkv",
                FileName = "recent-processing.mkv",
                ProcessingStatus = ProcessingStatus.Processing,
                ProcessingStartedAt = DateTime.UtcNow.AddHours(-1),
                AnalyzedAt = DateTime.UtcNow
            },
            new VideoAnalysis
            {
                FilePath = "/tmp/not-processing.mkv",
                FileName = "not-processing.mkv",
                ProcessingStatus = ProcessingStatus.None,
                ProcessingStartedAt = DateTime.UtcNow.AddHours(-48),
                AnalyzedAt = DateTime.UtcNow
            });

        await db.SaveChangesAsync();
    }
}
