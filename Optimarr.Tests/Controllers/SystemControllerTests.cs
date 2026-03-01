using System.Text.Json;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Optimarr.Controllers;

namespace Optimarr.Tests.Controllers;

public class SystemControllerTests
{
    [Fact]
    public async Task GetHealth_Endpoint_ReturnsHealthyPayload()
    {
        await using var server = new TestServer(new WebHostBuilder()
            .ConfigureServices(services =>
            {
                services.AddLogging();
                services.AddControllers().AddApplicationPart(typeof(SystemController).Assembly);
            })
            .Configure(app =>
            {
                app.UseRouting();
                app.UseEndpoints(endpoints => endpoints.MapControllers());
            }));

        var client = server.CreateClient();
        var response = await client.GetAsync("/api/system/health");

        response.EnsureSuccessStatusCode();
        var content = await response.Content.ReadAsStringAsync();
        using var doc = JsonDocument.Parse(content);
        Assert.Equal("healthy", doc.RootElement.GetProperty("status").GetString());
        Assert.True(doc.RootElement.TryGetProperty("version", out _));
    }

    [Fact]
    public void GetHealth_DirectControllerCall_ReturnsOk()
    {
        var controller = new SystemController(new LoggerFactory().CreateLogger<SystemController>());

        var result = controller.GetHealth();

        var ok = Assert.IsType<Microsoft.AspNetCore.Mvc.OkObjectResult>(result.Result);
        Assert.NotNull(ok.Value);
    }
}
