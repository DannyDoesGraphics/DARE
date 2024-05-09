use ash::vk;
use std::cell::RefCell;
use std::rc::Rc;

use crate::bootstrap::QueueAllocation;

/// Queue request struct
#[derive(Debug, Clone)]
pub struct QueueRequest {
    /// Requested flags the queue has
    pub family_flags: vk::QueueFlags,

    /// Number of queue to the requested
    pub count: u32,

    /// Whether the queue requested must be dedicated
    pub dedicated: bool,

    pub(crate) queues: Vec<crate::device::Queue>,
}

impl QueueRequest {
    pub fn new(family_flags: vk::QueueFlags, count: u32, dedicated: bool) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            family_flags,
            count,
            dedicated,
            queues: Vec::new(),
        }))
    }

    /// Retrieve queues that have been created
    ///
    /// Should only be used after [`bootstrap::LogicalDeviceBuilder`] has built the [`LogicalDevice`](crate::device::LogicalDevice)
    pub fn get_queues(&self) -> &[crate::device::Queue] {
        self.queues.as_slice()
    }
}

/// Determine the correct slotting of queues
pub(crate) fn determine_queue_slotting(
    queue_families: Vec<vk::QueueFamilyProperties>,
    queue_requests: Vec<Rc<RefCell<QueueRequest>>>,
) -> anyhow::Result<Vec<Vec<QueueAllocation>>> {
    if queue_requests.is_empty() {
        return Ok(Vec::new());
    }
    // (claimed_by_dedicated, claimed_by_non_dedicated)
    let mut queue_families: Vec<(u32, u32, vk::QueueFamilyProperties)> = queue_families
        .into_iter()
        .map(|queue| (0u32, 0u32, queue))
        .collect();
    //let (mut dedicated, mut non_dedicated): (Vec<crate::bootstrap::RequestedQueue>, Vec<crate::bootstrap::RequestedQueue>) = queue_requests.into_iter().partition(|queue| queue.dedicated);
    // A pair set to queue_families but contains lists of allocations
    let mut allocations: Vec<Vec<QueueAllocation>> = Vec::new();
    allocations.resize(queue_requests.len(), Vec::new());
    let mut counts: Vec<u64> = queue_requests
        .iter()
        .map(|queue| queue.borrow().count as u64)
        .collect();

    // First, allocate dedicated queues
    for (queue_index, (queue, queue_count)) in
        queue_requests.iter().zip(counts.iter_mut()).enumerate()
    {
        let queue = queue.borrow();
        if !queue.dedicated {
            unimplemented!();
        }
        for (family_index, (dedicated_claim, non_dedicated_claim, queue_family)) in queue_families
            .iter_mut()
            .enumerate()
            .filter(|(_, (_, _, queue_family))| {
                queue_family.queue_flags & queue.family_flags == queue.family_flags
            })
        {
            let taken_slots: u64 = *dedicated_claim as u64 + *non_dedicated_claim as u64;
            let free_slots: u64 = (queue_family.queue_count as u64 - taken_slots)
                .max(0)
                .min(*queue_count); // # of slots that are free for a queue
            if free_slots > 0 {
                *dedicated_claim += free_slots as u32; // claim
                *queue_count -= free_slots;
                // allocate
                allocations
                    .get_mut(queue_index)
                    .unwrap()
                    .push(QueueAllocation {
                        family_index: family_index as u32,
                        index: taken_slots as u32,
                        count: free_slots as u32,
                    });
            }
            if *queue_count == 0 {
                break;
            }
        }
        if *queue_count > 0 {
            return Err(anyhow::Error::from(
                crate::error::DagalError::ImpossibleQueue,
            )); // Impossible queue was allocated
        }
    }

    Ok(allocations)
}

#[cfg(test)]
mod test {

    use ash::vk;

    fn generate_queue_families() -> Vec<vk::QueueFamilyProperties> {
        vec![
            vk::QueueFamilyProperties {
                queue_flags: vk::QueueFlags::COMPUTE,
                queue_count: 4,
                timestamp_valid_bits: 0,
                min_image_transfer_granularity: Default::default(),
            },
            vk::QueueFamilyProperties {
                queue_flags: vk::QueueFlags::GRAPHICS,
                queue_count: 1,
                timestamp_valid_bits: 0,
                min_image_transfer_granularity: Default::default(),
            },
            vk::QueueFamilyProperties {
                queue_flags: vk::QueueFlags::COMPUTE,
                queue_count: 4,
                timestamp_valid_bits: 0,
                min_image_transfer_granularity: Default::default(),
            },
            vk::QueueFamilyProperties {
                queue_flags: vk::QueueFlags::GRAPHICS,
                queue_count: 0,
                timestamp_valid_bits: 0,
                min_image_transfer_granularity: Default::default(),
            },
        ]
    }

    #[test]
    fn queue_allocation_single() {
        let queue_families = generate_queue_families();
        // Test case with 1 queue flag
        let queue_requests = vec![super::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)];
        let allocations = super::determine_queue_slotting(queue_families, queue_requests).unwrap();
        assert!(allocations.first().is_some());
        assert_eq!(allocations.first().unwrap().len(), 1);
        assert_eq!(
            allocations.first().unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 0,
                count: 1,
            }
        );
    }

    #[test]
    fn queue_allocation_single_offset() {
        use ash::vk;
        let queue_families = generate_queue_families();
        // Test multiple queues in one family
        let queue_requests = vec![
            super::QueueRequest::new(vk::QueueFlags::COMPUTE, 3, true),
            super::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true),
        ];
        let allocations = super::determine_queue_slotting(queue_families, queue_requests).unwrap();
        assert!(allocations.first().is_some());
        assert_eq!(allocations.len(), 2);
        assert_eq!(
            allocations.first().unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 0,
                count: 3,
            }
        );
        assert_eq!(
            allocations.get(1).unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 3,
                count: 1,
            }
        );
    }

    #[test]
    fn queue_allocation_offsets() {
        use ash::vk;
        let queue_families = generate_queue_families();
        // Test case with 1 queue flag
        let queue_requests = vec![
            super::QueueRequest::new(vk::QueueFlags::COMPUTE, 5, true),
            super::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true),
        ];
        // Test if it is possible to create queues across different families
        let allocations = super::determine_queue_slotting(queue_families, queue_requests).unwrap();
        assert!(allocations.first().is_some());
        assert_eq!(allocations.len(), 2);
        assert_eq!(
            allocations.first().unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 0,
                count: 4,
            }
        );
        assert_eq!(
            allocations.first().unwrap().get(1).unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 0,
                count: 1,
            }
        );
        assert_eq!(
            allocations.get(1).unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 1,
                count: 1,
            }
        );
    }

    #[test]
    fn queue_allocation_offsets_differ() {
        use ash::vk;
        let queue_families = generate_queue_families();
        // Test case with 1 queue flag
        let queue_requests = vec![
            super::QueueRequest::new(vk::QueueFlags::COMPUTE, 5, true),
            super::QueueRequest::new(vk::QueueFlags::GRAPHICS, 1, true),
        ];
        // Test if it is possible to create queues across different families
        let allocations = super::determine_queue_slotting(queue_families, queue_requests).unwrap();
        assert!(allocations.first().is_some());
        assert_eq!(allocations.len(), 2);
        assert_eq!(
            allocations.first().unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 0,
                count: 4,
            }
        );
        assert_eq!(
            allocations.first().unwrap().get(1).unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 0,
                count: 1,
            }
        );
        assert_eq!(
            allocations.get(1).unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 1,
                index: 0,
                count: 1,
            }
        );
    }

    #[test]
    fn queue_allocation_single_impossible() {
        use ash::vk;
        let queue_families = generate_queue_families();
        // Expect an impossible allocation
        let queue_requests = vec![super::QueueRequest::new(vk::QueueFlags::COMPUTE, 10, true)];
        let allocations = super::determine_queue_slotting(queue_families, queue_requests);
        assert!(allocations.is_err());
    }
}
