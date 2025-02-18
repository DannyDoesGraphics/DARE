use crate::prelude as dagal;
use anyhow::Result;
use ash::vk;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueueAllocatorError {
    #[error("Queue is busy")]
    BusyQueue,
    #[error("Queue requested is not possible")]
    ImpossibleRequest,
    #[error("Poison error")]
    PoisonError,
}

#[derive(Debug)]
pub struct QueueRequest<M: dagal::concurrency::Lockable<Target = vk::Queue>> {
    pub flags: vk::QueueFlags,
    pub min_count: Option<usize>,
    pub count: usize,
    /// Prefer queues which are not currently locked
    pub prefer_free: bool,
    pub _phantom_data: PhantomData<M>,
}

pub struct QueueAllocator<M: dagal::concurrency::Lockable<Target = vk::Queue>> {
    queues: Arc<[dagal::device::Queue<M>]>,
}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> From<Vec<dagal::device::Queue<M>>>
    for QueueAllocator<M>
{
    fn from(value: Vec<dagal::device::Queue<M>>) -> Self {
        Self {
            queues: Arc::from(value.into_boxed_slice()),
        }
    }
}
impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> From<Arc<[dagal::device::Queue<M>]>>
    for QueueAllocator<M>
{
    fn from(value: Arc<[dagal::device::Queue<M>]>) -> Self {
        Self { queues: value }
    }
}

impl<M: dagal::concurrency::Lockable<Target = vk::Queue>> QueueAllocator<M> {
    /// Attempts to retrieve arrays s.t. they fit
    ///
    /// To apply the exclusion mask, it assumes an array pair u32s (index, family_index)
    pub fn retrieve_queues(
        &self,
        exclusion_mask: &[(u32, u32)],
        queue_flags: vk::QueueFlags,
        count: usize,
    ) -> Result<Vec<dagal::device::Queue<M>>, QueueAllocatorError> {
        let exclude: HashSet<(u32, u32)> = exclusion_mask.iter().map(|(a, b)| (*a, *b)).collect();
        let mut n: usize = 0;
        let v: Vec<dagal::device::Queue<M>> = self
            .queues
            .iter()
            .filter_map(|queue| {
                if n < count
                    && !exclude.contains(&(queue.get_index(), queue.get_family_index()))
                    && queue.get_queue_flags().contains(queue_flags)
                {
                    n += 1;
                    Some(queue.clone())
                } else {
                    None
                }
            })
            .collect();
        if v.len() < count {
            Err(QueueAllocatorError::ImpossibleRequest)
        } else {
            Ok(v)
        }
    }

    /// Find as many queues that are "fit" the requirements
    ///
    /// To apply the exclusion mask, it assumes an array pair u32s (index, family_index)
    pub fn matching_queues(
        &self,
        exclusion_mask: &[(u32, u32)],
        queue_flags: vk::QueueFlags,
    ) -> usize {
        let exclude: HashSet<(u32, u32)> = exclusion_mask.iter().map(|(a, b)| (*a, *b)).collect();
        self.queues
            .iter()
            .filter_map(|queue| {
                if !exclude.contains(&(queue.get_index(), queue.get_family_index()))
                    && queue.get_queue_flags() & queue_flags == queue_flags
                {
                    Some(queue.clone())
                } else {
                    None
                }
            })
            .count()
    }
}

impl<M: dagal::concurrency::SyncLockable<Target = vk::Queue>> QueueAllocator<M> {
    pub fn acquire_queue<'a>(
        &'a self,
        request: QueueRequest<M>,
    ) -> Result<Vec<M::Lock<'a>>, QueueAllocatorError> {
        let mut out: Vec<M::Lock<'a>> =
            Vec::with_capacity(request.min_count.unwrap_or(request.count));
        for queue in self.queues.iter() {
            // stop if we reached requested amount
            if out.len() >= request.count {
                break;
            }
            if let Ok(guard) = queue.try_queue_lock() {
                out.push(guard);
            }
        }

        if out.len() < request.min_count.unwrap_or(request.count) {
            Err(QueueAllocatorError::BusyQueue)
        } else {
            Ok(out.into_iter().collect::<Vec<M::Lock<'a>>>())
        }
    }
}

impl<M: dagal::concurrency::AsyncLockable<Target = vk::Queue>> QueueAllocator<M> {
    pub async fn acquire_queue_async<'a>(
        &'a self,
        request: QueueRequest<M>,
    ) -> Result<Vec<M::Lock<'a>>, QueueAllocatorError> {
        let mut used_queues: HashSet<(u32, u32)> =
            HashSet::with_capacity(request.min_count.unwrap_or(request.count));
        let mut out: Vec<M::Lock<'a>> =
            Vec::with_capacity(request.min_count.unwrap_or(request.count));
        for queue in self.queues.iter() {
            // stop if we reached requested amount
            if out.len() >= request.count {
                break;
            }
            match queue.try_queue_lock() {
                Ok(guard) => {
                    used_queues.insert((queue.get_family_index(), queue.get_index()));
                    out.push(guard);
                }
                Err(_) => {
                    if !request.prefer_free {
                        if let Ok(guard) = queue.acquire_queue_async().await {
                            used_queues.insert((queue.get_family_index(), queue.get_index()));
                            out.push(guard);
                        }
                    }
                }
            }
        }
        // take the most available queues
        if out.len() < request.min_count.unwrap_or(request.count) {
            for queue in self.queues.iter() {
                // stop if we reached requested amount
                if out.len() >= request.count {
                    break;
                }
                if !used_queues.contains(&(queue.get_family_index(), queue.get_index())) {
                    if let Ok(guard) = queue.acquire_queue_async().await {
                        out.push(guard);
                        used_queues.insert((queue.get_family_index(), queue.get_index()));
                    }
                } else {
                    continue;
                }
            }
        }

        if out.len() < request.min_count.unwrap_or(request.count) {
            Err(QueueAllocatorError::BusyQueue)
        } else {
            Ok(out.into_iter().collect())
        }
    }

    pub fn acquire_queue_blocking<'a>(
        &'a self,
        request: QueueRequest<M>,
    ) -> Result<Vec<M::Lock<'a>>, QueueAllocatorError> {
        let mut used_queues: HashSet<(u32, u32)> =
            HashSet::with_capacity(request.min_count.unwrap_or(request.count));
        let mut out: Vec<M::Lock<'a>> =
            Vec::with_capacity(request.min_count.unwrap_or(request.count));
        for queue in self.queues.iter() {
            // stop if we reached requested amount
            if out.len() >= request.count {
                break;
            }
            match queue.try_queue_lock() {
                Ok(guard) => {
                    used_queues.insert((queue.get_family_index(), queue.get_index()));
                    out.push(guard);
                }
                Err(_) => {
                    if !request.prefer_free {
                        let guard = queue.acquire_queue_blocking();
                        used_queues.insert((queue.get_family_index(), queue.get_index()));
                        out.push(guard);
                    }
                }
            }
        }
        // take the most available queues
        if out.len() < request.min_count.unwrap_or(request.count) {
            for queue in self.queues.iter() {
                // stop if we reached requested amount
                if out.len() >= request.count {
                    break;
                }
                if !used_queues.contains(&(queue.get_family_index(), queue.get_index())) {
                    let guard = queue.acquire_queue_blocking();
                    out.push(guard);
                    used_queues.insert((queue.get_family_index(), queue.get_index()));
                } else {
                    continue;
                }
            }
        }

        if out.len() < request.min_count.unwrap_or(request.count) {
            Err(QueueAllocatorError::BusyQueue)
        } else {
            Ok(out.into_iter().collect())
        }
    }
}
