using System.Net.Http.Headers;
using System.Text.Json;
using Microsoft.Extensions.Options;
using Optimarr.Models;

namespace Optimarr.Services
{
    public abstract class ServarrServiceBase : IServarrService
    {
        private readonly IOptionsMonitor<ServarrOptions> _optionsMonitor;
        protected HttpClient _httpClient;
        protected readonly ILogger _logger;
        private readonly object _clientSwapLock = new();

        private bool _isEnabled;
        private bool _isConnected;
        private string? _apiKey;
        private string? _baseUrl;
        private string? _basicAuthUsername;
        private string? _basicAuthPassword;

        public abstract string ProviderName { get; }
        protected abstract ServarrInstanceOptions? GetInstanceOptions(ServarrOptions options);

        protected ServarrServiceBase(IOptionsMonitor<ServarrOptions> optionsMonitor, ILogger logger)
        {
            _optionsMonitor = optionsMonitor;
            _logger = logger;
            _httpClient = CreateHttpClient();
            Initialize();
        }

        public bool IsEnabled => _isEnabled;
        public bool IsConnected => _isConnected;

        public void ReloadConfiguration()
        {
            Initialize();
        }

        public async Task<string?> GetVersionAsync()
        {
            if (!_isEnabled)
            {
                return null;
            }

            try
            {
                EnsureApiKeyHeader();
                var response = await _httpClient.GetAsync("/api/v3/system/status");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var status = JsonSerializer.Deserialize<JsonElement>(content);
                    return status.GetProperty("version").GetString();
                }

                if (response.StatusCode == System.Net.HttpStatusCode.Unauthorized)
                {
                    _logger.LogWarning("Unauthorized access to {Provider} system status. Check API key.", ProviderName);
                    _isConnected = false;
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting {Provider} version", ProviderName);
                _isConnected = false;
            }

            return null;
        }

        public async Task<List<ServarrRootFolder>> GetRootFoldersAsync()
        {
            if (!_isEnabled || !_isConnected)
            {
                return new List<ServarrRootFolder>();
            }

            EnsureApiKeyHeader();

            try
            {
                var response = await _httpClient.GetAsync("/api/v3/rootFolder");
                if (response.IsSuccessStatusCode)
                {
                    var content = await response.Content.ReadAsStringAsync();
                    var rootFolders = JsonSerializer.Deserialize<List<ServarrRootFolder>>(content, new JsonSerializerOptions
                    {
                        PropertyNameCaseInsensitive = true
                    });
                    return rootFolders ?? new List<ServarrRootFolder>();
                }
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Error getting {Provider} root folders", ProviderName);
            }

            return new List<ServarrRootFolder>();
        }

        protected void EnsureApiKeyHeader()
        {
            // Headers are configured during Initialize() on a freshly built HttpClient instance.
            // Avoid mutating DefaultRequestHeaders during request execution to prevent races.
        }

        private void Initialize()
        {
            var instanceOptions = GetInstanceOptions(_optionsMonitor.CurrentValue) ?? new ServarrInstanceOptions();
            _baseUrl = instanceOptions.BaseUrl;
            _apiKey = instanceOptions.ApiKey;
            _basicAuthUsername = instanceOptions.BasicAuthUsername;
            _basicAuthPassword = instanceOptions.BasicAuthPassword;
            _isEnabled = instanceOptions.Enabled && !string.IsNullOrEmpty(_baseUrl) && !string.IsNullOrEmpty(_apiKey);

            if (_isEnabled)
            {
                var configuredClient = CreateHttpClient();
                configuredClient.BaseAddress = new Uri(_baseUrl!);
                configuredClient.DefaultRequestHeaders.Add("X-Api-Key", _apiKey!);

                if (!string.IsNullOrEmpty(_basicAuthUsername) && !string.IsNullOrEmpty(_basicAuthPassword))
                {
                    var authValue = Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes($"{_basicAuthUsername}:{_basicAuthPassword}"));
                    configuredClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Basic", authValue);
                }

                configuredClient.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));

                // Initialize() is called from constructors and must stay sync.
                // Keep this synchronous bridge until a broader service initialization redesign.
                CheckConnectionAsync(configuredClient).GetAwaiter().GetResult();
                SwapHttpClient(configuredClient);
            }
            else
            {
                _isConnected = false;
                SwapHttpClient(CreateHttpClient());
            }
        }

        private async Task CheckConnectionAsync(HttpClient client)
        {
            try
            {
                EnsureApiKeyHeader();
                var response = await client.GetAsync("/api/v3/system/status");
                _isConnected = response.IsSuccessStatusCode;
                if (!_isConnected)
                {
                    _logger.LogWarning("{Provider} connection check failed: {StatusCode}", ProviderName, response.StatusCode);
                }
            }
            catch (Exception ex)
            {
                _isConnected = false;
                _logger.LogWarning(ex, "{Provider} connection check failed", ProviderName);
            }
        }

        private HttpClient CreateHttpClient()
        {
            return new HttpClient
            {
                Timeout = TimeSpan.FromMinutes(5)
            };
        }

        private void SwapHttpClient(HttpClient nextClient)
        {
            lock (_clientSwapLock)
            {
                // Keep old clients alive to avoid canceling any in-flight requests started pre-reload.
                _httpClient = nextClient;
            }
        }
    }
}
