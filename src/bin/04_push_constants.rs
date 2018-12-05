extern crate haltut;

use haltut::backend;
use haltut::prelude::*;
use haltut::utils;

use std::time::Instant;

static WIN_TITLE : &'static str = "Part 04: Push Constants";

#[cfg(windows)]
static VERT_SPIRV : &'static [u8] = include_bytes!("..\\..\\assets\\gen\\shaders\\part04.vert.spv");
#[cfg(windows)]
static FRAG_SPIRV : &'static [u8] = include_bytes!("..\\..\\assets\\gen\\shaders\\part04.frag.spv");

#[cfg(all(unix))]
static VERT_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part04.vert.spv");
#[cfg(all(unix))]
static FRAG_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part04.frag.spv");

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    colour: [f32; 4]
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct UniformBlock {
    projection: [[f32; 4]; 4]
}

// Push constants are a simpler and faster way of providing data
// to shaders than through the more complex bindings introduced
// thus far. The traadeoff is limited size: the spec only mandates
// that each shader stage should support 128 bytes of push constant
// data (TODO: check this).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct PushConstants {
    tint: [f32; 4],
    position: [f32; 3],
}


const MESH: &[Vertex] = &[
    Vertex {
        position: [0.0, -1.0, 0.0],
        colour: [1.0, 0.0, 0.0, 1.0],
    },
    Vertex {
        position: [-1.0, 0.0, 0.0],
        colour: [0.0, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0, 0.0],
        colour: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.0, -1.0, 0.0],
        colour: [1.0, 0.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0, 0.0],
        colour: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 0.0, 0.0],
        colour: [1.0, 1.0, 0.0, 1.0],
    },
];


fn main() {
    let mut events_loop = EventsLoop::new();
    let window = WindowBuilder::new()
                    .with_title(WIN_TITLE)
                    .with_dimensions((640, 480).into())
                    .with_decorations(true)
                    .build(&events_loop)
                    .unwrap();

    let instance = backend::Instance::create(WIN_TITLE, 1);

    let mut surface = instance.create_surface(&window);
    let mut adapter = instance.enumerate_adapters().remove(0);
    let (device, mut queue_group) = adapter
        .open_with::<_, Graphics>(1, |family| surface.supports_queue_family(family))
        .unwrap();

    let mut command_pool = device.create_command_pool_typed(&queue_group,
                                                            CommandPoolCreateFlags::empty(),
                                                            16);

    let physical_device = &adapter.physical_device;

    let (_caps, formats, _) = surface.compatibility(physical_device);

    let surface_colour_format = {
        match formats {
            Some(choices) => choices.into_iter()
                                    .find(|format| format.base_format().1 == ChannelType::Srgb)
                                    .unwrap(),
            None => Format::Rgba8Srgb,
        }
    };

    let render_pass = {
        let colour_attachment = Attachment {
            format: Some(surface_colour_format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present
        };

        let subpass = SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            preserves: &[],
            resolves: &[]
        };

        let dependency = SubpassDependency {
            passes: SubpassRef::External..SubpassRef::Pass(0),
            stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            accesses: Access::empty()..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE),
        };

        device.create_render_pass(&[colour_attachment], &[subpass], &[dependency])
    };


    let set_layout = device.create_descriptor_set_layout(
        &[DescriptorSetLayoutBinding {
            binding: 0,
            ty: DescriptorType::UniformBuffer,
            count: 1,
            stage_flags: ShaderStageFlags::VERTEX,
            immutable_samplers: false,
        }],
        &[],
    );

    // We need to do a little transmutation later; push constants
    // are u32, and we need to work out the size in 'push constants'
    // both here for the pipeline layout, and later to unsafe-cast
    // and upload the data for the draw-call.
    let num_push_constants = {
        let size_in_bytes = std::mem::size_of::<PushConstants>();
        let size_of_push_constant = std::mem::size_of::<u32>();
        size_in_bytes / size_of_push_constant
    };

    // As with the descriptor set layout, we add our push constants
    // to the pipeline layout.
    let pipeline_layout = device.create_pipeline_layout(
        vec![&set_layout],
        &[(ShaderStageFlags::VERTEX, 0..(num_push_constants as u32))]
    );

    let vertex_shader_module = device.create_shader_module(VERT_SPIRV).unwrap();
    let fragment_shader_module = device.create_shader_module(FRAG_SPIRV).unwrap();


    let pipeline = {
        let vs_entry = EntryPoint::<backend::Backend> {
            entry: "main",
            module: &vertex_shader_module,
            specialization: Default::default(),
        };

        let fs_entry = EntryPoint::<backend::Backend> {
            entry: "main",
            module: &fragment_shader_module,
            specialization: Default::default(),
        };

        let shader_entries = GraphicsShaderSet {
            vertex: vs_entry,
            hull: None,
            domain: None,
            geometry: None,
            fragment: Some(fs_entry),
        };

        let subpass = Subpass {
            index: 0,
            main_pass: &render_pass
        };

        let mut pipeline_desc = GraphicsPipelineDesc::new(shader_entries,
                                                          Primitive::TriangleList,
                                                          Rasterizer::FILL,
                                                          &pipeline_layout,
                                                          subpass);

        pipeline_desc.blender
                     .targets
                     .push(ColorBlendDesc(ColorMask::ALL, BlendState::ALPHA));

        pipeline_desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: std::mem::size_of::<Vertex>() as u32,
            rate: 0
        });
        pipeline_desc.attributes.push(AttributeDesc {
            location: 0,
            binding: 0,
            element: Element {
                format: Format::Rgb32Float,
                offset: 0
            }
        });
        pipeline_desc.attributes.push(AttributeDesc {
            location: 1,
            binding: 0,
            element: Element {
                format: Format::Rgba32Float,
                offset: 12
            }
        });

        device.create_graphics_pipeline(&pipeline_desc, None)
              .unwrap()
    };

    let mut desc_pool = device.create_descriptor_pool(
        1,
        &[DescriptorRangeDesc {
            ty: DescriptorType::UniformBuffer,
            count: 1
        }]
    );

    let desc_set = desc_pool.allocate_set(&set_layout).unwrap();

    let memory_types = physical_device.memory_properties().memory_types;

    let mesh = MESH;

    let (vertex_buffer, vertex_buffer_memory) = utils::create_buffer::<backend::Backend, Vertex>(
        &device,
        &memory_types,
        Properties::CPU_VISIBLE,
        buffer::Usage::VERTEX,
        &mesh
    );

    let (uniform_buffer, mut uniform_memory) = utils::create_buffer::<backend::Backend, UniformBlock>(
        &device,
        &memory_types,
        Properties::CPU_VISIBLE,
        buffer::Usage::UNIFORM,
        &[UniformBlock {
            projection: Default::default()
        }]
    );

    device.write_descriptor_sets(vec![DescriptorSetWrite{
        set: &desc_set,
        binding: 0,
        array_offset: 0,
        descriptors: Some(Descriptor::Buffer(&uniform_buffer, None..None))
    }]);

    // One push constant for each draw call.
    let diamonds = vec![
        PushConstants {
            position: [-1.0, -1.0, 0.0],
            tint: [1.0, 0.0, 0.0, 1.0]
        },
        PushConstants {
            position: [1.0, -1.0, 0.0],
            tint: [0.0, 1.0, 0.0, 1.0],
        },
        PushConstants {
            position: [-1.0, 1.0, 0.0],
            tint: [0.0, 0.0, 1.0, 1.0],
        },
        PushConstants {
            position: [1.0, 1.0, 0.0],
            tint: [1.0, 1.0, 1.0, 1.0],
        },
    ];


    let frame_semaphore = device.create_semaphore();
    let present_semaphore = device.create_semaphore();

    let mut swapchain_stuff : Option<(_, _, _, _)> = None;
    let mut rebuild_swapchain = false;

    // we have a timer now. fancy.
    let start_time = Instant::now();
    let mut last_time = start_time;

    'main: loop {
        let mut quitting = false;

        let now = Instant::now();
        let delta = now.duration_since(last_time);
        println!("dt: {:?}", delta);
        last_time = now;
        events_loop.poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => quitting = true,
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => quitting = true,
                    WindowEvent::Resized(_) => rebuild_swapchain = true,
                    _ => ()
                }

            }
        });

        if (rebuild_swapchain || quitting) && swapchain_stuff.is_some() {
            let (swapchain, _extent, frame_views, framebuffers) = swapchain_stuff.take().unwrap();

            device.wait_idle().unwrap();
            command_pool.reset();

            for framebuffer in framebuffers {
                device.destroy_framebuffer(framebuffer);
            }

            for image_view in frame_views {
                device.destroy_image_view(image_view);
            }

            device.destroy_swapchain(swapchain);
        }

        if quitting {
            break 'main;
        }

        if swapchain_stuff.is_none() {
            rebuild_swapchain = false;
            let (caps, _, _) = surface.compatibility(physical_device);

            let swap_config = SwapchainConfig::from_caps(&caps, surface_colour_format);
            let extent = swap_config.extent.to_extent();
            let (swapchain, backbuffer) = device.create_swapchain(&mut surface, swap_config, None);

            let (frame_views, framebuffers) = match backbuffer {
                Backbuffer::Images(images) => {
                    let color_range = SubresourceRange {
                        aspects: Aspects::COLOR,
                        levels: 0..1,
                        layers: 0..1,
                    };

                    let image_views = images
                        .iter()
                        .map(|image| {
                            device
                                .create_image_view(
                                    image,
                                    ViewKind::D2,
                                    surface_colour_format,
                                    Swizzle::NO,
                                    color_range.clone(),
                                ).unwrap()
                        }).collect::<Vec<_>>();

                    let fbos = image_views
                        .iter()
                        .map(|image_view| {
                            device
                                .create_framebuffer(&render_pass, vec![image_view], extent)
                                .unwrap()
                        }).collect();

                    (image_views, fbos)
                }
                Backbuffer::Framebuffer(fbo) => (Vec::new(), vec![fbo]),
            };

            swapchain_stuff = Some((swapchain, extent, frame_views, framebuffers));
        }

        let (swapchain, extent, _frame_views, framebuffers) = swapchain_stuff.as_mut().unwrap();

        let (width, height) = (extent.width, extent.height);
        let aspect_corrected_x = height as f32 / width as f32;
        let t = {
            let elapsed = start_time.elapsed();
            elapsed.as_secs() as f32 + elapsed.subsec_nanos() as f32 / 1_000_000_000.0
        };
        let zoom = t.cos() * 0.33 + 0.67;
        let x_scale = aspect_corrected_x * zoom;
        let y_scale = zoom;

        utils::fill_buffer::<backend::Backend, UniformBlock>(
            &device,
            &mut uniform_memory,
            &[UniformBlock {
                projection: [
                    [x_scale, 0.0, 0.0, 0.0],
                    [0.0, y_scale, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]
            }]
        );

        // Begin rendering.
        //
        command_pool.reset();

        let frame_index: SwapImageIndex = {
            match swapchain.acquire_image(!0, FrameSync::Semaphore(&frame_semaphore)) {
                Ok(i) => i,
                Err(_) => {
                    rebuild_swapchain = true;
                    continue;
                }
            }
        };

        let finished_command_buffer = {
            let mut command_buffer = command_pool.acquire_command_buffer(false);

            let viewport = Viewport {
                rect: Rect {
                    x: 0, y: 0,
                    w: extent.width as i16,
                    h: extent.height as i16,
                },
                depth: 0.0..1.0,
            };

            command_buffer.set_viewports(0, &[viewport.clone()]);
            command_buffer.set_scissors(0, &[viewport.rect]);

            command_buffer.bind_graphics_pipeline(&pipeline);

            command_buffer.bind_vertex_buffers(0, vec![(&vertex_buffer, 0)]);

            command_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, vec![&desc_set], &[]);

            {
                let mut encoder = command_buffer.begin_render_pass_inline(
                    &render_pass,
                    &framebuffers[frame_index as usize],
                    viewport.rect,
                    &[ClearValue::Color(ClearColor::Float([0.0, 0.0, 0.0, 1.0]))]
                );


                let num_vertices = mesh.len() as u32;

                // now, for each of our push constants
                // - cast the PushConstant data to &[u32]
                // - upload the data to the vertex shader
                // - draw our mesh with the vertex shader making use of the push constant data
                for diamond in &diamonds {
                    let push_constants = {
                        let start_ptr = diamond as *const PushConstants as *const u32;
                        unsafe {
                            std::slice::from_raw_parts(start_ptr, num_push_constants)
                        }
                    };
                    encoder.push_graphics_constants(
                        &pipeline_layout,
                        ShaderStageFlags::VERTEX,
                        0,
                        push_constants,
                    );
                    encoder.draw(0..num_vertices, 0..1);
                }
            }

            command_buffer.finish()
        };

        let submission = Submission::new()
            .wait_on(&[(&frame_semaphore, PipelineStage::BOTTOM_OF_PIPE)])
            .signal(&[&present_semaphore])
            .submit(vec![finished_command_buffer]);

        queue_group.queues[0].submit(submission, None);

        let result = swapchain.present(
            &mut queue_group.queues[0],
            frame_index,
            vec![&present_semaphore],
        );

        if result.is_err() {
            rebuild_swapchain = true;
        }
    }

    device.destroy_graphics_pipeline(pipeline);
    device.destroy_pipeline_layout(pipeline_layout);

    device.destroy_render_pass(render_pass);

    device.destroy_descriptor_pool(desc_pool);
    device.destroy_descriptor_set_layout(set_layout);
    device.destroy_buffer(uniform_buffer);
    device.free_memory(uniform_memory);


    device.destroy_buffer(vertex_buffer);
    device.free_memory(vertex_buffer_memory);

    device.destroy_shader_module(vertex_shader_module);
    device.destroy_shader_module(fragment_shader_module);

    device.destroy_command_pool(command_pool.into_raw());
    device.destroy_semaphore(frame_semaphore);
}
