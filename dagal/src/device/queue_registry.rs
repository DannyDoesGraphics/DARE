use ash::vk;

use crate::DagalError;
use crate::device::{LogicalDevice, PhysicalDevice, Queue};

/// Named device queues assigned at init.
#[derive(Debug)]
pub struct QueueRegistry {
    pub present: Queue,
    pub transfer: Option<Queue>,
    pub spare: Vec<Queue>,
}

impl QueueRegistry {
    pub fn from_device(
        device: &LogicalDevice,
        physical_device: &PhysicalDevice,
    ) -> crate::Result<Self> {
        let queues = physical_device
            .get_active_queues()
            .iter()
            .map(|queue_info| {
                let queue_ci: vk::DeviceQueueInfo2<'_> = (*queue_info).into();
                unsafe {
                    device.get_queue(
                        &queue_ci,
                        queue_info.queue_flags,
                        queue_info.strict,
                        queue_info.can_present,
                    )
                }
            })
            .collect();
        Self::from_queues(queues)
    }

    pub fn from_queues(mut queues: Vec<Queue>) -> crate::Result<Self> {
        if queues.is_empty() {
            return Err(DagalError::ImpossibleQueue.into());
        }
        let present_idx = queues.iter().position(|q| q.can_present()).unwrap_or(0);
        let present = queues.remove(present_idx);

        let transfer_idx = queues
            .iter()
            .enumerate()
            .filter(|(_, queue)| queue.get_queue_flags().contains(vk::QueueFlags::TRANSFER))
            .min_by_key(|(_, queue)| queue.get_queue_flags().as_raw().count_ones())
            .map(|(index, _)| index);
        let transfer = transfer_idx.map(|i| queues.remove(i));

        Ok(Self {
            present,
            transfer,
            spare: queues,
        })
    }

    pub fn take_transfer(&mut self) -> crate::Result<Queue> {
        self.transfer.take().ok_or(DagalError::ImpossibleQueue)
    }

    /// Take an unassigned queue supporting every flag in `flags`.
    pub fn take_spare(&mut self, flags: vk::QueueFlags) -> crate::Result<Queue> {
        let index = self
            .spare
            .iter()
            .enumerate()
            .filter(|(_, queue)| queue.get_queue_flags().contains(flags))
            .min_by_key(|(_, queue)| queue.get_queue_flags().as_raw().count_ones())
            .map(|(index, _)| index)
            .ok_or(DagalError::ImpossibleQueue)?;
        Ok(self.spare.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use ash::vk;

    use crate::bootstrap::app_info::{Expected, QueueRequest};
    use crate::util::tests::TestHarness;

    fn multi_family_harness() -> TestHarness {
        TestHarness::headless()
            .expect_queues(&[
                QueueRequest {
                    strict: false,
                    queue_type: vec![Expected::Required(
                        vk::QueueFlags::GRAPHICS
                            | vk::QueueFlags::TRANSFER
                            | vk::QueueFlags::COMPUTE,
                    )]
                    .into(),
                    count: Expected::Required(2),
                },
                QueueRequest {
                    strict: false,
                    queue_type: vec![Expected::Required(vk::QueueFlags::TRANSFER)].into(),
                    count: Expected::Preferred(u32::MAX),
                },
            ])
            .build()
            .unwrap()
    }

    #[test]
    fn transfer_prefers_a_dedicated_copy_queue() {
        let harness = multi_family_harness();
        let queues = harness.physical_device().get_active_queues();
        let registry =
            super::QueueRegistry::from_device(&harness.device(), harness.physical_device())
                .unwrap();

        let transfer = registry
            .transfer
            .as_ref()
            .expect("no transfer queue assigned");
        let chosen = transfer.get_queue_flags().as_raw().count_ones();
        let most_specialized = queues
            .iter()
            .filter(|info| info.queue_flags.contains(vk::QueueFlags::TRANSFER))
            .map(|info| info.queue_flags.as_raw().count_ones())
            .min()
            .unwrap();

        assert_eq!(
            chosen,
            most_specialized,
            "transfer got a {:?} queue while a more specialized one was available",
            transfer.get_queue_flags()
        );
    }

    #[test]
    fn take_spare_respects_requested_flags() {
        let harness = multi_family_harness();
        let mut registry =
            super::QueueRegistry::from_device(&harness.device(), harness.physical_device())
                .unwrap();

        let compute = registry.take_spare(vk::QueueFlags::COMPUTE).unwrap();
        assert!(compute.get_queue_flags().contains(vk::QueueFlags::COMPUTE));
    }

    #[test]
    fn take_spare_prefers_the_most_specialized_match() {
        let harness = multi_family_harness();
        let mut registry =
            super::QueueRegistry::from_device(&harness.device(), harness.physical_device())
                .unwrap();

        let wanted = vk::QueueFlags::COMPUTE | vk::QueueFlags::TRANSFER;
        let available = registry
            .spare
            .iter()
            .filter(|queue| queue.get_queue_flags().contains(wanted))
            .map(|queue| queue.get_queue_flags().as_raw().count_ones())
            .min()
            .expect("no spare queue satisfies the request");

        let taken = registry.take_spare(wanted).unwrap();

        assert_eq!(
            taken.get_queue_flags().as_raw().count_ones(),
            available,
            "took a {:?} queue while a more specialized one was available",
            taken.get_queue_flags()
        );
    }

    #[test]
    fn take_spare_hands_each_queue_out_once() {
        let harness = multi_family_harness();
        let mut registry =
            super::QueueRegistry::from_device(&harness.device(), harness.physical_device())
                .unwrap();

        let mut taken = Vec::new();
        while let Ok(queue) = registry.take_spare(vk::QueueFlags::empty()) {
            taken.push(queue.get_info());
        }
        let unique: std::collections::HashSet<(u32, u32)> = taken
            .iter()
            .map(|info| (info.family_index, info.index))
            .collect();

        assert!(!taken.is_empty(), "expected spare queues to exist");
        assert_eq!(unique.len(), taken.len(), "a queue was vended twice");
    }
}
