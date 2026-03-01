using Microsoft.Extensions.Configuration;
using Optimarr.Models;
using Optimarr.Services;

namespace Optimarr.Tests.Services;

public class MediaPropertyRatingServiceTests
{
    [Fact]
    public void CalculateRating_WhenMediaIsHighlyCompatible_ReturnsOptimalScore()
    {
        var service = new MediaPropertyRatingService();
        var video = CreateBaselineVideo();

        var result = service.CalculateRating(video);

        Assert.True(result.CompatibilityRating >= 80);
        Assert.Equal("Optimal", result.OverallScore);
    }

    [Fact]
    public void CalculateRating_WhenCodecsUnsupported_ReturnsPoorScore()
    {
        var service = new MediaPropertyRatingService();
        var video = CreateBaselineVideo();
        video.VideoCodec = "AV1"; // unsupported by default
        video.AudioTracks = new List<AudioTrack> { new() { Codec = "DTS", Channels = 2 } };
        video.IsHDR = false;

        var result = service.CalculateRating(video);

        Assert.True(result.CompatibilityRating < 60);
        Assert.Equal("Poor", result.OverallScore);
    }

    [Fact]
    public void CalculateRating_WhenHdrEnabled_HasHigherScoreThanSdr()
    {
        var service = new MediaPropertyRatingService();
        var hdrVideo = CreateBaselineVideo();
        hdrVideo.IsHDR = true;

        var sdrVideo = CreateBaselineVideo();
        sdrVideo.IsHDR = false;

        var hdrResult = service.CalculateRating(hdrVideo);
        var sdrResult = service.CalculateRating(sdrVideo);

        Assert.True(hdrResult.CompatibilityRating > sdrResult.CompatibilityRating);
        Assert.Contains(sdrResult.Issues, i => i.Contains("SDR content", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void CalculateRating_WhenBitDepthIs10Bit_AppliesBitDepthPenalty()
    {
        var service = new MediaPropertyRatingService();
        var video8Bit = CreateBaselineVideo();
        video8Bit.BitDepth = 8;

        var video10Bit = CreateBaselineVideo();
        video10Bit.BitDepth = 10;

        var result8Bit = service.CalculateRating(video8Bit);
        var result10Bit = service.CalculateRating(video10Bit);

        Assert.True(result10Bit.CompatibilityRating < result8Bit.CompatibilityRating);
        Assert.Contains(result10Bit.Issues, i => i.Contains("10-bit", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void CalculateRating_WhenBitrateAboveThresholdAndBelowCeiling_AppliesHighBitratePenalty()
    {
        var service = new MediaPropertyRatingService();
        var video = CreateBaselineVideo();
        video.FileSize = 100_000_000; // 100 MB
        video.Duration = 10; // ~80 Mbps

        var result = service.CalculateRating(video);

        Assert.Contains(result.Issues, i => i.Contains("High bitrate", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void CalculateRating_WhenBitrateExceedsCorruptionCeiling_DoesNotApplyHighBitratePenalty()
    {
        var service = new MediaPropertyRatingService();
        var video = CreateBaselineVideo();
        video.FileSize = 250_000_000_000; // 250 GB
        video.Duration = 1000; // 2000 Mbps, should be treated as unreasonable

        var result = service.CalculateRating(video);

        Assert.DoesNotContain(result.Issues, i => i.Contains("High bitrate", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void CalculateRating_WhenMp4LacksFastStart_AddsFastStartIssue()
    {
        var service = new MediaPropertyRatingService();
        var video = CreateBaselineVideo();
        video.Container = "MP4";
        video.IsFastStart = false;

        var result = service.CalculateRating(video);

        Assert.Contains(result.Issues, i => i.Contains("fast start", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void CalculateRating_WhenRatingEqualsThreshold_UsesExpectedClassification()
    {
        var config = BuildConfiguration(new Dictionary<string, string?>
        {
            ["MediaPropertySettings:VideoCodecs:H.264"] = "true",
            ["MediaPropertySettings:AudioCodecs:AAC"] = "true",
            ["MediaPropertySettings:Containers:MP4"] = "true",
            ["MediaPropertySettings:BitDepths:8"] = "true",
            ["MediaPropertySettings:RatingWeights:HDR"] = "20",
            ["MediaPropertySettings:RatingWeights:SurroundSound"] = "20",
            ["MediaPropertySettings:RatingWeights:HighBitrate"] = "0",
            ["MediaPropertySettings:RatingWeights:FastStart"] = "0",
            ["MediaPropertySettings:RatingWeights:UnsupportedVideoCodec"] = "0",
            ["MediaPropertySettings:RatingWeights:UnsupportedAudioCodec"] = "0",
            ["MediaPropertySettings:RatingWeights:UnsupportedContainer"] = "0",
            ["MediaPropertySettings:RatingWeights:UnsupportedSubtitleFormat"] = "0",
            ["MediaPropertySettings:RatingWeights:UnsupportedBitDepth"] = "0",
            ["MediaPropertySettings:RatingWeights:IncorrectCodecTag"] = "0",
            ["MediaPropertySettings:RatingWeights:HighBitrateThresholdMbps"] = "99999",
            ["MediaPropertySettings:RatingThresholds:Optimal"] = "80",
            ["MediaPropertySettings:RatingThresholds:Good"] = "60"
        });

        var service = new MediaPropertyRatingService(config);
        var optimalBoundary = CreateBaselineVideo();
        optimalBoundary.IsHDR = false; // exactly -20 => 80

        var goodBoundary = CreateBaselineVideo();
        goodBoundary.IsHDR = false;
        goodBoundary.AudioTracks = new List<AudioTrack> { new() { Codec = "AAC", Channels = 2 } }; // additional -20 => 60

        var optimalResult = service.CalculateRating(optimalBoundary);
        var goodResult = service.CalculateRating(goodBoundary);

        Assert.Equal(80, optimalResult.CompatibilityRating);
        Assert.Equal("Optimal", optimalResult.OverallScore);
        Assert.Equal(60, goodResult.CompatibilityRating);
        Assert.Equal("Good", goodResult.OverallScore);
    }

    private static IConfiguration BuildConfiguration(Dictionary<string, string?> values)
    {
        return new ConfigurationBuilder()
            .AddInMemoryCollection(values)
            .Build();
    }

    private static VideoInfo CreateBaselineVideo()
    {
        return new VideoInfo
        {
            FilePath = "/media/sample.mp4",
            Container = "MP4",
            VideoCodec = "H.264",
            VideoCodecTag = "AVC1",
            IsCodecTagCorrect = true,
            BitDepth = 8,
            IsHDR = true,
            IsFastStart = true,
            FileSize = 50_000_000,
            Duration = 3600,
            AudioTracks = new List<AudioTrack>
            {
                new() { Codec = "AAC", Channels = 6 }
            },
            SubtitleTracks = new List<SubtitleTrack>()
        };
    }
}
