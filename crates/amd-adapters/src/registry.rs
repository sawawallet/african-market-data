use std::collections::HashMap;
use std::sync::Arc;

use amd_core::ExchangeCode;

use crate::Adapter;
use crate::http::HttpClient;
use crate::kwayisi::KwayisiAdapter;

/// Adapters serving each venue, most-preferred first.
///
/// Registration order is meaningful: a later `register` takes precedence, so an
/// operator holding a licence registers their direct feed adapter after the
/// defaults and it wins without any further configuration.
#[derive(Default, Clone)]
pub struct AdapterRegistry {
    by_exchange: HashMap<ExchangeCode, Vec<Arc<dyn Adapter>>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The free, unencumbered sources this repository ships.
    pub fn with_defaults(http: HttpClient) -> Self {
        let mut r = Self::new();
        r.register(Arc::new(KwayisiAdapter::new(http)));
        r
    }

    pub fn register(&mut self, adapter: Arc<dyn Adapter>) -> &mut Self {
        for ex in adapter.exchanges() {
            let list = self.by_exchange.entry(*ex).or_default();
            list.retain(|a| a.id() != adapter.id());
            list.insert(0, Arc::clone(&adapter));
        }
        self
    }

    pub fn for_exchange(&self, exchange: ExchangeCode) -> &[Arc<dyn Adapter>] {
        self.by_exchange.get(&exchange).map_or(&[], Vec::as_slice)
    }

    /// The preferred adapter for a venue.
    pub fn preferred(&self, exchange: ExchangeCode) -> Option<&Arc<dyn Adapter>> {
        self.for_exchange(exchange).first()
    }

    pub fn supported(&self) -> Vec<ExchangeCode> {
        let mut v: Vec<_> = self.by_exchange.keys().copied().collect();
        v.sort();
        v
    }

    pub fn all(&self) -> Vec<Arc<dyn Adapter>> {
        let mut seen = Vec::new();
        for list in self.by_exchange.values() {
            for a in list {
                if !seen.iter().any(|x: &Arc<dyn Adapter>| x.id() == a.id()) {
                    seen.push(Arc::clone(a));
                }
            }
        }
        seen.sort_by_key(|a| a.id());
        seen
    }
}
