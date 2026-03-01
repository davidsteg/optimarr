using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging;
using Optimarr.Data;
using Optimarr.Helpers;
using Optimarr.Models;
using Optimarr.Services;
using System.Collections.Concurrent;

namespace Optimarr.Services
{
    public class VideoServarrMatcherService
    {
        private readonly AppDbContext _dbContext;
        private readonly ISonarrService _sonarrService;
        private readonly IRadarrService _radarrService;
        private readonly ILogger<VideoServarrMatcherService> _logger;
        private readonly IConfiguration _configuration;

        public VideoServarrMatcherService(
            AppDbContext dbContext,
            ISonarrService sonarrService,
            IRadarrService radarrService,
            ILogger<VideoServarrMatcherService> logger,
            IConfiguration configuration)
        {
            _dbContext = dbContext;
            _sonarrService = sonarrService;
            _radarrService = radarrService;
            _logger = logger;
            _configuration = configuration;
        }

        /// <summary>
        /// Matches a video file with Sonarr/Radarr data based on file path
        /// </summary>
        public async Task MatchVideoWithServarrAsync(VideoAnalysis video, 
            Dictionary<string, SonarrEpisode>? sonarrEpisodes = null,
            Dictionary<int, SonarrSeries>? sonarrSeries = null,
            Dictionary<string, RadarrMovie>? radarrMovies = null,
            Dictionary<string, SonarrEpisode>? sonarrEpisodesByTitle = null,
            Dictionary<string, RadarrMovie>? radarrMoviesByTitle = null)
        {
            if (string.IsNullOrEmpty(video.FilePath))
            {
                _logger.LogDebug("Skipping video {VideoId} - no file path", video.Id);
                return;
            }

            var normalizedPath = PathNormalizer.Normalize(video.FilePath);
            var normalizedTitle = NormalizeTitle(video.FilePath);

            // Try to match with Sonarr first (using pre-loaded data if available)
            if (_sonarrService.IsEnabled && _sonarrService.IsConnected)
            {
                try
                {
                    SonarrEpisode? episode = null;
                    
                    // First try path-based matching (most reliable)
                    if (sonarrEpisodes != null && sonarrEpisodes.TryGetValue(normalizedPath, out episode))
                    {
                        // Episode found by path
                    }
                    // Fallback to title-based matching if path match fails
                    else if (sonarrEpisodesByTitle != null && !string.IsNullOrEmpty(normalizedTitle) && 
                             sonarrEpisodesByTitle.TryGetValue(normalizedTitle, out episode))
                    {
                        _logger.LogDebug("Matched video {FilePath} with Sonarr episode by title: {Title}", 
                            video.FilePath, normalizedTitle);
                    }
                    else
                    {
                        // Fallback to API call if cache not available
                        episode = await _sonarrService.FindEpisodeByPath(video.FilePath);
                    }

                    if (episode != null)
                    {
                        // Get series information
                        SonarrSeries? series = null;
                        if (sonarrSeries != null && sonarrSeries.TryGetValue(episode.SeriesId, out series))
                        {
                            // Series found in cache
                        }
                        else
                        {
                            // Fallback to API call
                            var allSeries = await _sonarrService.GetSeries();
                            series = allSeries.FirstOrDefault(s => s.Id == episode.SeriesId);
                        }

                        if (series != null)
                        {
                            video.ServarrType = "Sonarr";
                            video.SonarrSeriesId = series.Id;
                            video.SonarrSeriesTitle = series.Title;
                            video.SonarrEpisodeId = episode.Id;
                            video.SonarrEpisodeNumber = episode.EpisodeNumber;
                            video.SonarrSeasonNumber = episode.SeasonNumber;
                            video.ServarrMatchedAt = DateTime.UtcNow;
                            _logger.LogDebug("Matched video {FilePath} with Sonarr series {SeriesTitle} S{Season}E{Episode}",
                                video.FilePath, series.Title, episode.SeasonNumber, episode.EpisodeNumber);
                            return;
                        }
                    }
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "Error matching video with Sonarr: {FilePath}", video.FilePath);
                }
            }

            // Try to match with Radarr (using pre-loaded data if available)
            if (_radarrService.IsEnabled && _radarrService.IsConnected)
            {
                try
                {
                    RadarrMovie? movie = null;
                    
                    // First try path-based matching (most reliable)
                    if (radarrMovies != null && radarrMovies.TryGetValue(normalizedPath, out movie))
                    {
                        // Movie found by path
                    }
                    // Fallback to title-based matching if path match fails
                    else if (radarrMoviesByTitle != null && !string.IsNullOrEmpty(normalizedTitle) && 
                             radarrMoviesByTitle.TryGetValue(normalizedTitle, out movie))
                    {
                        _logger.LogDebug("Matched video {FilePath} with Radarr movie by title: {Title}", 
                            video.FilePath, normalizedTitle);
                    }
                    else
                    {
                        // Fallback to API call if cache not available
                        movie = await _radarrService.FindMovieByPath(video.FilePath);
                    }

                    if (movie != null)
                    {
                        video.ServarrType = "Radarr";
                        video.RadarrMovieId = movie.Id;
                        video.RadarrMovieTitle = movie.Title;
                        // Try to extract year from path or use current year as fallback
                        video.RadarrYear = ExtractYearFromPath(video.FilePath) ?? DateTime.Now.Year;
                        video.ServarrMatchedAt = DateTime.UtcNow;
                        _logger.LogDebug("Matched video {FilePath} with Radarr movie {MovieTitle}",
                            video.FilePath, movie.Title);
                        return;
                    }
                }
                catch (Exception ex)
                {
                    _logger.LogWarning(ex, "Error matching video with Radarr: {FilePath}", video.FilePath);
                }
            }

            // If no match found, clear any existing Servarr data
            if (!string.IsNullOrEmpty(video.ServarrType))
            {
                _logger.LogDebug("Clearing Servarr match for video {FilePath}", video.FilePath);
                video.ServarrType = null;
                video.SonarrSeriesId = null;
                video.SonarrSeriesTitle = null;
                video.SonarrEpisodeId = null;
                video.SonarrEpisodeNumber = null;
                video.SonarrSeasonNumber = null;
                video.RadarrMovieId = null;
                video.RadarrMovieTitle = null;
                video.RadarrYear = null;
                video.ServarrMatchedAt = null;
            }
        }

        /// <summary>
        /// Matches all videos in the database with Sonarr/Radarr data
        /// </summary>
        public async Task<int> MatchAllVideosAsync()
        {
            return await MatchAllVideosAsync(null, null);
        }

        /// <summary>
        /// Matches all videos in the database with Sonarr/Radarr data (with progress tracking)
        /// </summary>
        public async Task<int> MatchAllVideosAsync(VideoMatchingProgressService? progressService, string? matchId)
        {
            _logger.LogInformation("Starting video matching process...");

            var totalVideos = await _dbContext.VideoAnalyses
                .AsNoTracking()
                .Where(v => !string.IsNullOrEmpty(v.FilePath))
                .CountAsync();

            _logger.LogInformation("Found {TotalVideos} videos to process", totalVideos);

            if (totalVideos == 0)
            {
                _logger.LogInformation("No videos to match");
                if (progressService != null && matchId != null)
                {
                    progressService.CompleteProgress(matchId, 0, 0);
                }
                return 0;
            }

            if (progressService != null && matchId != null)
            {
                progressService.UpdateProgress(matchId, 0, totalVideos, 0, 0, "Pre-loading Servarr data...");
            }

            var options = LoadMatchingOptions();
            var memoryBeforePreloadBytes = GC.GetTotalMemory(false);
            var cache = await PreloadServarrDataAsync(options);
            var memoryAfterPreloadBytes = GC.GetTotalMemory(false);
            var preloadDeltaMb = (memoryAfterPreloadBytes - memoryBeforePreloadBytes) / (1024d * 1024d);
            _logger.LogInformation(
                "Servarr preload memory delta: {DeltaMb:F1} MB (before: {BeforeMb:F1} MB, after: {AfterMb:F1} MB)",
                preloadDeltaMb,
                memoryBeforePreloadBytes / (1024d * 1024d),
                memoryAfterPreloadBytes / (1024d * 1024d));

            if (progressService != null && matchId != null)
            {
                progressService.UpdateProgress(matchId, 0, totalVideos, 0, 0, "Starting video matching...");
            }

            var result = await MatchVideosCoreAsync(
                totalVideos,
                options,
                cache,
                skip => _dbContext.VideoAnalyses
                    .Where(v => !string.IsNullOrEmpty(v.FilePath))
                    .OrderBy(v => v.Id)
                    .Skip(skip)
                    .Take(options.VideoBatchSize),
                progressService,
                matchId,
                "all videos");

            if (progressService != null && matchId != null)
            {
                progressService.CompleteProgress(matchId, result.MatchedCount, result.ErrorCount);
            }

            return result.MatchedCount;
        }

        /// <summary>
        /// Matches videos for a specific library path
        /// </summary>
        public async Task<int> MatchVideosForLibraryPathAsync(int libraryPathId)
        {
            var libraryPath = await _dbContext.LibraryPaths
                .AsNoTracking()
                .FirstOrDefaultAsync(lp => lp.Id == libraryPathId);
            if (libraryPath == null)
            {
                _logger.LogWarning("Library path {LibraryPathId} not found", libraryPathId);
                return 0;
            }

            _logger.LogInformation("Starting video matching for library path: {Path}", libraryPath.Path);

            // Use database-friendly query (avoid PathNormalizer.Normalize in LINQ)
            var libraryPathLower = libraryPath.Path.Replace('\\', '/').ToLowerInvariant();
            var totalVideos = await _dbContext.VideoAnalyses
                .AsNoTracking()
                .Where(v => !string.IsNullOrEmpty(v.FilePath) &&
                           (v.FilePath.Replace('\\', '/').ToLowerInvariant().StartsWith(libraryPathLower) ||
                            v.FilePath.Replace('\\', '/').ToLowerInvariant().StartsWith(libraryPathLower + "/")))
                .CountAsync();

            _logger.LogInformation("Found {TotalVideos} videos in library path to process", totalVideos);

            if (totalVideos == 0)
            {
                _logger.LogInformation("No videos found in library path");
                return 0;
            }

            var options = LoadMatchingOptions();
            var memoryBeforePreloadBytes = GC.GetTotalMemory(false);
            var cache = await PreloadServarrDataAsync(options);
            var memoryAfterPreloadBytes = GC.GetTotalMemory(false);
            var preloadDeltaMb = (memoryAfterPreloadBytes - memoryBeforePreloadBytes) / (1024d * 1024d);
            _logger.LogInformation(
                "Servarr preload memory delta: {DeltaMb:F1} MB (before: {BeforeMb:F1} MB, after: {AfterMb:F1} MB)",
                preloadDeltaMb,
                memoryBeforePreloadBytes / (1024d * 1024d),
                memoryAfterPreloadBytes / (1024d * 1024d));

            var result = await MatchVideosCoreAsync(
                totalVideos,
                options,
                cache,
                skip => _dbContext.VideoAnalyses
                    .Where(v => !string.IsNullOrEmpty(v.FilePath) &&
                               (v.FilePath.Replace('\\', '/').ToLowerInvariant().StartsWith(libraryPathLower) ||
                                v.FilePath.Replace('\\', '/').ToLowerInvariant().StartsWith(libraryPathLower + "/")))
                    .OrderBy(v => v.Id)
                    .Skip(skip)
                    .Take(options.VideoBatchSize),
                null,
                null,
                $"library path {libraryPath.Path}");

            return result.MatchedCount;
        }

        private MatchingOptions LoadMatchingOptions()
        {
            return new MatchingOptions
            {
                SeriesConcurrency = _configuration.GetValue<int>("MatchingOptions:SeriesConcurrency", 20),
                MovieConcurrency = _configuration.GetValue<int>("MatchingOptions:MovieConcurrency", 50),
                VideoBatchSize = _configuration.GetValue<int>("MatchingOptions:VideoBatchSize", 2000),
                SaveBatchSize = _configuration.GetValue<int>("MatchingOptions:SaveBatchSize", 200),
                MaxMatchConcurrency = _configuration.GetValue<int>("MatchingOptions:MaxMatchConcurrency", 50)
            };
        }

        private async Task<ServarrMatchCache> PreloadServarrDataAsync(MatchingOptions options)
        {
            _logger.LogInformation("Pre-loading Servarr data...");
            var cache = new ServarrMatchCache();

            await PreloadSonarrDataAsync(cache, options.SeriesConcurrency);
            await PreloadRadarrDataAsync(cache, options.MovieConcurrency);

            return cache;
        }

        private async Task PreloadSonarrDataAsync(ServarrMatchCache cache, int seriesConcurrency)
        {
            if (!_sonarrService.IsEnabled || !_sonarrService.IsConnected)
            {
                return;
            }

            try
            {
                _logger.LogInformation("Loading Sonarr series and episodes...");
                var seriesTask = _sonarrService.GetSeries();
                var episodeFilesTask = _sonarrService.GetAllEpisodeFiles();
                await Task.WhenAll(seriesTask, episodeFilesTask);

                var allSeries = await seriesTask;
                var allEpisodeFiles = await episodeFilesTask;
                cache.SonarrSeries = allSeries.ToDictionary(s => s.Id, s => s);

                var episodeFilesById = allEpisodeFiles
                    .Where(ef => !string.IsNullOrEmpty(ef.Path))
                    .GroupBy(ef => ef.Id)
                    .ToDictionary(g => g.Key, g => g.First());

                var sonarrEpisodesConcurrent = new ConcurrentDictionary<string, SonarrEpisode>();
                var sonarrEpisodesByTitleConcurrent = new ConcurrentDictionary<string, SonarrEpisode>();
                var seriesSemaphore = new SemaphoreSlim(seriesConcurrency, seriesConcurrency);

                var tasks = allSeries.Select(async series =>
                {
                    await seriesSemaphore.WaitAsync();
                    try
                    {
                        var episodes = await _sonarrService.GetEpisodesBySeries(series.Id);
                        var episodeTasks = episodes.Select(episode => Task.Run(() =>
                        {
                            string? path = null;
                            if (!string.IsNullOrEmpty(episode.Path))
                            {
                                path = episode.Path;
                            }
                            else if (episode.EpisodeFileId.HasValue &&
                                     episodeFilesById.TryGetValue(episode.EpisodeFileId.Value, out var episodeFile))
                            {
                                path = episodeFile.Path;
                            }

                            if (string.IsNullOrEmpty(path))
                            {
                                return;
                            }

                            var normalizedPath = PathNormalizer.Normalize(path);
                            sonarrEpisodesConcurrent.TryAdd(normalizedPath, episode);

                            var normalizedTitle = NormalizeTitle(path);
                            if (!string.IsNullOrEmpty(normalizedTitle))
                            {
                                sonarrEpisodesByTitleConcurrent.TryAdd(normalizedTitle, episode);
                            }
                        }));

                        await Task.WhenAll(episodeTasks);
                    }
                    catch (Exception ex)
                    {
                        _logger.LogWarning(ex, "Error loading episodes for series {SeriesId}: {Error}", series.Id, ex.Message);
                    }
                    finally
                    {
                        seriesSemaphore.Release();
                    }
                });

                await Task.WhenAll(tasks);

                cache.SonarrEpisodes = sonarrEpisodesConcurrent.ToDictionary(kvp => kvp.Key, kvp => kvp.Value);
                cache.SonarrEpisodesByTitle = sonarrEpisodesByTitleConcurrent.ToDictionary(kvp => kvp.Key, kvp => kvp.Value);

                _logger.LogInformation(
                    "Loaded {SeriesCount} Sonarr series, {EpisodeCount} episode paths, {TitleCount} episode titles",
                    allSeries.Count,
                    cache.SonarrEpisodes.Count,
                    cache.SonarrEpisodesByTitle.Count);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error pre-loading Sonarr data");
            }
        }

        private async Task PreloadRadarrDataAsync(ServarrMatchCache cache, int movieConcurrency)
        {
            if (!_radarrService.IsEnabled || !_radarrService.IsConnected)
            {
                return;
            }

            try
            {
                _logger.LogInformation("Loading Radarr movies...");
                var movies = await _radarrService.GetMovies();
                var radarrMoviesConcurrent = new ConcurrentDictionary<string, RadarrMovie>();
                var radarrMoviesByTitleConcurrent = new ConcurrentDictionary<string, RadarrMovie>();
                var movieSemaphore = new SemaphoreSlim(movieConcurrency, movieConcurrency);

                var movieTasks = movies.Select(async movie =>
                {
                    await movieSemaphore.WaitAsync();
                    try
                    {
                        if (movie.MovieFile == null || string.IsNullOrEmpty(movie.MovieFile.Path))
                        {
                            return;
                        }

                        var normalizedPath = PathNormalizer.Normalize(movie.MovieFile.Path);
                        radarrMoviesConcurrent.TryAdd(normalizedPath, movie);

                        var normalizedTitle = NormalizeTitle(movie.MovieFile.Path);
                        if (!string.IsNullOrEmpty(normalizedTitle))
                        {
                            radarrMoviesByTitleConcurrent.TryAdd(normalizedTitle, movie);
                        }
                    }
                    finally
                    {
                        movieSemaphore.Release();
                    }
                });

                await Task.WhenAll(movieTasks);
                cache.RadarrMovies = radarrMoviesConcurrent.ToDictionary(kvp => kvp.Key, kvp => kvp.Value);
                cache.RadarrMoviesByTitle = radarrMoviesByTitleConcurrent.ToDictionary(kvp => kvp.Key, kvp => kvp.Value);

                _logger.LogInformation(
                    "Loaded {MovieCount} movie paths and {TitleCount} movie titles from Radarr",
                    cache.RadarrMovies.Count,
                    cache.RadarrMoviesByTitle.Count);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error pre-loading Radarr data");
            }
        }

        private async Task<MatchRunResult> MatchVideosCoreAsync(
            int totalVideos,
            MatchingOptions options,
            ServarrMatchCache cache,
            Func<int, IQueryable<VideoAnalysis>> getBatchQuery,
            VideoMatchingProgressService? progressService,
            string? matchId,
            string scopeLabel)
        {
            _logger.LogInformation("Starting to match videos for {Scope}", scopeLabel);

            var matchedCount = 0;
            var processedCount = 0;
            var errorCount = 0;
            const int logInterval = 500;
            const int progressUpdateInterval = 50;
            var processedLock = new object();
            var matchingSemaphore = new SemaphoreSlim(options.MaxMatchConcurrency, options.MaxMatchConcurrency);

            void UpdateProgressIfNeeded(string? currentItem = null)
            {
                if (progressService == null || matchId == null)
                {
                    return;
                }

                if (processedCount == 1 || processedCount % progressUpdateInterval == 0 || processedCount == totalVideos)
                {
                    progressService.UpdateProgress(matchId, processedCount, totalVideos, matchedCount, errorCount, currentItem);
                }
            }

            var skip = 0;
            while (skip < totalVideos)
            {
                var videoBatch = await getBatchQuery(skip).ToListAsync();
                if (videoBatch.Count == 0)
                {
                    break;
                }

                _logger.LogInformation(
                    "Processing batch: {Skip} to {End} of {Total} videos for {Scope}",
                    skip + 1,
                    skip + videoBatch.Count,
                    totalVideos,
                    scopeLabel);

                await MatchVideoBatchAsync(
                    videoBatch,
                    cache,
                    matchingSemaphore,
                    onProcessed: video =>
                    {
                        lock (processedLock)
                        {
                            processedCount++;
                            UpdateProgressIfNeeded(System.IO.Path.GetFileName(video.FilePath));

                            if (processedCount % logInterval == 0)
                            {
                                var percent = totalVideos > 0 ? (processedCount * 100 / totalVideos) : 0;
                                _logger.LogInformation(
                                    "Matching progress for {Scope}: {Processed}/{Total} ({Percent}%), {Matched} matched",
                                    scopeLabel,
                                    processedCount,
                                    totalVideos,
                                    percent,
                                    matchedCount);
                            }
                        }
                    },
                    onNewMatch: () =>
                    {
                        lock (processedLock)
                        {
                            matchedCount++;
                        }
                    },
                    onError: ex =>
                    {
                        lock (processedLock)
                        {
                            errorCount++;
                        }

                        _logger.LogWarning(ex, "Error while matching batch for {Scope}", scopeLabel);
                    });

                await _dbContext.SaveChangesAsync();

                lock (processedLock)
                {
                    UpdateProgressIfNeeded();

                    if (processedCount % options.SaveBatchSize < options.VideoBatchSize ||
                        processedCount % logInterval < options.VideoBatchSize)
                    {
                        _logger.LogInformation(
                            "Progress for {Scope}: {Processed}/{Total} processed, {Matched} matched",
                            scopeLabel,
                            processedCount,
                            totalVideos,
                            matchedCount);
                    }
                }

                skip += options.VideoBatchSize;
            }

            if (_dbContext.ChangeTracker.HasChanges())
            {
                await _dbContext.SaveChangesAsync();
            }
            _logger.LogInformation(
                "Video matching completed for {Scope}: {Processed}/{Total} processed, {Matched} matched, {Errors} errors",
                scopeLabel,
                processedCount,
                totalVideos,
                matchedCount,
                errorCount);

            return new MatchRunResult
            {
                MatchedCount = matchedCount,
                ProcessedCount = processedCount,
                ErrorCount = errorCount
            };
        }

        private async Task MatchVideoBatchAsync(
            List<VideoAnalysis> videos,
            ServarrMatchCache cache,
            SemaphoreSlim matchingSemaphore,
            Action<VideoAnalysis> onProcessed,
            Action onNewMatch,
            Action<Exception> onError)
        {
            var videoPaths = new Dictionary<VideoAnalysis, string>();
            foreach (var video in videos)
            {
                if (!string.IsNullOrEmpty(video.FilePath))
                {
                    videoPaths[video] = PathNormalizer.Normalize(video.FilePath);
                }
            }

            var batchTasks = videos.Select(async video =>
            {
                await matchingSemaphore.WaitAsync();
                try
                {
                    if (!videoPaths.TryGetValue(video, out var normalizedPath))
                    {
                        onProcessed(video);
                        return;
                    }

                    var alreadyMatched = false;
                    if (!string.IsNullOrEmpty(video.ServarrType))
                    {
                        if (video.ServarrType == "Sonarr" && cache.SonarrEpisodes != null)
                        {
                            alreadyMatched = cache.SonarrEpisodes.ContainsKey(normalizedPath);
                        }
                        else if (video.ServarrType == "Radarr" && cache.RadarrMovies != null)
                        {
                            alreadyMatched = cache.RadarrMovies.ContainsKey(normalizedPath);
                        }
                    }

                    var hadMatch = !string.IsNullOrEmpty(video.ServarrType);
                    if (!alreadyMatched)
                    {
                        await MatchVideoWithServarrAsync(
                            video,
                            cache.SonarrEpisodes,
                            cache.SonarrSeries,
                            cache.RadarrMovies,
                            cache.SonarrEpisodesByTitle,
                            cache.RadarrMoviesByTitle);
                    }

                    if (!hadMatch && !string.IsNullOrEmpty(video.ServarrType))
                    {
                        onNewMatch();
                    }

                    onProcessed(video);
                }
                catch (Exception ex)
                {
                    onError(ex);
                }
                finally
                {
                    matchingSemaphore.Release();
                }
            });

            await Task.WhenAll(batchTasks);
        }

        private sealed class MatchingOptions
        {
            public int SeriesConcurrency { get; init; }
            public int MovieConcurrency { get; init; }
            public int VideoBatchSize { get; init; }
            public int SaveBatchSize { get; init; }
            public int MaxMatchConcurrency { get; init; }
        }

        private sealed class ServarrMatchCache
        {
            public Dictionary<string, SonarrEpisode>? SonarrEpisodes { get; set; }
            public Dictionary<string, SonarrEpisode>? SonarrEpisodesByTitle { get; set; }
            public Dictionary<int, SonarrSeries>? SonarrSeries { get; set; }
            public Dictionary<string, RadarrMovie>? RadarrMovies { get; set; }
            public Dictionary<string, RadarrMovie>? RadarrMoviesByTitle { get; set; }
        }

        private sealed class MatchRunResult
        {
            public int MatchedCount { get; init; }
            public int ProcessedCount { get; init; }
            public int ErrorCount { get; init; }
        }

        /// <summary>
        /// Extracts and normalizes the title from a file path (filename without extension)
        /// </summary>
        private string NormalizeTitle(string path)
        {
            if (string.IsNullOrEmpty(path)) return string.Empty;
            
            try
            {
                // Get filename without extension
                var fileName = System.IO.Path.GetFileName(path);
                if (string.IsNullOrEmpty(fileName)) return string.Empty;
                
                // Remove extension
                var title = System.IO.Path.GetFileNameWithoutExtension(fileName);
                if (string.IsNullOrEmpty(title)) return string.Empty;
                
                // Normalize: lowercase, trim whitespace
                return title.Trim().ToLowerInvariant();
            }
            catch (Exception ex)
            {
                _logger.LogDebug(ex, "Failed to normalize title from path: {Path}", path);
                return string.Empty;
            }
        }

        private int? ExtractYearFromPath(string path)
        {
            // Try to extract year from path (common patterns: (2023), [2023], 2023, etc.)
            var yearPattern = new System.Text.RegularExpressions.Regex(@"\b(19|20)\d{2}\b");
            var match = yearPattern.Match(path);
            if (match.Success && int.TryParse(match.Value, out var year))
            {
                if (year >= 1900 && year <= DateTime.Now.Year + 1)
                {
                    return year;
                }
            }
            return null;
        }
    }
}
