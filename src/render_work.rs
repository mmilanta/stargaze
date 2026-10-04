//! Bound submitted work and adapt sample batches to completed GPU timings.
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Default)]
struct SampleBudget {
    seconds_per_sample: Option<f64>,
    last_samples: u32,
}

impl SampleBudget {
    fn samples(&self, requested: u32, fps: u32) -> u32 {
        let Some(cost) = self.seconds_per_sample else {
            return 1;
        };
        let fps = if fps == 0 { 60 } else { fps };
        let target = (0.75 / fps as f64).clamp(0.003, 0.05);
        let affordable = (target / cost).floor().max(1.0) as u32;
        affordable
            .min(self.last_samples.saturating_mul(2).max(1))
            .min(requested.clamp(1, 64))
    }

    fn observe(&mut self, samples: u32, elapsed: Duration) {
        if samples == 0 {
            return; // Opaque UI and exposure-only redraws do not time tracing.
        }
        let cost = (elapsed.as_secs_f64() / samples as f64).max(0.0001);
        // Respond immediately to a slowdown; grow conservatively after it.
        self.seconds_per_sample = Some(match self.seconds_per_sample {
            Some(old) if cost < old => old * 0.75 + cost * 0.25,
            _ => cost,
        });
        self.last_samples = samples;
    }
}

struct Pending {
    samples: u32,
    completion: Receiver<Duration>,
}

#[derive(Default)]
pub(crate) struct Work {
    budget: SampleBudget,
    pending: Option<Pending>,
}

impl Work {
    pub fn reset_budget(&mut self) {
        self.budget = SampleBudget::default();
    }

    pub fn samples(&self, requested: u32, fps: u32) -> u32 {
        self.budget.samples(requested, fps)
    }

    fn finish_ready(&mut self) -> bool {
        let Some(pending) = self.pending.take() else {
            return true;
        };
        match pending.completion.try_recv() {
            Ok(elapsed) => self.budget.observe(pending.samples, elapsed),
            Err(TryRecvError::Empty) => {
                self.pending = Some(pending);
                return false;
            }
            Err(TryRecvError::Disconnected) => self.reset_budget(),
        }
        true
    }

    pub fn ready(&mut self, device: &wgpu::Device) -> bool {
        if self.pending.is_some() {
            // Poll callbacks without waiting on the GPU or blocking input.
            let _ = device.poll(wgpu::PollType::Poll);
        }
        self.finish_ready()
    }

    pub fn submitted(&mut self, queue: &wgpu::Queue, samples: u32, started: Instant) {
        debug_assert!(self.pending.is_none(), "only one frame may be in flight");
        let (send, completion) = mpsc::channel();
        queue.on_submitted_work_done(move || {
            let _ = send.send(started.elapsed());
        });
        self.pending = Some(Pending {
            samples,
            completion,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_requested_batches_start_small_and_slow_work_stays_small() {
        let mut work = Work::default();
        assert_eq!(work.samples(35, 15), 1);
        work.budget.observe(1, Duration::from_millis(350));
        assert_eq!(work.samples(35, 15), 1);
        // A fast, simple view may ramp up, but the next scene starts at one.
        work.reset_budget();
        for n in [1, 2, 4, 8, 16] {
            work.budget
                .observe(n, Duration::from_micros(n as u64 * 500));
        }
        assert_eq!(work.samples(35, 15), 32);
        work.budget.observe(32, Duration::from_millis(16));
        assert_eq!(work.samples(35, 15), 35);
        work.reset_budget();
        assert_eq!(work.samples(35, 15), 1);
    }

    #[test]
    fn batches_obey_the_cap_and_shrink_immediately_after_a_slow_frame() {
        let mut budget = SampleBudget::default();
        budget.observe(16, Duration::from_millis(16));
        assert_eq!(budget.samples(64, 60), 12);
        assert_eq!(budget.samples(64, 240), 3);
        assert_eq!(budget.samples(64, 0), 12);
        assert_eq!(budget.samples(4, 15), 4);
        budget.observe(16, Duration::from_millis(800));
        assert_eq!(budget.samples(64, 60), 1);
        budget.observe(0, Duration::ZERO);
        assert_eq!(budget.samples(64, 60), 1);
    }

    #[test]
    fn unfinished_work_defers_submission_without_waiting_and_ui_does_not_grow_batches() {
        let mut work = Work::default();
        let (send, completion) = mpsc::channel();
        work.pending = Some(Pending {
            samples: 1,
            completion,
        });
        assert!(!work.finish_ready());
        assert!(work.pending.is_some());
        send.send(Duration::from_millis(1)).unwrap();
        assert!(work.finish_ready());
        assert!(work.pending.is_none());
        assert_eq!(work.samples(35, 15), 2);
        let (send, completion) = mpsc::channel();
        work.pending = Some(Pending {
            samples: 0,
            completion,
        });
        send.send(Duration::from_micros(1)).unwrap();
        assert!(work.finish_ready());
        assert_eq!(work.samples(35, 15), 2);
    }

    #[test]
    #[ignore = "requires a GPU; checks actual queue completion callbacks"]
    fn gpu_submission_gate_completes_without_blocking_polls() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut work = Work::default();
        let encoder = device.create_command_encoder(&Default::default());
        let started = Instant::now();
        queue.submit(Some(encoder.finish()));
        work.submitted(&queue, 1, started);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(work.ready(&device));
        assert!(work.pending.is_none());
        assert!(work.budget.seconds_per_sample.is_some());
    }

    #[test]
    #[ignore = "requires a GPU; exercises the reported 35-sample level-entry setting"]
    fn gpu_level_entry_starts_small_and_refines_without_losing_samples() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        for name in ["puzzle", "halo"] {
            let mut tracer = crate::pathtracer::PathTracer::new(
                &device,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                (1, 1),
                &[],
                crate::pathtracer::Options {
                    samples_per_frame: 35,
                    max_bounces: 10,
                    sample_limit: 35,
                    auto_exposure: false,
                    ..Default::default()
                },
            );
            let scene = crate::config::load(format!(
                "{}/configs/{name}.yaml",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let mut frame = crate::State::with_scene(scene).build_frame(1, 1);
            let mut work = Work::default();
            for step in 0..35 {
                if tracer.samples() == 35 {
                    break;
                }
                assert!(work.ready(&device));
                if tracer.prepare_frame(&frame) {
                    work.reset_budget();
                }
                let batch = work.samples(35, 15);
                if step == 0 {
                    assert_eq!(batch, 1);
                }
                tracer.set_quality(batch, 10, 35);
                let before = tracer.samples();
                let mut encoder = device.create_command_encoder(&Default::default());
                tracer.encode(&queue, &mut encoder, &frame);
                let samples = tracer.samples() - before;
                assert!(samples > 0 && samples <= batch);
                let started = Instant::now();
                queue.submit(Some(encoder.finish()));
                work.submitted(&queue, samples, started);
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                assert!(work.ready(&device));
            }
            assert_eq!(tracer.samples(), 35);
            frame.scene_time += 1.0;
            assert!(tracer.prepare_frame(&frame));
            assert_eq!(tracer.samples(), 0);
            work.reset_budget();
            assert_eq!(work.samples(35, 15), 1);
        }
    }
}
