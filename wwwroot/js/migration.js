// Database Migration Status Management
let migrationCheckInterval = null;
let activeMigrationSeenAt = null;

export async function checkMigrationStatus() {
    try {
        const response = await fetch('/api/system/migration');
        if (!response.ok) {
            // If endpoint doesn't exist or fails, hide banner
            hideMigrationBanner();
            return;
        }
        
        const progress = await response.json();
        const rawStatus = progress.status ?? progress.Status ?? '';
        const status = rawStatus.toString().trim().toLowerCase();
        
        // Don't show banner for "unknown" status (migration service hasn't started yet)
        if (status === 'unknown') {
            hideMigrationBanner();
            return;
        }
        
        // Poll while migration service is evaluating/applying work, but only show the banner
        // when migration is actually doing write work (creating/migrating) or on explicit error.
        // This avoids a "flash on refresh" when status briefly reports "checking".
        const isActiveMigration =
            status === 'creating' ||
            status === 'migrating';
        const shouldPollMigration =
            status === 'checking' ||
            status === 'creating' ||
            status === 'migrating';
        const isError = status === 'error';

        // Reduce startup flash noise: only show active migration if it persists briefly.
        if (isActiveMigration) {
            activeMigrationSeenAt ??= Date.now();
        } else {
            activeMigrationSeenAt = null;
        }
        const isPersistentActiveMigration =
            isActiveMigration &&
            activeMigrationSeenAt !== null &&
            (Date.now() - activeMigrationSeenAt >= 1500);

        if (isPersistentActiveMigration || isError) {
            updateMigrationBanner(progress, status);
        } else {
            hideMigrationBanner();
        }

        // If migration is in progress, poll for updates
        if (shouldPollMigration) {
            if (!migrationCheckInterval) {
                migrationCheckInterval = setInterval(checkMigrationStatus, 1000); // Poll every second
            }
        } else {
            // Migration completed/idle/error (not actively running), stop polling
            if (migrationCheckInterval) {
                clearInterval(migrationCheckInterval);
                migrationCheckInterval = null;
            }
        }
    } catch (error) {
        console.error('Error checking migration status:', error);
        hideMigrationBanner();
    }
}

function updateMigrationBanner(progress, normalizedStatus) {
    const banner = document.getElementById('migrationBanner');
    const messageEl = document.getElementById('migrationMessage');
    const detailsEl = document.getElementById('migrationDetails');
    
    if (!banner || !messageEl) return;
    
    // Show banner
    banner.style.display = 'block';
    
    // Update message
    messageEl.textContent = progress.message || progress.Message || 'Checking database...';
    
    // Update details
    if (detailsEl) {
        let details = '';
        
        const pendingMigrations = progress.pendingMigrations || progress.PendingMigrations || [];
        const appliedMigrations = progress.appliedMigrations || progress.AppliedMigrations || [];
        const errorText = progress.error || progress.Error;

        if (pendingMigrations.length > 0) {
            details += `Pending: ${pendingMigrations.join(', ')}`;
        }
        
        if (appliedMigrations.length > 0) {
            if (details) details += ' | ';
            details += `Applied: ${appliedMigrations.join(', ')}`;
        }
        
        // Show error details if available
        if (normalizedStatus === 'error' && errorText) {
            if (details) details += ' | ';
            details += `Error: ${errorText}`;
        }
        
        if (details) {
            detailsEl.textContent = details;
            detailsEl.style.display = 'block';
        } else {
            detailsEl.style.display = 'none';
        }
    }
    
    // Update banner class based on status
    banner.className = 'migration-banner';
    if (normalizedStatus === 'error') {
        banner.classList.add('error');
    } else if (normalizedStatus === 'completed') {
        banner.classList.add('completed');
    }
    
    // Update icon based on status
    const iconEl = banner.querySelector('.migration-banner-icon');
    if (iconEl) {
        if (normalizedStatus === 'completed') {
            iconEl.textContent = '✓';
            iconEl.style.animation = 'none';
        } else if (normalizedStatus === 'error') {
            iconEl.textContent = '✗';
            iconEl.style.animation = 'none';
        } else {
            iconEl.textContent = '🔄';
            iconEl.style.animation = 'spin 2s linear infinite';
        }
    }
}

function hideMigrationBanner() {
    const banner = document.getElementById('migrationBanner');
    if (banner) {
        banner.style.display = 'none';
    }
    activeMigrationSeenAt = null;
    
    if (migrationCheckInterval) {
        clearInterval(migrationCheckInterval);
        migrationCheckInterval = null;
    }
}

// Export for potential manual refresh
export { checkMigrationStatus as refreshMigrationStatus };

