extern crate haltut;

use haltut::prelude::*;

use haltut::backend;
use backend::Backend;
use haltut::utils;


static WIN_TITLE : &'static str = "Part 02: Vertex Buffers";
static VERT_SPIRV : &'static [u8] = include_bytes!("../assets/gen/shaders/part02.vert.spv");
static FRAG_SPIRV : &'static [u8] = include_bytes!("../assets/gen/shaders/part02.frag.spv");

// repr(C) ensures deterministic layout in memory.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Vertex {
    position: [f32; 3],
    colour: [f32; 4]
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
                    .with_decorations(false)
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

    let pipeline_layout = device.create_pipeline_layout(&[], &[]);

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

        // We need to let the pipeline know about all the different formats
        // of vertex buffer we're going to use.
        // - The `binding` number is an ID for this entry.
        // - The `stride` indicates the size of each vertex in bytes.
        // - The `rate` is used for instanced rendering, and can be ignored at
        // this point.
        pipeline_desc.vertex_buffers.push(VertexBufferDesc {
            binding: 0,
            stride: std::mem::size_of::<Vertex>() as u32,
            rate: 0
        });

        // We have to declare two vertex attributes: position, and colour.
        // Note that their locaitons have to match the locations in the shader,
        // and their format has to be appropriate for the data type in the
        // shader.
        //
        // vec3 = Rgb32Float
        // vec4 = Rgba32Float
        //
        // Additionally, the second attribute must have an offset of 12 bytes
        // in the vertex, because this refers to the size of the first field.
        // The `binding` parameter refers back to the ID we gave in the
        // VertexBufferDesc above.
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

    let memory_types = physical_device.memory_properties().memory_types;

    let mesh = MESH;

    // Here's where we create the buffer itself, and the memory it uses.
    let (vertex_buffer, vertex_buffer_memory) = utils::create_buffer::<Backend, Vertex>(
        &device,
        &memory_types,
        Properties::CPU_VISIBLE,
        buffer::Usage::VERTEX,
        mesh
    );

    let frame_semaphore = device.create_semaphore();
    let frame_fence = device.create_fence(false);

    let mut swapchain_stuff : Option<(_, _, _, _)> = None;
    let mut rebuild_swapchain = false;

    'main: loop {
        let mut quitting = false;
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
            // Take ownership of swapchain_stuff contents.
            let (swapchain, _extent, frame_views, framebuffers) = swapchain_stuff.take().unwrap();

            // Wait for all queues to be idle and reset the comand pool, so that
            // we know no commands are being executed while we destroy the
            // swapchain.
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

            // Here we just create the swapchain, image views, and framebuffers
            // like we did in part 00, and store them in swapchain_stuff.
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

        // To access the swapchain, we need to get a mutable reference to the
        // contents of swapchain_stuff. We know it's safe to unwrap because we just
        // checked it wasn't `None`.
        let (swapchain, extent, _frame_views, framebuffers) = swapchain_stuff.as_mut().unwrap();

        // Begin rendering.
        //
        device.reset_fence(&frame_fence);
        command_pool.reset();

        let frame_index: SwapImageIndex = swapchain.acquire_image(!0, FrameSync::Semaphore(&frame_semaphore))
                                                   .expect("Failed to acquire frame!");

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

            // This is where we tell our pipeline to use a specific vertex
            // buffer.
            // The first argument again refers to the vertex buffer `binding`.
            // The second argument is a Vec of buffers in the form
            //  (buffer, offset)
            // where offset is relative to the binding number.
            command_buffer.bind_vertex_buffers(0, vec![(&vertex_buffer, 0)]);

            {
                // Clear the screen and begin the render pass.
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

        let semaphore = (&frame_semaphore, PipelineStage::BOTTOM_OF_PIPE);
        let submission = Submission::new()
                            .wait_on(&[semaphore])
                            .submit(vec![finished_command_buffer]);

        // We submit the 'submission' to one of our command queues, which will
        // signal frame_fence once rendering is completed.
        queue_group.queues[0].submit(submission, Some(&frame_fence));

        // We first wait for rendering to complete...
        device.wait_for_fence(&frame_fence, !0);

        // ...and then present the image on screen.
        swapchain.present(&mut queue_group.queues[0], frame_index, &[])
                 .expect("Failed to present");
    }

    device.destroy_graphics_pipeline(pipeline);
    device.destroy_pipeline_layout(pipeline_layout);


    device.destroy_render_pass(render_pass);

    device.destroy_buffer(vertex_buffer);
    device.free_memory(vertex_buffer_memory);

    device.destroy_shader_module(vertex_shader_module);
    device.destroy_shader_module(fragment_shader_module);

    device.destroy_command_pool(command_pool.into_raw());
    device.destroy_fence(frame_fence);
    device.destroy_semaphore(frame_semaphore);
}
