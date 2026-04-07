use std::path::Path;
use std::sync::Arc;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// Watches content, template, and config directories for changes.
/// Sends notifications on the broadcast channel when files change.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Start watching the given directories. Sends `()` on `tx` when any file changes.
    pub fn new(
        watch_dirs: &[&Path],
        tx: Arc<broadcast::Sender<()>>,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res
                && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
            {
                let _ = tx.send(());
            }
        })?;

        for dir in watch_dirs {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
            }
        }

        Ok(Self { _watcher: watcher })
    }
}
