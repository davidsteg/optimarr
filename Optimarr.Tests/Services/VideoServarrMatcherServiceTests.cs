using Microsoft.Extensions.Configuration;
using Moq;
using Optimarr.Models;
using Optimarr.Services;
using Optimarr.Tests.Common;

namespace Optimarr.Tests.Services;

public class VideoServarrMatcherServiceTests : TestBase
{
    [Fact]
    public async Task MatchVideoWithServarrAsync_WhenExactPathMatchesSonarr_PopulatesSonarrFields()
    {
        using var dbContext = CreateInMemoryDbContext();
        var sonarr = new Mock<ISonarrService>();
        var radarr = new Mock<IRadarrService>();
        var logger = CreateLogger<VideoServarrMatcherService>();
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();

        sonarr.SetupGet(x => x.IsEnabled).Returns(true);
        sonarr.SetupGet(x => x.IsConnected).Returns(true);
        radarr.SetupGet(x => x.IsEnabled).Returns(false);
        radarr.SetupGet(x => x.IsConnected).Returns(false);

        var matcher = new VideoServarrMatcherService(dbContext, sonarr.Object, radarr.Object, logger.Object, configuration);
        var video = new VideoAnalysis { Id = 1, FilePath = "/tv/Show/Season 1/Episode 01.mkv" };

        var episodesByPath = new Dictionary<string, SonarrEpisode>
        {
            ["/tv/show/season 1/episode 01.mkv"] = new() { Id = 5, SeriesId = 42, EpisodeNumber = 1, SeasonNumber = 1 }
        };
        var seriesById = new Dictionary<int, SonarrSeries>
        {
            [42] = new() { Id = 42, Title = "My Show" }
        };

        await matcher.MatchVideoWithServarrAsync(video, episodesByPath, seriesById);

        Assert.Equal("Sonarr", video.ServarrType);
        Assert.Equal(42, video.SonarrSeriesId);
        Assert.Equal("My Show", video.SonarrSeriesTitle);
        Assert.Equal(5, video.SonarrEpisodeId);
        Assert.Equal(1, video.SonarrEpisodeNumber);
        Assert.Equal(1, video.SonarrSeasonNumber);
        Assert.NotNull(video.ServarrMatchedAt);
    }

    [Fact]
    public async Task MatchVideoWithServarrAsync_WhenTitleFallbackMatches_PopulatesSonarrFields()
    {
        using var dbContext = CreateInMemoryDbContext();
        var sonarr = new Mock<ISonarrService>();
        var radarr = new Mock<IRadarrService>();
        var logger = CreateLogger<VideoServarrMatcherService>();
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();

        sonarr.SetupGet(x => x.IsEnabled).Returns(true);
        sonarr.SetupGet(x => x.IsConnected).Returns(true);
        sonarr.Setup(x => x.GetSeries()).ReturnsAsync(new List<SonarrSeries>());
        radarr.SetupGet(x => x.IsEnabled).Returns(false);
        radarr.SetupGet(x => x.IsConnected).Returns(false);

        var matcher = new VideoServarrMatcherService(dbContext, sonarr.Object, radarr.Object, logger.Object, configuration);
        var video = new VideoAnalysis { Id = 2, FilePath = "/tv/My Show - S01E02.mkv" };

        var episodesByTitle = new Dictionary<string, SonarrEpisode>
        {
            ["my show - s01e02"] = new() { Id = 9, SeriesId = 100, EpisodeNumber = 2, SeasonNumber = 1 }
        };
        var seriesById = new Dictionary<int, SonarrSeries>
        {
            [100] = new() { Id = 100, Title = "My Show" }
        };

        await matcher.MatchVideoWithServarrAsync(video, null, seriesById, null, episodesByTitle);

        Assert.Equal("Sonarr", video.ServarrType);
        Assert.Equal(100, video.SonarrSeriesId);
        Assert.Equal(9, video.SonarrEpisodeId);
    }

    [Fact]
    public async Task MatchVideoWithServarrAsync_WhenNoMatchFound_ClearsExistingServarrFields()
    {
        using var dbContext = CreateInMemoryDbContext();
        var sonarr = new Mock<ISonarrService>();
        var radarr = new Mock<IRadarrService>();
        var logger = CreateLogger<VideoServarrMatcherService>();
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();

        sonarr.SetupGet(x => x.IsEnabled).Returns(true);
        sonarr.SetupGet(x => x.IsConnected).Returns(true);
        sonarr.Setup(x => x.FindEpisodeByPath(It.IsAny<string>())).ReturnsAsync((SonarrEpisode?)null);

        radarr.SetupGet(x => x.IsEnabled).Returns(true);
        radarr.SetupGet(x => x.IsConnected).Returns(true);
        radarr.Setup(x => x.FindMovieByPath(It.IsAny<string>())).ReturnsAsync((RadarrMovie?)null);

        var matcher = new VideoServarrMatcherService(dbContext, sonarr.Object, radarr.Object, logger.Object, configuration);
        var video = new VideoAnalysis
        {
            Id = 3,
            FilePath = "/movies/Unknown.mkv",
            ServarrType = "Radarr",
            RadarrMovieId = 99,
            RadarrMovieTitle = "Old Match",
            ServarrMatchedAt = DateTime.UtcNow
        };

        await matcher.MatchVideoWithServarrAsync(video);

        Assert.Null(video.ServarrType);
        Assert.Null(video.RadarrMovieId);
        Assert.Null(video.RadarrMovieTitle);
        Assert.Null(video.ServarrMatchedAt);
    }

    [Fact]
    public async Task MatchVideoWithServarrAsync_WhenServicesReturnNull_LeavesVideoUnmatched()
    {
        using var dbContext = CreateInMemoryDbContext();
        var sonarr = new Mock<ISonarrService>();
        var radarr = new Mock<IRadarrService>();
        var logger = CreateLogger<VideoServarrMatcherService>();
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();

        sonarr.SetupGet(x => x.IsEnabled).Returns(true);
        sonarr.SetupGet(x => x.IsConnected).Returns(true);
        sonarr.Setup(x => x.FindEpisodeByPath(It.IsAny<string>())).ReturnsAsync((SonarrEpisode?)null);

        radarr.SetupGet(x => x.IsEnabled).Returns(true);
        radarr.SetupGet(x => x.IsConnected).Returns(true);
        radarr.Setup(x => x.FindMovieByPath(It.IsAny<string>())).ReturnsAsync((RadarrMovie?)null);

        var matcher = new VideoServarrMatcherService(dbContext, sonarr.Object, radarr.Object, logger.Object, configuration);
        var video = new VideoAnalysis { Id = 4, FilePath = "/unknown/file.mkv" };

        await matcher.MatchVideoWithServarrAsync(video);

        Assert.Null(video.ServarrType);
        Assert.Null(video.SonarrSeriesId);
        Assert.Null(video.RadarrMovieId);
    }
}
