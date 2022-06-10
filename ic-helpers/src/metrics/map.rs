#[derive(Clone)]
pub struct MetricsMap<T> {
    interval_hours: u64,
    pub map: std::collections::BTreeMap<u64, T>,
}

impl<T> MetricsMap<T> {
    pub fn new<const INTERVAL: u64>() -> Self {
        Self {
            interval_hours: INTERVAL,
            map: std::collections::BTreeMap::new(),
        }
    }

    pub fn get_interval(&self) -> u64 {
        self.interval_hours
    }

    pub fn insert(&mut self, new_metric: T) -> Option<T> {
        let current_ts = ic_kit::ic::time() / (6u64.pow(10) * 60);
        let last_ts = self
            .map
            .iter()
            .next_back()
            .map(|(k, _)| *k)
            .unwrap_or(current_ts);
        let new_ts = if current_ts < last_ts + self.interval_hours {
            last_ts
        } else {
            current_ts - (current_ts % self.interval_hours)
        };
        self.map.insert(new_ts, new_metric)
    }
}

impl<T> std::default::Default for MetricsMap<T> {
    fn default() -> Self {
        Self {
            interval_hours: 1,
            map: std::collections::BTreeMap::new(),
        }
    }
}
