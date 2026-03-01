using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;
using Optimarr.Services;

namespace Optimarr.Controllers
{
    [ApiController]
    [Route("api/[controller]")]
    public class ServarrController : ControllerBase
    {
        private static class ConfigKeys
        {
            public const string Servarr = "Servarr";
            public const string Sonarr = "Sonarr";
            public const string Radarr = "Radarr";
            public const string PathMappings = "PathMappings";
            public const string SonarrBaseUrl = "Servarr:Sonarr:BaseUrl";
            public const string SonarrApiKey = "Servarr:Sonarr:ApiKey";
            public const string SonarrEnabled = "Servarr:Sonarr:Enabled";
            public const string SonarrPathMappings = "Servarr:Sonarr:PathMappings";
            public const string RadarrBaseUrl = "Servarr:Radarr:BaseUrl";
            public const string RadarrApiKey = "Servarr:Radarr:ApiKey";
            public const string RadarrEnabled = "Servarr:Radarr:Enabled";
            public const string RadarrPathMappings = "Servarr:Radarr:PathMappings";
        }

        private readonly SonarrService _sonarrService;
        private readonly RadarrService _radarrService;
        private readonly ServarrSyncService _syncService;
        private readonly VideoServarrMatcherService _matcherService;
        private readonly VideoMatchingProgressService _progressService;
        private readonly ILogger<ServarrController> _logger;
        private readonly IConfiguration _configuration;
        private readonly IWebHostEnvironment _environment;
        private readonly IServiceProvider _serviceProvider;
        private readonly IHostApplicationLifetime _lifetime;

        public ServarrController(
            SonarrService sonarrService,
            RadarrService radarrService,
            ServarrSyncService syncService,
            VideoServarrMatcherService matcherService,
            VideoMatchingProgressService progressService,
            ILogger<ServarrController> logger,
            IConfiguration configuration,
            IWebHostEnvironment environment,
            IServiceProvider serviceProvider,
            IHostApplicationLifetime lifetime)
        {
            _sonarrService = sonarrService;
            _radarrService = radarrService;
            _syncService = syncService;
            _matcherService = matcherService;
            _progressService = progressService;
            _logger = logger;
            _configuration = configuration;
            _environment = environment;
            _serviceProvider = serviceProvider;
            _lifetime = lifetime;
        }

        /// <summary>
        /// Returns Sonarr and Radarr enablement, connectivity, and version status.
        /// </summary>
        [HttpGet("status")]
        public async Task<ActionResult<ServarrStatusResponse>> GetStatus()
        {
            _logger.LogDebug("Servarr status check requested");
            return Ok(new ServarrStatusResponse
            {
                Sonarr = new ServiceStatus
                {
                    Enabled = _sonarrService.IsEnabled,
                    Connected = _sonarrService.IsConnected,
                    Version = await _sonarrService.GetVersionAsync()
                },
                Radarr = new ServiceStatus
                {
                    Enabled = _radarrService.IsEnabled,
                    Connected = _radarrService.IsConnected,
                    Version = await _radarrService.GetVersionAsync()
                }
            });
        }

        /// <summary>
        /// Triggers analysis preparation for all episodes in a Sonarr series.
        /// </summary>
        [HttpPost("sonarr/analyze-series/{seriesId}")]
        public async Task<ActionResult> AnalyzeSonarrSeries(int seriesId)
        {
            _logger.LogInformation("Sonarr series analysis requested for series ID: {SeriesId}", seriesId);
            try
            {
                if (!_sonarrService.IsEnabled)
                {
                    _logger.LogWarning("Sonarr analysis requested but Sonarr is not enabled");
                    return BadRequest(new { error = "Sonarr is not enabled" });
                }

                var episodes = await _sonarrService.GetEpisodesBySeries(seriesId);
                _logger.LogInformation("Retrieved {Count} episodes for series {SeriesId}", episodes.Count, seriesId);
                var results = new List<object>();

                foreach (var episode in episodes)
                {
                    if (!string.IsNullOrEmpty(episode.Path) && System.IO.File.Exists(episode.Path))
                    {
                        // Analyze each episode file
                        // This would integrate with the video analyzer
                        results.Add(new
                        {
                            EpisodeId = episode.Id,
                            EpisodeNumber = episode.EpisodeNumber,
                            Path = episode.Path,
                            Status = "Analyzed"
                        });
                    }
                }

                return Ok(results);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error analyzing Sonarr series");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Triggers analysis preparation for a Radarr movie by ID.
        /// </summary>
        [HttpPost("radarr/analyze-movie/{movieId}")]
        public async Task<ActionResult> AnalyzeRadarrMovie(int movieId)
        {
            _logger.LogInformation("Radarr movie analysis requested for movie ID: {MovieId}", movieId);
            try
            {
                if (!_radarrService.IsEnabled)
                {
                    _logger.LogWarning("Radarr analysis requested but Radarr is not enabled");
                    return BadRequest(new { error = "Radarr is not enabled" });
                }

                var movie = await _radarrService.GetMovie(movieId);
                if (movie == null)
                {
                    return NotFound(new { error = "Movie not found" });
                }

                if (!string.IsNullOrEmpty(movie.Path) && System.IO.File.Exists(movie.Path))
                {
                    // Analyze movie file
                    return Ok(new
                    {
                        MovieId = movie.Id,
                        Title = movie.Title,
                        Path = movie.Path,
                        Status = "Ready for analysis"
                    });
                }

                return BadRequest(new { error = "Movie file not found" });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error analyzing Radarr movie");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Tests Sonarr connectivity using provided URL and API key values.
        /// </summary>
        [HttpPost("sonarr/test")]
        public async Task<ActionResult> TestSonarrConnection([FromBody] TestConnectionRequest request)
        {
            if (string.IsNullOrWhiteSpace(request.BaseUrl))
                return BadRequest(new { error = "BaseUrl is required." });

            if (!Uri.TryCreate(request.BaseUrl, UriKind.Absolute, out var parsedUri)
                || (parsedUri.Scheme != "http" && parsedUri.Scheme != "https"))
                return BadRequest(new { error = "BaseUrl must be a valid HTTP or HTTPS URL." });

            try
            {
                using var httpClient = new System.Net.Http.HttpClient();
                httpClient.BaseAddress = parsedUri;
                httpClient.DefaultRequestHeaders.Add("X-Api-Key", request.ApiKey);
                httpClient.DefaultRequestHeaders.Accept.Add(new System.Net.Http.Headers.MediaTypeWithQualityHeaderValue("application/json"));

                var response = await httpClient.GetAsync("/api/v3/system/status");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var status = System.Text.Json.JsonSerializer.Deserialize<System.Text.Json.JsonElement>(content);
                    var version = status.GetProperty("version").GetString();
                    return Ok(new { success = true, message = "Connection successful", version });
                }
                else
                {
                    return BadRequest(new { success = false, message = $"Connection failed: {response.StatusCode}" });
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error testing Sonarr connection");
                return StatusCode(500, new { success = false, message = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Tests Radarr connectivity using provided URL and API key values.
        /// </summary>
        [HttpPost("radarr/test")]
        public async Task<ActionResult> TestRadarrConnection([FromBody] TestConnectionRequest request)
        {
            if (string.IsNullOrWhiteSpace(request.BaseUrl))
                return BadRequest(new { error = "BaseUrl is required." });

            if (!Uri.TryCreate(request.BaseUrl, UriKind.Absolute, out var parsedUri)
                || (parsedUri.Scheme != "http" && parsedUri.Scheme != "https"))
                return BadRequest(new { error = "BaseUrl must be a valid HTTP or HTTPS URL." });

            try
            {
                using var httpClient = new System.Net.Http.HttpClient();
                httpClient.BaseAddress = parsedUri;
                httpClient.DefaultRequestHeaders.Add("X-Api-Key", request.ApiKey);
                httpClient.DefaultRequestHeaders.Accept.Add(new System.Net.Http.Headers.MediaTypeWithQualityHeaderValue("application/json"));

                var response = await httpClient.GetAsync("/api/v3/system/status");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var status = System.Text.Json.JsonSerializer.Deserialize<System.Text.Json.JsonElement>(content);
                    var version = status.GetProperty("version").GetString();
                    return Ok(new { success = true, message = "Connection successful", version });
                }
                else
                {
                    return BadRequest(new { success = false, message = $"Connection failed: {response.StatusCode}" });
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error testing Radarr connection");
                return StatusCode(500, new { success = false, message = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Synchronizes library paths and metadata from Radarr.
        /// </summary>
        [HttpPost("radarr/sync")]
        public async Task<ActionResult> SyncRadarr()
        {
            _logger.LogInformation("Radarr library sync requested");
            try
            {
                var result = await _syncService.SyncRadarrAsync();
                if (result.Success)
                {
                    return Ok(result);
                }
                else
                {
                    return BadRequest(result);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error syncing Radarr library");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Synchronizes library paths and metadata from Sonarr.
        /// </summary>
        [HttpPost("sonarr/sync")]
        public async Task<ActionResult> SyncSonarr()
        {
            _logger.LogInformation("Sonarr library sync requested");
            try
            {
                var result = await _syncService.SyncSonarrAsync();
                if (result.Success)
                {
                    return Ok(result);
                }
                else
                {
                    return BadRequest(result);
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error syncing Sonarr library");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Runs Sonarr and Radarr synchronization in a single operation.
        /// </summary>
        [HttpPost("sync-all")]
        public async Task<ActionResult> SyncAll()
        {
            _logger.LogInformation("Full Servarr library sync requested");
            try
            {
                // Run Radarr and Sonarr syncs in parallel for better performance
                var radarrTask = _syncService.SyncRadarrAsync();
                var sonarrTask = _syncService.SyncSonarrAsync();
                
                await Task.WhenAll(radarrTask, sonarrTask);
                
                var radarrResult = await radarrTask;
                var sonarrResult = await sonarrTask;

                return Ok(new
                {
                    radarr = radarrResult,
                    sonarr = sonarrResult,
                    overallSuccess = radarrResult.Success && sonarrResult.Success
                });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error syncing Servarr libraries");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Returns current Sonarr connection settings with masked API key output.
        /// </summary>
        [HttpGet("sonarr/settings")]
        public ActionResult<object> GetSonarrSettings()
        {
            var rawKey = _configuration[ConfigKeys.SonarrApiKey] ?? "";
            return Ok(new
            {
                baseUrl = _configuration[ConfigKeys.SonarrBaseUrl] ?? "",
                apiKey = rawKey.Length > 4
                    ? new string('*', rawKey.Length - 4) + rawKey[^4..]
                    : new string('*', rawKey.Length),
                enabled = _configuration.GetValue<bool>(ConfigKeys.SonarrEnabled, false)
            });
        }

        /// <summary>
        /// Saves Sonarr connection settings to runtime configuration.
        /// </summary>
        [HttpPost("sonarr/settings")]
        public async Task<ActionResult> SaveSonarrSettings([FromBody] ServarrSettingsRequest request)
        {
            return await SaveServarrSettings(ConfigKeys.Sonarr, request);
        }

        /// <summary>
        /// Returns current Radarr connection settings with masked API key output.
        /// </summary>
        [HttpGet("radarr/settings")]
        public ActionResult<object> GetRadarrSettings()
        {
            var rawKey = _configuration[ConfigKeys.RadarrApiKey] ?? "";
            return Ok(new
            {
                baseUrl = _configuration[ConfigKeys.RadarrBaseUrl] ?? "",
                apiKey = rawKey.Length > 4
                    ? new string('*', rawKey.Length - 4) + rawKey[^4..]
                    : new string('*', rawKey.Length),
                enabled = _configuration.GetValue<bool>(ConfigKeys.RadarrEnabled, false)
            });
        }

        /// <summary>
        /// Saves Radarr connection settings to runtime configuration.
        /// </summary>
        [HttpPost("radarr/settings")]
        public async Task<ActionResult> SaveRadarrSettings([FromBody] ServarrSettingsRequest request)
        {
            return await SaveServarrSettings(ConfigKeys.Radarr, request);
        }

        /// <summary>
        /// Returns configured Sonarr path mappings used for host-container translation.
        /// </summary>
        [HttpGet("sonarr/path-mappings")]
        public ActionResult<object> GetSonarrPathMappings()
        {
            try
            {
                var section = _configuration.GetSection(ConfigKeys.SonarrPathMappings);
                var mappings = new List<PathMappingRequest>();
                
                if (section.Exists())
                {
                    foreach (var item in section.GetChildren())
                    {
                        var from = item["From"] ?? "";
                        var to = item["To"] ?? "";
                        if (!string.IsNullOrEmpty(from) && !string.IsNullOrEmpty(to))
                        {
                            mappings.Add(new PathMappingRequest { From = from, To = to });
                        }
                    }
                }
                
                return Ok(new { mappings });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Sonarr path mappings");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Saves Sonarr path mappings with traversal and null-byte validation.
        /// </summary>
        [HttpPost("sonarr/path-mappings")]
        public async Task<ActionResult> SaveSonarrPathMappings([FromBody] List<PathMappingRequest> mappings)
        {
            return await SaveServarrPathMappings(ConfigKeys.Sonarr, mappings);
        }

        /// <summary>
        /// Returns configured Radarr path mappings used for host-container translation.
        /// </summary>
        [HttpGet("radarr/path-mappings")]
        public ActionResult<object> GetRadarrPathMappings()
        {
            try
            {
                var section = _configuration.GetSection(ConfigKeys.RadarrPathMappings);
                var mappings = new List<PathMappingRequest>();
                
                if (section.Exists())
                {
                    foreach (var item in section.GetChildren())
                    {
                        var from = item["From"] ?? "";
                        var to = item["To"] ?? "";
                        if (!string.IsNullOrEmpty(from) && !string.IsNullOrEmpty(to))
                        {
                            mappings.Add(new PathMappingRequest { From = from, To = to });
                        }
                    }
                }
                
                return Ok(new { mappings });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting Radarr path mappings");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Saves Radarr path mappings with traversal and null-byte validation.
        /// </summary>
        [HttpPost("radarr/path-mappings")]
        public async Task<ActionResult> SaveRadarrPathMappings([FromBody] List<PathMappingRequest> mappings)
        {
            return await SaveServarrPathMappings(ConfigKeys.Radarr, mappings);
        }

        private async Task<ActionResult> SaveServarrSettings(string provider, ServarrSettingsRequest request)
        {
            try
            {
                if (provider != ConfigKeys.Sonarr && provider != ConfigKeys.Radarr)
                    return BadRequest(new { error = "Invalid provider." });

                var appsettingsPath = GetAppsettingsPath();
                if (!System.IO.File.Exists(appsettingsPath))
                {
                    _logger.LogWarning("appsettings.json not found at {Path}, creating new file", appsettingsPath);
                    System.IO.Directory.CreateDirectory(System.IO.Path.GetDirectoryName(appsettingsPath)!);
                }

                var jsonContent = System.IO.File.Exists(appsettingsPath)
                    ? await System.IO.File.ReadAllTextAsync(appsettingsPath)
                    : "{}";

                using var jsonDoc = System.Text.Json.JsonDocument.Parse(jsonContent);
                var root = jsonDoc.RootElement;
                var jsonObject = new System.Text.Json.Nodes.JsonObject();

                foreach (var prop in root.EnumerateObject())
                {
                    if (prop.Name != ConfigKeys.Servarr)
                    {
                        jsonObject[prop.Name] = System.Text.Json.Nodes.JsonNode.Parse(prop.Value.GetRawText());
                    }
                }

                var servarrObject = new System.Text.Json.Nodes.JsonObject();
                if (root.TryGetProperty(ConfigKeys.Servarr, out var existingServarr))
                {
                    foreach (var prop in existingServarr.EnumerateObject())
                    {
                        if (prop.Name != provider)
                        {
                            servarrObject[prop.Name] = System.Text.Json.Nodes.JsonNode.Parse(prop.Value.GetRawText());
                        }
                    }
                }

                servarrObject[provider] = new System.Text.Json.Nodes.JsonObject
                {
                    ["BaseUrl"] = request.BaseUrl ?? "",
                    ["ApiKey"] = request.ApiKey ?? "",
                    ["Enabled"] = request.Enabled
                };

                jsonObject[ConfigKeys.Servarr] = servarrObject;
                await System.IO.File.WriteAllTextAsync(
                    appsettingsPath,
                    jsonObject.ToJsonString(new System.Text.Json.JsonSerializerOptions { WriteIndented = true }));

                if (_configuration is Microsoft.Extensions.Configuration.IConfigurationRoot configRoot)
                {
                    configRoot.Reload();
                    _logger.LogInformation("Configuration reloaded");
                }

                if (provider == ConfigKeys.Sonarr)
                {
                    _sonarrService.ReloadConfiguration();
                    _logger.LogInformation("Sonarr settings saved and configuration reloaded");
                    return Ok(new { message = "Sonarr settings saved successfully. The service will reconnect with the new settings." });
                }

                _radarrService.ReloadConfiguration();
                _logger.LogInformation("Radarr settings saved and configuration reloaded");
                return Ok(new { message = "Radarr settings saved successfully. The service will reconnect with the new settings." });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error saving {Provider} settings", provider);
                return StatusCode(500, new { error = $"Failed to save {provider} settings. Check logs for details." });
            }
        }

        private async Task<ActionResult> SaveServarrPathMappings(string provider, List<PathMappingRequest> mappings)
        {
            try
            {
                if (provider != ConfigKeys.Sonarr && provider != ConfigKeys.Radarr)
                    return BadRequest(new { error = "Invalid provider." });

                foreach (var mapping in mappings)
                {
                    var from = mapping.From ?? "";
                    var to = mapping.To ?? "";
                    if (from.Contains("..") || from.Contains('\0') || to.Contains("..") || to.Contains('\0'))
                        return BadRequest(new { error = "Path mappings must not contain '..' or null characters." });
                }

                var appsettingsPath = GetAppsettingsPath();
                if (!System.IO.File.Exists(appsettingsPath))
                {
                    System.IO.Directory.CreateDirectory(System.IO.Path.GetDirectoryName(appsettingsPath)!);
                }

                var jsonContent = System.IO.File.Exists(appsettingsPath)
                    ? await System.IO.File.ReadAllTextAsync(appsettingsPath)
                    : "{}";

                using var jsonDoc = System.Text.Json.JsonDocument.Parse(jsonContent);
                var root = jsonDoc.RootElement;
                var jsonObject = new System.Text.Json.Nodes.JsonObject();

                foreach (var prop in root.EnumerateObject())
                {
                    if (prop.Name != ConfigKeys.Servarr)
                    {
                        jsonObject[prop.Name] = System.Text.Json.Nodes.JsonNode.Parse(prop.Value.GetRawText());
                    }
                }

                var servarrObject = new System.Text.Json.Nodes.JsonObject();
                if (root.TryGetProperty(ConfigKeys.Servarr, out var existingServarr))
                {
                    foreach (var prop in existingServarr.EnumerateObject())
                    {
                        if (prop.Name != provider)
                        {
                            servarrObject[prop.Name] = System.Text.Json.Nodes.JsonNode.Parse(prop.Value.GetRawText());
                        }
                    }
                }

                var providerObject = new System.Text.Json.Nodes.JsonObject();
                if (root.TryGetProperty(ConfigKeys.Servarr, out var servarr) && servarr.TryGetProperty(provider, out var existingProvider))
                {
                    foreach (var prop in existingProvider.EnumerateObject())
                    {
                        if (prop.Name != ConfigKeys.PathMappings)
                        {
                            providerObject[prop.Name] = System.Text.Json.Nodes.JsonNode.Parse(prop.Value.GetRawText());
                        }
                    }
                }

                var mappingsArray = new System.Text.Json.Nodes.JsonArray();
                foreach (var mapping in mappings.Where(m => !string.IsNullOrEmpty(m.From) && !string.IsNullOrEmpty(m.To)))
                {
                    mappingsArray.Add(new System.Text.Json.Nodes.JsonObject
                    {
                        ["From"] = mapping.From,
                        ["To"] = mapping.To
                    });
                }

                providerObject[ConfigKeys.PathMappings] = mappingsArray;
                servarrObject[provider] = providerObject;
                jsonObject[ConfigKeys.Servarr] = servarrObject;

                await System.IO.File.WriteAllTextAsync(
                    appsettingsPath,
                    jsonObject.ToJsonString(new System.Text.Json.JsonSerializerOptions { WriteIndented = true }));

                if (_configuration is Microsoft.Extensions.Configuration.IConfigurationRoot configRoot)
                {
                    configRoot.Reload();
                }

                return Ok(new { message = "Path mappings saved successfully" });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error saving {Provider} path mappings", provider);
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        private string GetAppsettingsPath()
        {
            var contentRoot = _environment.ContentRootPath;
            var configAppsettingsPath = System.IO.Path.Combine(contentRoot, "config", "appsettings.json");
            var rootAppsettingsPath = System.IO.Path.Combine(contentRoot, "appsettings.json");

            if (System.IO.File.Exists(configAppsettingsPath))
                return configAppsettingsPath;

            if (System.IO.File.Exists(rootAppsettingsPath))
                return rootAppsettingsPath;

            return configAppsettingsPath;
        }

        /// <summary>
        /// Starts asynchronous Servarr matching for all analyzed videos.
        /// </summary>
        [HttpPost("match-videos")]
        public Task<ActionResult> MatchVideosWithServarr()
        {
            _logger.LogInformation("Video matching with Servarr requested");
            try
            {
                // Check if services are available
                if (!_sonarrService.IsEnabled && !_radarrService.IsEnabled)
                {
                    return Task.FromResult<ActionResult>(BadRequest(new { error = "Neither Sonarr nor Radarr is enabled. Please configure at least one service in Settings." }));
                }

                if ((_sonarrService.IsEnabled && !_sonarrService.IsConnected) && 
                    (_radarrService.IsEnabled && !_radarrService.IsConnected))
                {
                    return Task.FromResult<ActionResult>(BadRequest(new { error = "Servarr services are enabled but not connected. Please check your connection settings." }));
                }

                // Generate match ID
                var matchId = Guid.NewGuid().ToString();
                _progressService.CreateProgress(matchId);

                // Start matching in background.
                // NOTE: IServiceProvider.CreateScope() is intentional here — MatchAllVideosAsync is a
                // long-running operation that outlives the HTTP request. The request's scoped DI container
                // is disposed when the response is sent; a fresh scope ensures VideoServarrMatcherService
                // and VideoMatchingProgressService are not accessed after disposal.
                // Per PLAN.md §A2 Risk Flag 3, this is an accepted documented exception.
                _ = Task.Run(async () =>
                {
                    try
                    {
                        using var scope = _serviceProvider.CreateScope();
                        var matcherService = scope.ServiceProvider.GetRequiredService<VideoServarrMatcherService>();
                        var progressService = scope.ServiceProvider.GetRequiredService<VideoMatchingProgressService>();

                        _logger.LogInformation("Starting video matching process (matchId: {MatchId})...", matchId);
                        await matcherService.MatchAllVideosAsync(progressService, matchId);
                        _logger.LogInformation("Video matching completed (matchId: {MatchId})", matchId);
                    }
                    catch (OperationCanceledException)
                    {
                        _logger.LogWarning("Video matching cancelled on application shutdown (matchId: {MatchId})", matchId);
                        _progressService.FailProgress(matchId, "Cancelled on application shutdown");
                    }
                    catch (Exception ex)
                    {
                        _logger.LogError(ex, "Error in background video matching (matchId: {MatchId})", matchId);
                        _progressService.FailProgress(matchId, "An internal error occurred. Check logs for details.");
                    }
                }, _lifetime.ApplicationStopping);

                return Task.FromResult<ActionResult>(Ok(new { matchId, message = "Video matching started" }));
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error starting video matching with Servarr");
                return Task.FromResult<ActionResult>(StatusCode(500, new { error = "An internal error occurred. Check logs for details." }));
            }
        }

        /// <summary>
        /// Returns progress details for a specific background video-matching run.
        /// </summary>
        [HttpGet("match-videos/progress/{matchId}")]
        public ActionResult<VideoMatchingProgress> GetMatchProgress(string matchId)
        {
            var progress = _progressService.GetProgress(matchId);
            if (progress == null)
            {
                return NotFound(new { error = "Match progress not found" });
            }
            return Ok(progress);
        }

        /// <summary>
        /// Returns currently active video-matching job identifiers.
        /// </summary>
        [HttpGet("match-videos/active")]
        public ActionResult<object> GetActiveMatches()
        {
            try
            {
                var activeMatchId = _progressService.GetActiveMatchId();
                if (activeMatchId == null)
                {
                    return Ok(new { activeMatchId = (string?)null, hasActiveMatch = false });
                }

                var progress = _progressService.GetProgress(activeMatchId);
                return Ok(new 
                { 
                    activeMatchId = activeMatchId,
                    hasActiveMatch = true,
                    progress = progress
                });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting active matches");
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

        /// <summary>
        /// Starts asynchronous Servarr matching for videos under one library path.
        /// </summary>
        [HttpPost("match-videos/{libraryPathId}")]
        public async Task<ActionResult> MatchVideosForLibraryPath(int libraryPathId)
        {
            _logger.LogInformation("Video matching for library path {LibraryPathId} requested", libraryPathId);
            try
            {
                var matchedCount = await _matcherService.MatchVideosForLibraryPathAsync(libraryPathId);
                return Ok(new { matched = matchedCount, message = $"Matched {matchedCount} videos for library path" });
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error matching videos for library path {LibraryPathId}", libraryPathId);
                return StatusCode(500, new { error = "An internal error occurred. Check logs for details." });
            }
        }

    }

    public class TestConnectionRequest
    {
        public string BaseUrl { get; set; } = string.Empty;
        public string ApiKey { get; set; } = string.Empty;
    }

    public class ServarrStatusResponse
    {
        public ServiceStatus Sonarr { get; set; } = new();
        public ServiceStatus Radarr { get; set; } = new();
    }

    public class ServiceStatus
    {
        public bool Enabled { get; set; }
        public bool Connected { get; set; }
        public string? Version { get; set; }
    }

    public class ServarrSettingsRequest
    {
        public string BaseUrl { get; set; } = string.Empty;
        public string ApiKey { get; set; } = string.Empty;
        public bool Enabled { get; set; }
    }

    public class PathMappingRequest
    {
        public string From { get; set; } = string.Empty;
        public string To { get; set; } = string.Empty;
    }

}

