namespace Optimarr.Services
{
    public class ServarrRootFolder
    {
        public int Id { get; set; }
        public string Path { get; set; } = string.Empty;
        public bool Accessible { get; set; }
        public long? FreeSpace { get; set; }
    }
}
