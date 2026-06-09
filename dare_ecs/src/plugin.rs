use std::any::TypeId;
use std::collections::HashSet;

/// A plugin is initialized via [`Self::build`], finalized in [`Self::finish`], and may
/// detach resources in [`Self::cleanup`] (e.g. moving a sub-app to another thread).
pub trait Plugin {
    /// Defines the plugin's initialization logic
    fn build(&self, app: &mut super::App);
    /// Defines the plugin's finalization logic
    fn finish(&self, _app: &mut super::App) {}
    /// Consumes the plugin and allows for any cleanup to be ran
    fn cleanup(self: Box<Self>, _app: &mut super::App)
    where
        Self: Sized,
    {
    }
}

trait ErasedPlugin {
    fn finish(&self, app: &mut super::App);
    fn cleanup(self: Box<Self>, app: &mut super::App);
}

impl<T: Plugin> ErasedPlugin for T {
    fn finish(&self, app: &mut super::App) {
        Plugin::finish(self, app);
    }

    fn cleanup(self: Box<Self>, app: &mut super::App) {
        Plugin::cleanup(self, app);
    }
}

pub(crate) struct PluginRegistry {
    plugins: Vec<Box<dyn ErasedPlugin>>,
    plugin_ids: HashSet<TypeId>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            plugin_ids: HashSet::new(),
        }
    }

    pub fn contains<P: Plugin + 'static>(&self) -> bool {
        self.plugin_ids.contains(&TypeId::of::<P>())
    }

    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        if !self.plugin_ids.insert(TypeId::of::<P>()) {
            return;
        }
        self.plugins.push(Box::new(plugin));
    }

    pub fn finish(&self, app: &mut super::App) {
        for plugin in &self.plugins {
            plugin.finish(app);
        }
    }

    pub fn cleanup(&mut self, app: &mut super::App) {
        for plugin in self.plugins.drain(..) {
            ErasedPlugin::cleanup(plugin, app);
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
