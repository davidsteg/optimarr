using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Moq;
using Optimarr.Data;

namespace Optimarr.Tests.Common;

public abstract class TestBase
{
    protected AppDbContext CreateInMemoryDbContext()
    {
        var options = new DbContextOptionsBuilder<AppDbContext>()
            .UseInMemoryDatabase(Guid.NewGuid().ToString("N"))
            .Options;

        return new AppDbContext(options);
    }

    protected static Mock<ILogger<T>> CreateLogger<T>() where T : class
    {
        return new Mock<ILogger<T>>();
    }
}
