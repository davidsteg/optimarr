using Optimarr.Helpers;

namespace Optimarr.Tests.Helpers;

public class PathNormalizerTests
{
    public static IEnumerable<object?[]> NormalizeCases()
    {
        yield return new object?[] { @"C:\Media\TV\Show\Episode 01.mkv", "c:/media/tv/show/episode 01.mkv" };
        yield return new object?[] { "/mnt/media/tv/show/Episode 01.mkv", "/mnt/media/tv/show/episode 01.mkv" };
        yield return new object?[] { @"\\NAS\Share\Movies\Film.mkv", "//nas/share/movies/film.mkv" };
        yield return new object?[] { "/mnt/media/tv/", "/mnt/media/tv" };
        yield return new object?[] { "//mnt//media//tv//Show//", "//mnt//media//tv//show" };
        yield return new object?[] { "/mnt/media/My%20Show/Episode%201.mkv", "/mnt/media/my show/episode 1.mkv" };
        yield return new object?[] { string.Empty, string.Empty };
        yield return new object?[] { null, string.Empty };
    }

    [Theory]
    [MemberData(nameof(NormalizeCases))]
    public void Normalize_GivenPathVariants_ReturnsCanonicalPath(string? input, string expected)
    {
        var normalized = PathNormalizer.Normalize(input);
        Assert.Equal(expected, normalized);
    }
}
