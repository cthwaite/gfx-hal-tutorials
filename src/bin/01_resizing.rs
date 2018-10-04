#[cfg(windows)]
extern crate gfx_backend_dx12 as backend;
#[cfg(target_os = "macos")]
extern crate gfx_backend_metal as backend;
#[cfg(all(unix, not(target_os = "macos")))]
extern crate gfx_backend_vulkan as backend;

extern crate gfx_hal;
extern crate winit;

// There are a lot of imports - best to just accept it.
use gfx_hal::{
    command::{ClearColor, ClearValue},
    format::{Aspects, ChannelType, Format, Swizzle},
    image::{Access, Layout, SubresourceRange, ViewKind},
    pass::{
        Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp, Subpass, SubpassDependency,
        SubpassDesc, SubpassRef,
    },
    pool::CommandPoolCreateFlags,
    pso::{
        BlendState, ColorBlendDesc, ColorMask, EntryPoint, GraphicsPipelineDesc, GraphicsShaderSet,
        PipelineStage, Rasterizer, Rect, Viewport,
    },
    queue::Submission,
    Backbuffer, Device, FrameSync, Graphics, Instance, Primitive, Surface, SwapImageIndex,
    Swapchain, SwapchainConfig,
};

use winit::{Event, EventsLoop, KeyboardInput, VirtualKeyCode, WindowBuilder, WindowEvent};

static WIN_TITLE : &'static str = "Part 01: Resizing";
static VERT_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part00.vert.spv");
static FRAG_SPIRV : &'static [u8] = include_bytes!("../../assets/gen/shaders/part00.frag.spv");

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

        device.create_graphics_pipeline(&pipeline_desc, None)
              .unwrap()
    };

    let frame_semaphore = device.create_semaphore();
    let frame_fence = device.create_fence(false);


    // We're going to defer the construction of our swapchain, extent, image
    // views, and framebuffers until the main loop, because we will need to
    // repeat it whenever the window resizes. For now, we leave them empty.
    //
    // We're using an option containing a tuple so that we can drop all four
    // items simultaneously.
    // We also take advantage of type inference by withholding the types of
    // each tuple member at this point.
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


    device.destroy_render_pass(render_pass);

    device.destroy_shader_module(vertex_shader_module);
    device.destroy_shader_module(fragment_shader_module);

    device.destroy_command_pool(command_pool.into_raw());
    device.destroy_fence(frame_fence);
    device.destroy_semaphore(frame_semaphore);
}
