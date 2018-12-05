extern crate haltut;

use haltut::backend;
use haltut::prelude::*;
use haltut::utils;

use std::time::Instant;

static WIN_TITLE : &'static str = "Part 03: Uniforms";

#[cfg(windows)]
static VERT_SPIRV : &'static [u8] = include_bytes!("..\\..\\assets\\gen\\shaders\\part03.vert.spv");
#[cfg(windows)]
static FRAG_SPIRV : &'static [u8] = include_bytes!("..\\..\\assets\\gen\\shaders\\part03.frag.spv");

#[cfg(all(unix))]
static VERT_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part03.vert.spv");
#[cfg(all(unix))]
static FRAG_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part03.frag.spv");

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    colour: [f32; 4]
}

// A struct to upload to a uniform buffer; this is a 4x4 projection matrix.
// As before, repr(C) for deterministic layout.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct UniformBlock {
    projection: [[f32; 4]; 4]
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

    // ALthough this could change between swapchains, we're going to ignore that
    // for now...
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

    // Shaders access resources such as buffers and images by using special variables which are indirectly bound to 
    // buffer and image views by the API.
    // 
    // Variables are organised into sets, where each set of bindings is represented by a 'descriptor set' object
    // in the API, and a discriptor set is bound all at once. A 'descriptor' is an opaque data structure representing
    // a shader resource. The content of each set is determined by its 'descriptor set layout', which we define here.
    //
    // We pass in a list of bindings - just one for now, representing our uniform buffer - along with an array size
    // 'count' indicating the number of descriptors in the binding, a set of shader stages permitted to access the
    // binding, and (if using immutable samplers) an array of sampler descriptors. We want our uniform, a projection
    // matrix, to be available to the Vertex shader.
    //
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

    // For the first time, we're passing something create_pipeline_layout - an
    // array of descriptor set layouts. We pass in our new
    // descriptor set layout to indicate that it should be accessible from within
    // the pipeline. cumulatively, the set of layouts (and push constants, which are
    // covered in the next tutorial) describe the interface between shader stages
    // and shader resources.
    let pipeline_layout = device.create_pipeline_layout(vec![&set_layout], &[]);

    let vertex_shader_module = device.create_shader_module(VERT_SPIRV).unwrap();
    let fragment_shader_module = device.create_shader_module(FRAG_SPIRV).unwrap();

    // A pipeline object encodes almost all the state you need in order to draw
    // geometry on screen.
    // For now, that's really only which shaders to use, what kind of blending
    // to do, and what kind of primitives to draw.
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

    // Descriptor sets can't be created directly, and like command buffers,
    // must be allocated from a pool.
    let mut desc_pool = device.create_descriptor_pool(
        1, // maximum number of descriptor sets
        &[DescriptorRangeDesc {
            ty: DescriptorType::UniformBuffer,
            count: 1 // amount of space
        }]
    );

    // Allocate our previously-specified descriptor set from the pool.
    let desc_set = desc_pool.allocate_set(&set_layout).unwrap();

    let memory_types = physical_device.memory_properties().memory_types;

    let mesh = MESH;

    // Using our new utility functions in place of last tutorial's boilerplate...
    let (vertex_buffer, vertex_buffer_memory) = utils::create_buffer::<backend::Backend, Vertex>(
        &device,
        &memory_types,
        Properties::CPU_VISIBLE,
        buffer::Usage::VERTEX,
        &mesh
    );

    // ... and also here, to create our uniform buffer.
    let (uniform_buffer, mut uniform_memory) = utils::create_buffer::<backend::Backend, UniformBlock>(
        &device,
        &memory_types,
        Properties::CPU_VISIBLE,
        buffer::Usage::UNIFORM,
        &[UniformBlock {
            projection: Default::default()
        }]
    );

    // "Specifying the parameters of a descriptor set write operation" - ???
    // Understand 'writing descriptor sets' and also what's specified here
    // apart from the Descriptor::Buffer bit...
    device.write_descriptor_sets(vec![DescriptorSetWrite{
        set: &desc_set,
        binding: 0,
        array_offset: 0,
        descriptors: Some(Descriptor::Buffer(&uniform_buffer, None..None))
    }]);


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
                    // Set the rebuild flag if the window resizes.
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

        // new stuff!
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
            
            // explain this!!
            command_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, vec![&desc_set], &[]);

            {
                let mut encoder = command_buffer.begin_render_pass_inline(
                    &render_pass,
                    &framebuffers[frame_index as usize],
                    viewport.rect,
                    &[ClearValue::Color(ClearColor::Float([0.0, 0.0, 0.0, 1.0]))]
                );

                // Draw the number of vertices in our mesh.
                let num_vertices = mesh.len() as u32;
                encoder.draw(0..num_vertices, 0..1);
            }

            command_buffer.finish()
        };


        // new too...
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

    
    // TODO: Note the various new things we have to clean up
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
