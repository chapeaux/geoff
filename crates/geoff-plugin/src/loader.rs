use std::path::Path;

use libloading::{Library, Symbol};

use crate::traits::Plugin;

/// A Rust plugin loaded from a cdylib shared library.
///
/// The shared library must export a `create_plugin` function with the signature:
/// `extern "C" fn() -> *mut dyn Plugin`
pub struct RustPluginLoader {
    /// Loaded libraries kept alive for the duration of the plugin's lifetime.
    _libraries: Vec<Library>,
    /// Plugins created from loaded libraries.
    plugins: Vec<Box<dyn Plugin>>,
}

impl RustPluginLoader {
    pub fn new() -> Self {
        Self {
            _libraries: Vec::new(),
            plugins: Vec::new(),
        }
    }

    /// Load a Rust plugin from a cdylib shared library at the given path.
    ///
    /// The library must export: `extern "C" fn create_plugin() -> *mut dyn Plugin`
    ///
    /// # Safety
    ///
    /// This function loads and executes arbitrary code from a shared library.
    /// The caller must ensure the library is trusted and compatible.
    pub unsafe fn load(
        &mut self,
        path: &Path,
    ) -> std::result::Result<&dyn Plugin, Box<dyn std::error::Error + Send + Sync>> {
        // SAFETY: caller guarantees the library is trusted
        let lib = unsafe { Library::new(path) }.map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "failed to load plugin library {}: {e}",
                path.display()
            )))
        })?;

        // SAFETY: caller guarantees the library exports a compatible create_plugin symbol
        let constructor: Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> =
            unsafe { lib.get(b"create_plugin") }.map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "plugin library {} missing create_plugin symbol: {e}",
                    path.display()
                )))
            })?;

        let raw = unsafe { constructor() };
        if raw.is_null() {
            return Err(Box::new(std::io::Error::other(format!(
                "create_plugin returned null in {}",
                path.display()
            ))));
        }
        let plugin = unsafe { Box::from_raw(raw) };

        self.plugins.push(plugin);
        self._libraries.push(lib);

        // Return a reference to the last inserted plugin
        Ok(self.plugins.last().unwrap().as_ref())
    }

    /// Returns references to all loaded plugins.
    pub fn plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    /// Consume the loader and return all loaded plugins.
    pub fn into_plugins(self) -> Vec<Box<dyn Plugin>> {
        self.plugins
    }
}

impl Default for RustPluginLoader {
    fn default() -> Self {
        Self::new()
    }
}
