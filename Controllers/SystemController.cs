using System.Reflection;
using Microsoft.AspNetCore.Mvc;
using Optimarr.Services;

namespace Optimarr.Controllers
{
    [ApiController]
    [Route("api/[controller]")]
    public class SystemController : ControllerBase
    {
        private readonly ILogger<SystemController> _logger;

        public SystemController(ILogger<SystemController> logger)
        {
            _logger = logger;
        }

        /// <summary>
        /// Returns application liveness status, current version, and server timestamp.
        /// </summary>
        [HttpGet("health")]
        public ActionResult<object> GetHealth()
        {
            return Ok(new
            {
                status = "healthy",
                version = ReadCurrentVersion(),
                timestamp = DateTime.UtcNow
            });
        }

        /// <summary>
        /// Returns current database migration progress information.
        /// </summary>
        [HttpGet("migration")]
        public ActionResult<object> GetMigrationStatus()
        {
            var progress = DatabaseMigrationService.GetMigrationProgress();
            if (progress == null)
            {
                return Ok(new
                {
                    status = "unknown",
                    message = "Migration status not available"
                });
            }

            return Ok(progress);
        }

        /// <summary>
        /// Returns readiness state and reports 503 until startup migrations complete.
        /// </summary>
        [HttpGet("ready")]
        public ActionResult<object> GetReadiness()
        {
            var progress = DatabaseMigrationService.GetMigrationProgress();
            if (progress == null)
            {
                return StatusCode(StatusCodes.Status503ServiceUnavailable, new
                {
                    status = "not_ready",
                    reason = "migration_status_unavailable",
                    message = "Migration status is not available yet."
                });
            }

            var migrationStatus = progress.Status?.Trim().ToLowerInvariant();
            var isReady = migrationStatus == "completed";

            if (!isReady)
            {
                return StatusCode(StatusCodes.Status503ServiceUnavailable, new
                {
                    status = "not_ready",
                    migrationStatus = progress.Status,
                    progress.Message
                });
            }

            return Ok(new
            {
                status = "ready",
                migrationStatus = progress.Status
            });
        }

        /// <summary>
        /// Returns application version metadata from VERSION file or assembly attributes.
        /// </summary>
        [HttpGet("version")]
        public ActionResult<object> GetVersion()
        {
            var assembly = Assembly.GetExecutingAssembly();
            var assemblyVersion = assembly.GetName().Version?.ToString() ?? "Unknown";
            var informationalVersion = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
            var versionFromFile = ReadVersionFile();

            return Ok(new
            {
                version = versionFromFile ?? informationalVersion ?? assemblyVersion,
                assemblyVersion,
                buildDate = GetBuildDate(assembly),
                source = versionFromFile != null ? "VERSION file" : "Assembly"
            });
        }

        /// <summary>
        /// Accepts a reboot request and returns platform-specific restart guidance.
        /// </summary>
        [HttpPost("reboot")]
        public ActionResult Reboot()
        {
            _logger.LogWarning("Reboot requested by user");
            
            // Note: Actual reboot requires system permissions and is platform-specific
            // For Docker containers, this would typically be handled by the container orchestrator
            // For now, we'll just log the request and return a message
            
            // In a Docker environment, you might want to:
            // - Exit the application (which will be restarted by Docker if restart policy is set)
            // - Send a signal to the container orchestrator
            // - Use a health check endpoint to trigger restart
            
            return Ok(new
            {
                message = "Reboot request received. In Docker, the container will restart if a restart policy is configured. For native installations, please restart the service manually.",
                note = "The application will attempt to gracefully shut down."
            });
        }

        private static string? ReadVersionFile()
        {
            var versionFile = Path.Combine(AppContext.BaseDirectory, "VERSION");
            if (!System.IO.File.Exists(versionFile)) return null;
            try { return System.IO.File.ReadAllText(versionFile).Trim(); }
            catch { return null; }
        }

        private string ReadCurrentVersion()
        {
            var assembly = Assembly.GetExecutingAssembly();
            var informationalVersion = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
            return ReadVersionFile() ?? informationalVersion ?? assembly.GetName().Version?.ToString() ?? "Unknown";
        }

        private string? GetBuildDate(Assembly assembly)
        {
            try
            {
                var filePath = assembly.Location;
                if (string.IsNullOrEmpty(filePath)) return null;
                
                var fileInfo = new System.IO.FileInfo(filePath);
                return fileInfo.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss");
            }
            catch
            {
                return null;
            }
        }
    }
}

