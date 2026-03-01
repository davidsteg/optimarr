namespace Optimarr.Models
{
    public class ServarrOptions
    {
        public ServarrInstanceOptions Sonarr { get; set; } = new();
        public ServarrInstanceOptions Radarr { get; set; } = new();
    }

    public class ServarrInstanceOptions
    {
        public string BaseUrl { get; set; } = string.Empty;
        public string ApiKey { get; set; } = string.Empty;
        public bool Enabled { get; set; }
        public string? BasicAuthUsername { get; set; }
        public string? BasicAuthPassword { get; set; }
    }
}
