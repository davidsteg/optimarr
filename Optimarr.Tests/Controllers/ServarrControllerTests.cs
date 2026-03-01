using System.Text.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;
using Moq;
using Optimarr.Controllers;
using Optimarr.Data;
using Optimarr.Models;
using Optimarr.Services;
using Optimarr.Tests.Common;

namespace Optimarr.Tests.Controllers;

public class ServarrControllerTests : TestBase
{
    [Fact]
    public async Task SaveAndGetSonarrSettings_MasksApiKeyInResponse()
    {
        var tempRoot = CreateTempDirectory();
        var configDir = Path.Combine(tempRoot, "config");
        Directory.CreateDirectory(configDir);
        var appsettingsPath = Path.Combine(configDir, "appsettings.json");
        await File.WriteAllTextAsync(appsettingsPath, "{}");

        var configuration = new ConfigurationBuilder()
            .SetBasePath(tempRoot)
            .AddJsonFile("config/appsettings.json", optional: false, reloadOnChange: false)
            .Build();

        var controller = CreateController(configuration, tempRoot);

        var saveResult = await controller.SaveSonarrSettings(new ServarrSettingsRequest
        {
            BaseUrl = "http://localhost:8989",
            ApiKey = "abcdef123456",
            Enabled = true
        });

        Assert.IsType<OkObjectResult>(saveResult);

        var getResult = controller.GetSonarrSettings();
        var ok = Assert.IsType<OkObjectResult>(getResult);

        var json = JsonSerializer.Serialize(ok.Value);
        using var doc = JsonDocument.Parse(json);
        var apiKey = doc.RootElement.GetProperty("apiKey").GetString();
        var baseUrl = doc.RootElement.GetProperty("baseUrl").GetString();

        Assert.NotNull(apiKey);
        Assert.NotEqual("abcdef123456", apiKey);
        Assert.EndsWith("3456", apiKey, StringComparison.Ordinal);
        Assert.Equal("http://localhost:8989", baseUrl);
    }

    [Theory]
    [InlineData("file:///etc/passwd")]
    [InlineData("ftp://localhost")]
    [InlineData("not-a-url")]
    public async Task TestSonarrConnection_WhenBaseUrlInvalid_ReturnsBadRequest(string baseUrl)
    {
        var configuration = new ConfigurationBuilder().AddInMemoryCollection().Build();
        var controller = CreateController(configuration, CreateTempDirectory());

        var result = await controller.TestSonarrConnection(new TestConnectionRequest
        {
            BaseUrl = baseUrl,
            ApiKey = "abc"
        });

        Assert.IsType<BadRequestObjectResult>(result);
    }

    private static ServarrController CreateController(IConfiguration configuration, string contentRootPath)
    {
        var optionsMonitor = new TestOptionsMonitor<ServarrOptions>(new ServarrOptions
        {
            Sonarr = new ServarrInstanceOptions { Enabled = false },
            Radarr = new ServarrInstanceOptions { Enabled = false }
        });

        var sonarrService = new SonarrService(optionsMonitor, new Mock<Microsoft.Extensions.Logging.ILogger<SonarrService>>().Object);
        var radarrService = new RadarrService(optionsMonitor, new Mock<Microsoft.Extensions.Logging.ILogger<RadarrService>>().Object);

        var dbOptions = new DbContextOptionsBuilder<AppDbContext>()
            .UseInMemoryDatabase(Guid.NewGuid().ToString("N"))
            .Options;
        var dbContext = new AppDbContext(dbOptions);

        var syncService = new ServarrSyncService(
            dbContext,
            radarrService,
            sonarrService,
            new Mock<Microsoft.Extensions.Logging.ILogger<ServarrSyncService>>().Object,
            configuration);

        var matcherService = new VideoServarrMatcherService(
            dbContext,
            sonarrService,
            radarrService,
            new Mock<Microsoft.Extensions.Logging.ILogger<VideoServarrMatcherService>>().Object,
            configuration);

        var progressService = new VideoMatchingProgressService(
            new Mock<Microsoft.Extensions.Logging.ILogger<VideoMatchingProgressService>>().Object);

        var env = new Mock<IWebHostEnvironment>();
        env.SetupGet(x => x.ContentRootPath).Returns(contentRootPath);

        var lifetime = new Mock<IHostApplicationLifetime>();
        lifetime.SetupGet(x => x.ApplicationStopping).Returns(CancellationToken.None);

        return new ServarrController(
            sonarrService,
            radarrService,
            syncService,
            matcherService,
            progressService,
            new Mock<Microsoft.Extensions.Logging.ILogger<ServarrController>>().Object,
            configuration,
            env.Object,
            Mock.Of<IServiceProvider>(),
            lifetime.Object);
    }

    private static string CreateTempDirectory()
    {
        var path = Path.Combine(Path.GetTempPath(), "optimarr-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(path);
        return path;
    }
}
