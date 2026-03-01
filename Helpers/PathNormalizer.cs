namespace Optimarr.Helpers
{
	/// <summary>
	/// Canonical path normalization for Servarr path comparison.
	/// Handles: Windows backslashes, trailing slashes, URL-encoded characters.
	/// </summary>
	public static class PathNormalizer
	{
		public static string Normalize(string? path)
		{
			if (string.IsNullOrEmpty(path)) return string.Empty;

			// Decode URL-encoded characters (e.g., %20 → space, %2F → /)
			// Uri.UnescapeDataString is safe for already-decoded strings (returns unchanged)
			var decoded = Uri.UnescapeDataString(path);

			// Normalize: forward slashes, strip trailing slash, lowercase
			return decoded.Replace('\\', '/').TrimEnd('/').ToLowerInvariant();
		}
	}
}
