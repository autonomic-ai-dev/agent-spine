//! Speculative parallel hydrate: fan-out context probes, fan-in first success (Phase 9 P2).

use std::future::Future;
use std::pin::Pin;

pub type HydrateProbe = Pin<Box<dyn Future<Output = Option<String>> + Send>>;

#[derive(Debug, Clone)]
pub struct HydratePlan {
    pub probes: Vec<&'static str>,
}

impl Default for HydratePlan {
    fn default() -> Self {
        Self {
            probes: vec!["git_diff", "lint", "tests", "relevant_files"],
        }
    }
}

pub async fn run_speculative_hydrate(probes: Vec<HydrateProbe>) -> Vec<String> {
    if probes.is_empty() {
        return Vec::new();
    }
    let mut handles = Vec::with_capacity(probes.len());
    for probe in probes {
        handles.push(tokio::spawn(async move { probe.await }));
    }
    let mut out = Vec::new();
    for handle in handles {
        if let Ok(Some(chunk)) = handle.await {
            out.push(chunk);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collects_parallel_probe_outputs() {
        let probes = vec![
            Box::pin(async { Some("lint: ok".into()) }) as HydrateProbe,
            Box::pin(async { None }) as HydrateProbe,
            Box::pin(async { Some("tests: 42 passed".into()) }) as HydrateProbe,
        ];
        let chunks = run_speculative_hydrate(probes).await;
        assert_eq!(chunks.len(), 2);
    }
}
