namespace Optimarr.Middleware
{
	public class ApiKeyMiddleware
	{
		private readonly RequestDelegate _next;
		private readonly IConfiguration _configuration;
		private const string ApiKeyHeaderName = "X-Optimarr-Key";

		// Paths that bypass authentication regardless of configured key.
		// /api/system/health   — required for Docker liveness checks
		// /api/system/migration — required for startup probe during long DB migrations
		// /api/system/ready    — required for readiness checks
		// /api/system/auth-config — required so the frontend can discover whether auth is enabled
		private static readonly string[] PublicPaths =
		[
			"/api/system/health",
			"/api/system/migration",
			"/api/system/ready",
			"/api/system/auth-config"
		];

		public ApiKeyMiddleware(RequestDelegate next, IConfiguration configuration)
		{
			_next = next;
			_configuration = configuration;
		}

		public async Task InvokeAsync(HttpContext context)
		{
			var configuredKey = _configuration["Auth:ApiKey"];

			// Auth is disabled when no key is configured (backward-compatible default)
			if (string.IsNullOrEmpty(configuredKey))
			{
				await _next(context);
				return;
			}

			// Allow public paths through without a key (health check, auth discovery)
			if (PublicPaths.Any(p => context.Request.Path.StartsWithSegments(p, StringComparison.OrdinalIgnoreCase)))
			{
				await _next(context);
				return;
			}

			// Only protect /api/* routes — static files under wwwroot are not protected
			if (!context.Request.Path.StartsWithSegments("/api", StringComparison.OrdinalIgnoreCase))
			{
				await _next(context);
				return;
			}

			if (!context.Request.Headers.TryGetValue(ApiKeyHeaderName, out var providedKey)
				|| !string.Equals(configuredKey, providedKey, StringComparison.Ordinal))
			{
				context.Response.StatusCode = StatusCodes.Status401Unauthorized;
				context.Response.ContentType = "application/json";
				await context.Response.WriteAsync("{\"error\":\"Unauthorized\"}");
				return;
			}

			await _next(context);
		}
	}
}
