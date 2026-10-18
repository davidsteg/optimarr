-- Path of the played file as Jellyfin sees it, so Jellystat imports look each item up only once.
ALTER TABLE playback_events ADD COLUMN remote_path TEXT;
