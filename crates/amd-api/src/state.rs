use std::sync::Arc;

use amd_adapters::AdapterRegistry;
use amd_bus::Bus;
use amd_store::{ClickhouseStore, Entitlements, ReferenceStore};

#[derive(Clone)]
pub struct AppState {
    pub adapters: Arc<AdapterRegistry>,
    pub bus: Option<Bus>,
    pub archive: Option<ClickhouseStore>,
    pub reference: Option<ReferenceStore>,
    pub entitlements: Option<Entitlements>,
}

impl AppState {
    /// Adapters only. Lets the API boot and serve live sources before any
    /// infrastructure exists, which keeps the first-run experience short.
    pub fn adapters_only(adapters: AdapterRegistry) -> Self {
        AppState {
            adapters: Arc::new(adapters),
            bus: None,
            archive: None,
            reference: None,
            entitlements: None,
        }
    }
}
