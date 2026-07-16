//! Vulkan-only timestamp validation.
//!
//! On macOS this requires MoltenVK. Metal is intentionally not used because
//! Metal 4 currently returns zero-filled timestamp query results through wgpu.

use astrelis_gpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, DeviceDescriptor, Features, MapMode,
    PollMode, QuerySetDescriptor, QueryType, RenderPassDescriptor, RenderPassTimestampWrites,
    RequestAdapterOptions,
};

#[test]
fn vulkan_timestamps_are_nonzero_and_ordered() {
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(astrelis_gpu_wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            use_environment: false,
        });
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) if std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_none() => {
                eprintln!(
                    "skipping Vulkan timestamp test ({error}); install MoltenVK on macOS or set \
                     ASTRELIS_REQUIRE_VULKAN_PROFILING=1 in Vulkan CI"
                );
                return;
            }
            Err(error) => panic!("Vulkan profiling adapter is required: {error}"),
        };
        assert_eq!(adapter.info().api, astrelis_gpu::GraphicsApi::Vulkan);
        if !adapter.features().contains(Features::TIMESTAMP_QUERY) {
            if std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_some() {
                panic!("Vulkan adapter does not support timestamp queries");
            }
            eprintln!("skipping Vulkan timestamp test: timestamp queries unsupported");
            return;
        }
        let (device, queue) = adapter
            .request_device(DeviceDescriptor {
                required_features: Features::TIMESTAMP_QUERY,
                ..Default::default()
            })
            .await
            .expect("request timestamp-capable Vulkan device");
        assert!(device.capabilities().reliable_timestamps);

        let queries = device.create_query_set(QuerySetDescriptor {
            label: Some("timestamp pair".into()),
            query_type: QueryType::Timestamp,
            count: 2,
        });
        let resolve = device.create_buffer(BufferDescriptor {
            label: Some("timestamp resolve".into()),
            size: 16,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(BufferDescriptor {
            label: Some("timestamp readback".into()),
            size: 16,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        encoder
            .render_pass(RenderPassDescriptor {
                label: Some("timestamped empty pass".into()),
                color_attachments: Vec::new(),
                timestamp_writes: Some(RenderPassTimestampWrites {
                    query_set: queries.clone(),
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            })
            .expect("record timestamp pass");
        encoder
            .resolve_query_set(&queries, 0..2, &resolve, 0)
            .expect("resolve timestamps");
        encoder
            .copy_buffer_to_buffer(&resolve, 0, &readback, 0, 16)
            .expect("copy timestamps");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");

        let mapping = readback.map_async(MapMode::Read, 0..16);
        device.poll(PollMode::Wait).expect("wait for timestamps");
        mapping.await.expect("map timestamp results");
        let bytes = readback.read_mapped(0..16).expect("read timestamps");
        readback.unmap();
        let start = u64::from_le_bytes(bytes[0..8].try_into().expect("start bytes"));
        let end = u64::from_le_bytes(bytes[8..16].try_into().expect("end bytes"));
        if start == 0 && std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_none() {
            eprintln!(
                "skipping Vulkan timestamp assertion: the installed Vulkan/MoltenVK driver \
                 returned zero timestamps"
            );
            return;
        }
        assert_ne!(start, 0, "Vulkan returned a zero start timestamp");
        assert!(end >= start, "timestamp pair is not ordered");
        assert!(queue.timestamp_period() > 0.0);
    });
}

#[test]
fn vulkan_profiler_reports_a_queue_lane() {
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(astrelis_gpu_wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            use_environment: false,
        });
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) if std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_none() => {
                eprintln!("skipping Vulkan profiler test: {error}");
                return;
            }
            Err(error) => panic!("Vulkan profiling adapter is required: {error}"),
        };
        let required = Features::TIMESTAMP_QUERY | Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !adapter.features().contains(required) {
            if std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_some() {
                panic!("Vulkan adapter lacks encoder timestamp queries");
            }
            eprintln!("skipping Vulkan profiler test: encoder timestamps unsupported");
            return;
        }
        let (device, queue) = adapter
            .request_device(DeviceDescriptor {
                required_features: required,
                ..Default::default()
            })
            .await
            .expect("request profiling device");
        let mut profiler = match astrelis_gpu_wgpu::WgpuGpuProfiler::new(
            &device,
            &queue,
            Some("Vulkan test queue"),
        ) {
            Ok(profiler) => profiler,
            Err(error) if std::env::var_os("ASTRELIS_REQUIRE_VULKAN_PROFILING").is_none() => {
                eprintln!("skipping Vulkan profiler test: {error}");
                return;
            }
            Err(error) => panic!("create profiler: {error}"),
        };
        let lane = profiler.lane();
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        profiler
            .scope(
                "profiled encoder scope",
                &mut encoder,
                |_profiler, encoder| {
                    encoder.push_debug_group("some work");
                    encoder.pop_debug_group();
                },
            )
            .expect("record profiler scope");
        profiler
            .resolve_frame(&mut encoder)
            .expect("resolve profiler frame");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit profiler frame");
        profiler.end_frame().expect("end profiler frame");
        device
            .poll(PollMode::Wait)
            .expect("wait for profiler frame");
        assert_eq!(
            profiler
                .process_finished_frames(&device, &queue)
                .expect("process profiler frame"),
            1
        );
        let timeline = astrelis_profiling::Profiler::get()
            .timeline
            .read()
            .expect("timeline poisoned");
        let stream = timeline
            .gpu_streams
            .get(&lane)
            .expect("registered queue lane");
        assert!(
            !stream.spans.is_empty(),
            "profiled scope was not forwarded to the timeline"
        );
    });
}
