using System.Text.Json;
using Optimarr.Helpers;
using Optimarr.Models;
using Microsoft.Extensions.Options;

namespace Optimarr.Services
{
    public class SonarrService : ServarrServiceBase, ISonarrService
    {
        public SonarrService(IOptionsMonitor<ServarrOptions> optionsMonitor, ILogger<SonarrService> logger)
            : base(optionsMonitor, logger)
        {
        }

        public override string ProviderName => "Sonarr";

        protected override ServarrInstanceOptions? GetInstanceOptions(ServarrOptions options)
        {
            return options.Sonarr;
        }

        public async Task<SonarrEpisode?> GetEpisode(int episodeId)
        {
            if (!IsEnabled || !IsConnected)
                return null;

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync($"/api/v3/episode/{episodeId}");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var episode = JsonSerializer.Deserialize<SonarrEpisode>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return episode;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Sonarr episode {EpisodeId}", episodeId);
            }

            return null;
        }

        public async Task<List<SonarrEpisode>> GetEpisodesBySeries(int seriesId)
        {
            if (!IsEnabled || !IsConnected)
                return new List<SonarrEpisode>();

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync($"/api/v3/episode?seriesId={seriesId}");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var episodes = JsonSerializer.Deserialize<List<SonarrEpisode>>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return episodes ?? new List<SonarrEpisode>();
                }
                else
                {
                    var errorContent = await response.Content.ReadAsStringAsync();
                    _logger.LogWarning("Sonarr API error for episodes (series {SeriesId}): {StatusCode} - {Error}", 
                        seriesId, response.StatusCode, errorContent);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Sonarr episodes for series {SeriesId}", seriesId);
            }

            return new List<SonarrEpisode>();
        }

        public async Task<SonarrEpisodeFile?> GetEpisodeFile(int episodeFileId)
        {
            if (!IsEnabled || !IsConnected)
                return null;

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync($"/api/v3/episodefile/{episodeFileId}");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var episodeFile = JsonSerializer.Deserialize<SonarrEpisodeFile>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return episodeFile;
                }
                else if (response.StatusCode == System.Net.HttpStatusCode.Unauthorized)
                {
                    _logger.LogWarning("Unauthorized access to Sonarr episode file {EpisodeFileId}. Check API key.", episodeFileId);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Sonarr episode file: {EpisodeFileId}", episodeFileId);
            }

            return null;
        }

        /// <summary>
        /// Gets all episode files in bulk (much faster than individual calls)
        /// </summary>
        public async Task<List<SonarrEpisodeFile>> GetAllEpisodeFiles()
        {
            if (!IsEnabled || !IsConnected)
                return new List<SonarrEpisodeFile>();

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync("/api/v3/episodefile");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var episodeFiles = JsonSerializer.Deserialize<List<SonarrEpisodeFile>>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return episodeFiles ?? new List<SonarrEpisodeFile>();
                }
                else if (response.StatusCode == System.Net.HttpStatusCode.Unauthorized)
                {
                    _logger.LogWarning("Unauthorized access to Sonarr episode files. Check API key.");
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting all Sonarr episode files");
            }

            return new List<SonarrEpisodeFile>();
        }

        public async Task<List<SonarrSeries>> GetSeries()
        {
            if (!IsEnabled || !IsConnected)
            {
                _logger.LogWarning("Sonarr service is not enabled or not connected");
                return new List<SonarrSeries>();
            }

            EnsureApiKeyHeader();

            try
            {
                _logger.LogDebug("Requesting Sonarr series list from {Provider}", ProviderName);
                var response = await _httpClient.GetAsync("/api/v3/series");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var series = JsonSerializer.Deserialize<List<SonarrSeries>>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    var count = series?.Count ?? 0;
                    _logger.LogDebug("Successfully retrieved {Count} series from Sonarr", count);
                    return series ?? new List<SonarrSeries>();
                }
                else
                {
                    var errorContent = await response.Content.ReadAsStringAsync();
                    _logger.LogWarning("Sonarr API error for series: {StatusCode} - {Error}", 
                        response.StatusCode, errorContent);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Sonarr series");
            }

            return new List<SonarrSeries>();
        }

        public async Task<SonarrEpisode?> FindEpisodeByPath(string filePath)
        {
            if (!IsEnabled || !IsConnected)
                return null;

            EnsureApiKeyHeader();

            try
            {
                var allEpisodeFiles = await GetAllEpisodeFiles();
                var episodeFilesById = allEpisodeFiles
                    .Where(episodeFile => episodeFile.Id > 0 && !string.IsNullOrEmpty(episodeFile.Path))
                    .ToDictionary(episodeFile => episodeFile.Id, episodeFile => episodeFile);

                // Get all series
                var series = await GetSeries();
                if (series == null || series.Count == 0)
                    return null;

                // Normalize path for comparison
                var normalizedPath = PathNormalizer.Normalize(filePath);

                foreach (var s in series)
                {
                    // Get all episodes for this series
                    var episodes = await GetEpisodesBySeries(s.Id);
                    foreach (var episode in episodes)
                    {
                        if (episode.EpisodeFileId.HasValue)
                        {
                            // Resolve the episode file from the bulk map to avoid per-episode API calls (N+1)
                            episodeFilesById.TryGetValue(episode.EpisodeFileId.Value, out var episodeFile);
                            if (episodeFile != null && !string.IsNullOrEmpty(episodeFile.Path))
                            {
                                var normalizedEpisodePath = PathNormalizer.Normalize(episodeFile.Path);
                                if (normalizedEpisodePath == normalizedPath)
                                {
                                    return episode;
                                }
                            }
                        }
                        // Fallback to episode path if available
                        if (!string.IsNullOrEmpty(episode.Path))
                        {
                            var normalizedEpisodePath = PathNormalizer.Normalize(episode.Path);
                            if (normalizedEpisodePath == normalizedPath)
                            {
                                return episode;
                            }
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error finding Sonarr episode by path: {Path}", filePath);
            }

            return null;
        }

        public async Task<bool> DeleteEpisodeFile(int episodeFileId)
        {
            if (!IsEnabled || !IsConnected)
                return false;

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.DeleteAsync($"/api/v3/episodefile/{episodeFileId}");
                return response.IsSuccessStatusCode;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error deleting Sonarr episode file: {EpisodeFileId}", episodeFileId);
                return false;
            }
        }

        public async Task<bool> TriggerEpisodeSearch(int episodeId)
        {
            if (!IsEnabled || !IsConnected)
                return false;

            EnsureApiKeyHeader();

            try
            {
                var request = new
                {
                    name = "EpisodeSearch",
                    episodeIds = new[] { episodeId }
                };

                var content = new StringContent(JsonSerializer.Serialize(request), 
                    System.Text.Encoding.UTF8, "application/json");
                
                var response = await _httpClient.PostAsync("/api/v3/command", content);
                return response.IsSuccessStatusCode;
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error triggering Sonarr episode search: {EpisodeId}", episodeId);
                return false;
            }
        }

    }

    public class SonarrSeries
    {
        public int Id { get; set; }
        public string Title { get; set; } = string.Empty;
        public string Path { get; set; } = string.Empty;
    }

    public class SonarrEpisode
    {
        public int Id { get; set; }
        public int SeriesId { get; set; }
        public int EpisodeNumber { get; set; }
        public int SeasonNumber { get; set; }
        public string? Path { get; set; }
        public bool HasFile { get; set; }
        public int? EpisodeFileId { get; set; }
    }

    public class SonarrEpisodeFile
    {
        public int Id { get; set; }
        public string? Path { get; set; }
        public int EpisodeId { get; set; }
    }

}

