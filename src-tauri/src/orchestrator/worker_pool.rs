// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AiImp Development
// Public API — consumed via neuralswarm Tauri bridge + tests
#![allow(dead_code)]
//! Worker Pool Isolation for ImpForge Orchestrator
//!
//! Semaphore-based concurrency limits per resource pool.
//! Prevents resource contention (e.g., multiple GPU workers fighting
//! for VRAM, or too many shell processes overwhelming the system).
//!
//! Pool types:
//! - **cpu**: CPU-bound analysis tasks (max 4 concurrent)
//! - **gpu**: GPU-accelerated inference (max 2 concurrent, VRAM-limited)
//! - **shell**: External process spawning (max 3 concurrent)
//! - **embed**: Embedding generation (max 1 concurrent, model loading)
//!
//! Scientific basis: OS-level resource scheduling with cooperative
//! semaphores (Dijkstra 1965), adapted for async Tokio runtime.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Configuration for worker pool concurrency limits.
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    pub cpu_max: usize,
    pub gpu_max: usize,
    pub shell_max: usize,
    pub embed_max: usize,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            cpu_max: 4,
            gpu_max: 2,
            shell_max: 3,
            embed_max: 1,
        }
    }
}

/// Pool manager holding semaphores per resource type.
///
/// Usage: `pool.acquire("gpu").await` blocks if the GPU pool is full.
/// The returned permit is dropped when the worker finishes, releasing
/// the slot for the next worker.
pub struct WorkerPool {
    semaphores: HashMap<String, Arc<Semaphore>>,
    config: WorkerPoolConfig,
}

impl WorkerPool {
    /// Create a new pool manager with the given config.
    pub fn new(config: WorkerPoolConfig) -> Self {
        let mut semaphores = HashMap::new();
        semaphores.insert("cpu".to_string(), Arc::new(Semaphore::new(config.cpu_max)));
        semaphores.insert("gpu".to_string(), Arc::new(Semaphore::new(config.gpu_max)));
        semaphores.insert("shell".to_string(), Arc::new(Semaphore::new(config.shell_max)));
        semaphores.insert("embed".to_string(), Arc::new(Semaphore::new(config.embed_max)));
        Self { semaphores, config }
    }

    /// Try to acquire a pool permit without blocking.
    ///
    /// Returns `Some(permit)` if a slot is available, `None` if the pool is full.
    /// The caller must hold the permit for the duration of the worker execution.
    pub fn try_acquire(&self, pool: &str) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.semaphores
            .get(pool)
            .and_then(|sem| sem.clone().try_acquire_owned().ok())
    }

    /// Get the number of available permits for a pool.
    pub fn available(&self, pool: &str) -> usize {
        self.semaphores
            .get(pool)
            .map(|sem| sem.available_permits())
            .unwrap_or(0)
    }

    /// Get the maximum capacity for a pool.
    pub fn capacity(&self, pool: &str) -> usize {
        match pool {
            "cpu" => self.config.cpu_max,
            "gpu" => self.config.gpu_max,
            "shell" => self.config.shell_max,
            "embed" => self.config.embed_max,
            _ => 0,
        }
    }

    /// Get current utilization (active / capacity) for all pools.
    pub fn utilization(&self) -> HashMap<String, (usize, usize)> {
        self.semaphores
            .iter()
            .map(|(name, sem)| {
                let cap = self.capacity(name);
                let active = cap - sem.available_permits();
                (name.clone(), (active, cap))
            })
            .collect()
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new(WorkerPoolConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WorkerPoolConfig::default();
        assert_eq!(config.cpu_max, 4);
        assert_eq!(config.gpu_max, 2);
        assert_eq!(config.shell_max, 3);
        assert_eq!(config.embed_max, 1);
    }

    #[test]
    fn test_pool_creation() {
        let pool = WorkerPool::default();
        assert_eq!(pool.available("cpu"), 4);
        assert_eq!(pool.available("gpu"), 2);
        assert_eq!(pool.available("shell"), 3);
        assert_eq!(pool.available("embed"), 1);
        assert_eq!(pool.available("unknown"), 0);
    }

    #[test]
    fn test_try_acquire_and_release() {
        let pool = WorkerPool::default();
        assert_eq!(pool.available("embed"), 1);

        // Acquire the only embed slot
        let permit = pool.try_acquire("embed");
        assert!(permit.is_some());
        assert_eq!(pool.available("embed"), 0);

        // Second acquire should fail (pool full)
        let permit2 = pool.try_acquire("embed");
        assert!(permit2.is_none());

        // Drop the permit → slot released
        drop(permit);
        assert_eq!(pool.available("embed"), 1);
    }

    #[test]
    fn test_gpu_pool_limits() {
        let pool = WorkerPool::default();

        let p1 = pool.try_acquire("gpu").unwrap();
        let p2 = pool.try_acquire("gpu").unwrap();
        assert_eq!(pool.available("gpu"), 0);

        // Third GPU worker should be rejected
        assert!(pool.try_acquire("gpu").is_none());

        drop(p1);
        assert_eq!(pool.available("gpu"), 1);
        drop(p2);
        assert_eq!(pool.available("gpu"), 2);
    }

    #[test]
    fn test_cpu_pool_four_slots() {
        let pool = WorkerPool::default();
        let mut permits = Vec::new();

        for i in 0..4 {
            let p = pool.try_acquire("cpu");
            assert!(p.is_some(), "Failed to acquire CPU slot {}", i);
            permits.push(p.unwrap());
        }

        assert!(pool.try_acquire("cpu").is_none());
        assert_eq!(pool.available("cpu"), 0);

        drop(permits);
        assert_eq!(pool.available("cpu"), 4);
    }

    #[test]
    fn test_utilization() {
        let pool = WorkerPool::default();
        let _p1 = pool.try_acquire("cpu").unwrap();
        let _p2 = pool.try_acquire("gpu").unwrap();

        let util = pool.utilization();
        assert_eq!(util["cpu"], (1, 4));
        assert_eq!(util["gpu"], (1, 2));
        assert_eq!(util["shell"], (0, 3));
        assert_eq!(util["embed"], (0, 1));
    }

    #[test]
    fn test_custom_config() {
        let config = WorkerPoolConfig {
            cpu_max: 8,
            gpu_max: 4,
            shell_max: 6,
            embed_max: 2,
        };
        let pool = WorkerPool::new(config);
        assert_eq!(pool.available("cpu"), 8);
        assert_eq!(pool.available("gpu"), 4);
        assert_eq!(pool.capacity("cpu"), 8);
    }

    #[test]
    fn test_unknown_pool() {
        let pool = WorkerPool::default();
        assert!(pool.try_acquire("nonexistent").is_none());
        assert_eq!(pool.capacity("nonexistent"), 0);
    }
}
