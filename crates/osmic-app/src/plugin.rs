use crate::app::App;

/// A composable unit of functionality, inspired by Bevy's plugin system.
///
/// Plugins configure the `App` during the build phase by inserting resources,
/// registering event handlers, and adding other plugins.
pub trait Plugin: Send + Sync + 'static {
    /// Called during app construction to configure resources and events.
    fn build(&self, app: &mut App);

    /// Returns true when this plugin's prerequisites are satisfied.
    fn ready(&self, _app: &App) -> bool {
        true
    }

    /// Called after all plugins are built, for finalization.
    fn finish(&self, _app: &mut App) {}

    /// Called during app shutdown for cleanup.
    fn cleanup(&self, _app: &mut App) {}

    /// Human-readable name (defaults to type name).
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

/// A collection of plugins that are added together.
pub trait PluginGroup {
    fn build(self) -> PluginGroupBuilder;
}

/// Builder for assembling a group of plugins.
pub struct PluginGroupBuilder {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginGroupBuilder {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn into_plugins(self) -> Vec<Box<dyn Plugin>> {
        self.plugins
    }
}

impl Default for PluginGroupBuilder {
    fn default() -> Self {
        Self::new()
    }
}
