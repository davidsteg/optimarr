using System.Text.Json;
using Optimarr.Helpers;
using Optimarr.Models;
using Microsoft.Extensions.Options;

namespace Optimarr.Services
{
    public class RadarrService : ServarrServiceBase, IRadarrService
    {
        public RadarrService(IOptionsMonitor<ServarrOptions> optionsMonitor, ILogger<RadarrService> logger)
            : base(optionsMonitor, logger)
        {
        }

        public override string ProviderName => "Radarr";

        protected override ServarrInstanceOptions? GetInstanceOptions(ServarrOptions options)
        {
            return options.Radarr;
        }

        public async Task<RadarrMovie?> GetMovie(int movieId)
        {
            if (!IsEnabled || !IsConnected)
                return null;

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync($"/api/v3/movie/{movieId}");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var movie = JsonSerializer.Deserialize<RadarrMovie>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return movie;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Radarr movie");
            }

            return null;
        }

        public async Task<List<RadarrMovie>> GetMovies()
        {
            if (!IsEnabled || !IsConnected)
            {
                _logger.LogWarning("Radarr service is not enabled or not connected");
                return new List<RadarrMovie>();
            }

            EnsureApiKeyHeader();

            try
            {
                _logger.LogDebug("Requesting Radarr movies list from {Provider}", ProviderName);
                var response = await _httpClient.GetAsync("/api/v3/movie");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var movies = JsonSerializer.Deserialize<List<RadarrMovie>>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    var count = movies?.Count ?? 0;
                    _logger.LogDebug("Successfully retrieved {Count} movies from Radarr", count);
                    return movies ?? new List<RadarrMovie>();
                }
                else
                {
                    var errorContent = await response.Content.ReadAsStringAsync();
                    _logger.LogWarning("Radarr API error for movies: {StatusCode} - {Error}", 
                        response.StatusCode, errorContent);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Radarr movies");
            }

            return new List<RadarrMovie>();
        }

        public async Task<RadarrMovie?> FindMovieByPath(string filePath)
        {
            if (!IsEnabled || !IsConnected)
                return null;

            EnsureApiKeyHeader();

            try
            {
                var movies = await GetMovies();
                if (movies == null || movies.Count == 0)
                    return null;

                // Normalize path for comparison
                var normalizedPath = PathNormalizer.Normalize(filePath);

                foreach (var movie in movies)
                {
                    if (movie.MovieFile != null && !string.IsNullOrEmpty(movie.MovieFile.Path))
                    {
                        var normalizedMoviePath = PathNormalizer.Normalize(movie.MovieFile.Path);
                        if (normalizedMoviePath == normalizedPath)
                        {
                            return movie;
                        }
                    }
                    // Also check the movie path itself
                    if (!string.IsNullOrEmpty(movie.Path))
                    {
                        var normalizedMoviePath = PathNormalizer.Normalize(movie.Path);
                        if (normalizedPath.StartsWith(normalizedMoviePath, StringComparison.OrdinalIgnoreCase))
                        {
                            return movie;
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error finding Radarr movie by path: {Path}", filePath);
            }

            return null;
        }

        public async Task<bool> DeleteMovieFile(int movieFileId)
        {
            if (!IsEnabled || !IsConnected)
                return false;

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.DeleteAsync($"/api/v3/movieFile/{movieFileId}");
                return response.IsSuccessStatusCode;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error deleting Radarr movie file: {MovieFileId}", movieFileId);
                return false;
            }
        }

        public async Task<bool> TriggerMovieSearch(int movieId)
        {
            if (!IsEnabled || !IsConnected)
                return false;

            EnsureApiKeyHeader();

            try
            {
                var request = new
                {
                    name = "MoviesSearch",
                    movieIds = new[] { movieId }
                };

                var content = new StringContent(JsonSerializer.Serialize(request), 
                    System.Text.Encoding.UTF8, "application/json");
                
                var response = await _httpClient.PostAsync("/api/v3/command", content);
                return response.IsSuccessStatusCode;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error triggering Radarr movie search: {MovieId}", movieId);
                return false;
            }
        }

    }

    public class RadarrMovie
    {
        public int Id { get; set; }
        public string Title { get; set; } = string.Empty;
        public string? Path { get; set; }
        public bool HasFile { get; set; }
        public RadarrMovieFile? MovieFile { get; set; }
    }

    public class RadarrMovieFile
    {
        public int Id { get; set; }
        public string? Path { get; set; }
    }

}

