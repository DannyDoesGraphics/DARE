use anyhow::Result;
use ash::vk;

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
}

impl QueueRequest {
    pub fn new(family_flags: vk::QueueFlags, count: u32, dedicated: bool) -> Self {
        Self {
            family_flags,
            count,
            dedicated,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QueueFamily {
    /// We assume from [free_index, family.queues] are free
    free_index: u32,
    family: vk::QueueFamilyProperties,
    family_index: usize,
}

/// Determine the correct slotting of queues
///
/// Returns a vector containing a 1:1 mapping to the [`queue_requests`] parameter
pub(crate) fn determine_queue_slotting(
    queue_families: Vec<vk::QueueFamilyProperties>,
    queue_requests: Vec<QueueRequest>,
) -> Result<Vec<Vec<QueueAllocation>>> {
    if queue_requests.is_empty() {
        return Ok(Vec::new());
    }
    //let (mut dedicated, mut non_dedicated): (Vec<crate::bootstrap::RequestedQueue>, Vec<crate::bootstrap::RequestedQueue>) = queue_requests.into_iter().partition(|queue| queue.dedicated);
    // A pair set to queue_families but contains lists of allocations
    let mut allocations: Vec<Vec<QueueAllocation>> = Vec::new();
    allocations.resize(queue_requests.len(), Vec::new());

    let mut queue_families = queue_families
        .iter()
        .enumerate()
        .map(|(family_index, family)| QueueFamily {
            free_index: 0,
            family: *family,
            family_index,
        })
        .collect::<Vec<QueueFamily>>();
    queue_requests
        .iter()
        .map(|request| {
            // recursively find suitable families
            let mut remaining_queues = request.count;
            let mut suitable_families: Vec<QueueAllocation> = Vec::new();
            while remaining_queues > 0 {
                // amount taken from the family
                let suitable_family = queue_families.iter_mut().find_map(|family| {
                    if family.family.queue_flags & request.family_flags == request.family_flags
                        && family.free_index < family.family.queue_count
                    {
                        // do not take more queues than what exists or what we need
                        let take_amount = request
                            .count
                            .min(family.family.queue_count - family.free_index)
                            .min(remaining_queues);
                        family.free_index += take_amount;
                        remaining_queues -= take_amount;
                        Some(QueueAllocation {
                            family_index: family.family_index as u32,
                            index: family.free_index - take_amount,
                            count: take_amount,
                            family_flags: family.family.queue_flags,
                        })
                    } else {
                        None
                    }
                });
                match suitable_family {
                    None => return Err(anyhow::Error::from(crate::DagalError::ImpossibleQueue)),
                    Some(family) => {
                        suitable_families.push(family);
                    }
                }
            }
            Ok(suitable_families)
        })
        .collect::<Result<Vec<Vec<QueueAllocation>>>>()
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
                family_flags: vk::QueueFlags::COMPUTE,
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
                family_flags: vk::QueueFlags::COMPUTE,
            }
        );
        assert_eq!(
            allocations.get(1).unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 0,
                index: 3,
                count: 1,
                family_flags: vk::QueueFlags::COMPUTE,
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
                family_flags: vk::QueueFlags::COMPUTE,
            }
        );
        assert_eq!(
            allocations.first().unwrap().get(1).unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 0,
                count: 1,
                family_flags: vk::QueueFlags::COMPUTE,
            }
        );
        assert_eq!(
            allocations.get(1).unwrap().first().unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 1,
                count: 1,
                family_flags: vk::QueueFlags::COMPUTE,
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
                family_flags: vk::QueueFlags::COMPUTE,
            }
        );
        assert_eq!(
            allocations.first().unwrap().get(1).unwrap().clone(),
            super::QueueAllocation {
                family_index: 2,
                index: 0,
                count: 1,
                family_flags: vk::QueueFlags::GRAPHICS,
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
