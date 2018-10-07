extern crate haltut;

use haltut::prelude::*;
use haltut::backend;


fn main() {
    let mut events_loop = EventsLoop::new();
    let window = WindowBuilder::new()
                    .with_title("Part 00: Triangle")
                    .with_dimensions((640, 480).into())
                    .with_decorations(false)
                    .build(&events_loop)
                    .unwrap();
    // The Instance serves as an entry point to the graphics API; the create
    // method takes an application name and version, but these aren't important.
    let instance = backend::Instance::create("Part 00: Triangle", 1);

    // A Surface is an abstraction for the OS's native window.
    let mut surface = instance.create_surface(&window);

    // Just take the first available physical device, which could be a graphics
    // card or something... else...
    let mut adapter = instance.enumerate_adapters().remove(0);

    // The device is a logical device allowing you to perform GPU operations.
    // The queue group contains a set of command queues which we can later submit
    // drawing commands to.
    //
    // Here we're requesting one queue with the 'Graphics' capability so we can
    // do rendering. We also pass a closure to choose the first queue family
    // that our surface supports, to allocate queues from.
    let num_queues = 1;
    let (device, mut queue_group) = adapter
        .open_with::<_, Graphics>(num_queues, |family| surface.supports_queue_family(family))
        .unwrap();

    // A command pool is used to acquire command buffers, which are used to
    // send drawing instructions to the GPU.
    let max_buffers = 16;
    let mut command_pool = device.create_command_pool_typed(&queue_group,
                                                            CommandPoolCreateFlags::empty(),
                                                            max_buffers);

    let physical_device = &adapter.physical_device;

    // We want to get the capabilities ('caps') of the surface, which tells us
    // what parameters we can use for our swapchain later. We also get a list
    // of supported image formats for our surface.
    let (caps, formats, _) = surface.compatibility(physical_device);
    let surface_colour_format = {
        // We pick a colour format from the list of supported formats. If there
        // is no list, we default to 'Rgba8Srgb'.
        match formats {
            Some(choices) => choices.into_iter()
                                    .find(|format| format.base_format().1 == ChannelType::Srgb)
                                    .unwrap(),
            None => Format::Rgba8Srgb,
        }
    };

    // A render pass defines which 'attachements' (images) are to be used for
    // what purposes.
    // Right now, we only have a colour attachment for the final output,
    // but eventually we might have depth or stencil attachments, or even other
    // colour attachments for other purposes.
    let render_pass = {
        let colour_attachment = Attachment {
            format: Some(surface_colour_format),
            samples: 1,
            ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
            stencil_ops: AttachmentOps::DONT_CARE,
            layouts: Layout::Undefined..Layout::Present
        };

        // A reder pass oculd have multiple subpasses; we're using one for now.
        let subpass = SubpassDesc {
            colors: &[(0, Layout::ColorAttachmentOptimal)],
            depth_stencil: None,
            inputs: &[],
            preserves: &[],
            resolves: &[]
        };

        // This expresses the dependencies between subpasses.
        // Again, we only have one subpass for now.
        let dependency = SubpassDependency {
            passes: SubpassRef::External..SubpassRef::Pass(0),
            stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            accesses: Access::empty()..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE),
        };

        device.create_render_pass(&[colour_attachment], &[subpass], &[dependency])
    };

    // The pipeline layout defines the shape of the data you can send to a
    // shader.
    // This includes the number of uniforms and push constants. We don't need
    // them for now.
    let pipeline_layout = device.create_pipeline_layout(&[], &[]);

    // Shader modules are needed to create the pipeline definition.
    // The shaders are loaded from SPIR-V binary files.
    let vertex_shader_module = {
        let spirv = include_bytes!("../../assets/gen/shaders/part00.vert.spv");
        device.create_shader_module(spirv).unwrap()
    };
    let fragment_shader_module = {
        let spirv = include_bytes!("../../assets/gen/shaders/part00.frag.spv");
        device.create_shader_module(spirv).unwrap()
    };

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

        device.create_graphics_pipeline(&pipeline_desc, None)
              .unwrap()
    };

    // Initialize our swapchain, images, framebuffers etc.
    // We expect to have to rebuild these when the window is resized - however,
    // we're going to ignore that for this example.

    // A swapchain is effectively a chain of images (commonly two) that will be
    // displayed to the screen. While one is being displayed, we can draw to
    // one of the others.
    //
    // In a rare instance of the API creating resources for you, the backbuffer
    // contains the actual images that make up the swapchain. We'll create
    // image views and framebuffers from these next.
    //
    // We also want to store the swapchain's extent, which tells us how big
    // each image is.

    // SwapchainConfig
    // {
    //     present_mode: Fifo,
    //     format: Bgra8Srgb,
    //     extent: Extent2D { width: 256, height: 256 },
    //     image_count: 2,
    //     image_layers: 1,
    //     image_usage: COLOR_ATTACHMENT
    //  }
    let swap_config = SwapchainConfig::from_caps(&caps, surface_colour_format);

    // Extent
    // {
    //     width: 256,
    //     height: 256,
    //     depth: 1
    // }
    let extent = swap_config.extent.to_extent();


    let (mut swapchain, backbuffer) = device.create_swapchain(&mut surface, swap_config, None);

    // We can think of the image as just the raw binary of the literal image
    // with metadata about the format.
    //
    // Accessing the image must be done through an image view - which is more or
    // less a sub-range of the base image. For example, it could be one 2D slice
    // of a 3D texture. In many cases, the view will comprise the whole image.
    // We can also use an image view to swizzle or reinterpret the image format.
    //
    // Framebuffers bind certain image views to certain attachments. So for
    // example, if a render pass requires one colour attachement and one depth
    // attachment - the framebuffer chooses specific image views for each one.
    //
    // Here we create an image view and a framebuffer for each image in our
    // swapchain.

    let (frame_views, framebuffers) = match backbuffer {
        // This arm is currently only used by the OpenGL backend,
        // which supplies an opaque framebuffer instead of giving us control
        // over individual images.
        Backbuffer::Framebuffer(fbo) => (vec![], vec![fbo]),
        Backbuffer::Images(images) => {
            let colour_range = SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            };



            let image_views = images.iter()
                                    .map(|image| {
                                        device.create_image_view(image,
                                                                 ViewKind::D2,
                                                                 surface_colour_format,
                                                                 Swizzle::NO,
                                                                 colour_range.clone()).unwrap()
                                    })
                                    .collect::<Vec<_>>();
            let fbos = image_views.iter()
                                  .map(|image_view| {
                                    device.create_framebuffer(&render_pass, vec![image_view], extent)
                                          .unwrap()
                                  }).collect();
            (image_views, fbos)
        }
    };

    // The frame semaphore is used to allow us to wait for an image to be ready
    // before attempting to draw on it.
    let frame_semaphore = device.create_semaphore();

    // The frame fence is used to allow us to wait until our draw commands have
    // finished before attempting to display the image.
    let frame_fence = device.create_fence(false);

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
                    _ => ()
                }

            }
        });

        if quitting {
            break 'main;
        }

        // Begin rendering.
        //
        device.reset_fence(&frame_fence);
        command_pool.reset();

        // A swapchain contains multiple images - which one to draw on?
        // This returns the index of the image we use. The image may not be
        // ready for rendering yet, but will signal frame_semaphore when it is.
        let frame_index: SwapImageIndex = swapchain.acquire_image(!0, FrameSync::Semaphore(&frame_semaphore))
                                                   .expect("Failed to acquire frame!");

        // We have to build a command buffer before we send it off to be drawn.
        // We don't technically have to do this every frame, but if it changes
        // every frame, then we do.
        let finished_command_buffer = {
            // acquire_command_buffer(allow_pending_resubmit: bool)
            // you can only record to one command buffer per pool at the same time
            let mut command_buffer = command_pool.acquire_command_buffer(false);

            // Define a rectangle on screen to draw into: in this case, the
            // whole screen.
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

            // Choose a pipeline.
            command_buffer.bind_graphics_pipeline(&pipeline);

            {
                // Clear the screen and begin the render pass.
                let mut encoder = command_buffer.begin_render_pass_inline(
                    &render_pass,
                    &framebuffers[frame_index as usize],
                    viewport.rect,
                    &[ClearValue::Color(ClearColor::Float([0.0, 0.0, 0.0, 1.0]))]
                );

                // Draw the geometry. In this case 0..3 indicates the range of
                // vertices to be drawn. We have no vertex buffer as yet, so
                // this really just tells our shader to draw one triangle. The
                // specific vertices to draw at this point are encoded in the
                // shader itself.
                //
                // The 0..1 is the range of instances to draw. This is
                // irrelevant unless we're using instanced rendering.
                encoder.draw(0..3, 0..1);
            }

            // Finish building the command buffer; it is now ready to send to
            // the GPU.
            command_buffer.finish()
        };

        // This is what we submit to the command queeu. We wait until
        // frame_semaphore is signalled, at which point we know our chosen image
        // is available to draw on.
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

    for framebuffer in framebuffers {
        device.destroy_framebuffer(framebuffer);
    }

    for image_view in frame_views {
        device.destroy_image_view(image_view);
    }

    device.destroy_render_pass(render_pass);
    device.destroy_swapchain(swapchain);

    device.destroy_shader_module(vertex_shader_module);
    device.destroy_shader_module(fragment_shader_module);

    device.destroy_command_pool(command_pool.into_raw());
    device.destroy_fence(frame_fence);
    device.destroy_semaphore(frame_semaphore);
}
