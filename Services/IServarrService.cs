namespace Optimarr.Services
{
    public interface IServarrService
    {
        string ProviderName { get; }
        bool IsEnabled { get; }
        bool IsConnected { get; }
        Task<string?> GetVersionAsync();
        Task<List<ServarrRootFolder>> GetRootFoldersAsync();
        void ReloadConfiguration();
    }

    public interface ISonarrService : IServarrService
    {
        Task<SonarrEpisode?> GetEpisode(int episodeId);
        Task<List<SonarrEpisode>> GetEpisodesBySeries(int seriesId);
        Task<SonarrEpisodeFile?> GetEpisodeFile(int episodeFileId);
        Task<List<SonarrEpisodeFile>> GetAllEpisodeFiles();
        Task<List<SonarrSeries>> GetSeries();
        Task<SonarrEpisode?> FindEpisodeByPath(string filePath);
        Task<bool> DeleteEpisodeFile(int episodeFileId);
        Task<bool> TriggerEpisodeSearch(int episodeId);
    }

    public interface IRadarrService : IServarrService
    {
        Task<RadarrMovie?> GetMovie(int movieId);
        Task<List<RadarrMovie>> GetMovies();
        Task<RadarrMovie?> FindMovieByPath(string filePath);
        Task<bool> DeleteMovieFile(int movieFileId);
        Task<bool> TriggerMovieSearch(int movieId);
    }
}
