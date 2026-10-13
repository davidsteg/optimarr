# Config directory

Optimarr 2 keeps all settings in its database (`/app/data/optimarr2.db`) and edits them from the
web UI, so this folder is normally empty.

It is only read once, on first start: if it contains an `appsettings.json` from Optimarr 1.x,
the Sonarr, Radarr, Jellyfin and scoring settings are imported from it. You can delete the file
afterwards.
