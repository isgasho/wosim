mod context;
mod frame;
mod state;
mod surface;

use std::{sync::Arc, time::Instant};

pub use context::*;
pub use frame::*;
use generator::{Generator, Template};
use network::{value_channel, Connection, Message, MessageReceiver};
use protocol::{Request, SLOT_COUNT};
pub use state::*;
pub use surface::*;
use tokio::{spawn, sync::mpsc};
use tracing::error;
use util::handle::HandleFlow;
use vulkan::{Format, RenderPass, SwapchainKHR};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Fullscreen,
};

use crate::{
    action::Action,
    cache::Cache,
    frame::PerFrame,
    renderer::{RenderError, RenderResult},
    session::{Session, SessionState},
    vulkan::create_swapchain,
};

pub struct Root {
    pub frames: PerFrame<RootFrame>,
    pub surface: Cache<SwapchainKHR, RootSurface>,
    pub render_pass: Cache<Format, RenderPass>,
    pub state: RootState,
    pub context: RootContext,
}

impl Root {
    pub fn new(event_loop: &EventLoop<Action>, state: RootState) -> eyre::Result<Self> {
        let context = RootContext::new(event_loop)?;
        let frames = PerFrame::new(|_| RootFrame::new(&context))?;
        Ok(Self {
            frames,
            surface: Cache::default(),
            render_pass: Cache::default(),
            state,
            context,
        })
    }

    pub fn render(&mut self) -> Result<RenderResult, RenderError> {
        let context = &self.context;
        let render_pass = self
            .render_pass
            .try_get(self.context.swapchain.image_format(), || {
                context.create_render_pass()
            })?;
        let surface = self.surface.try_get(**context.swapchain, || {
            RootSurface::new(context, render_pass)
        })?;
        let frame = &mut self.frames[self.context.frame_count];
        let result = frame.render(&mut self.context, &mut self.state, render_pass, surface);
        self.context.frame_count += 1;
        result
    }

    pub async fn handle(&mut self, event: Event<'_, Action>) -> eyre::Result<ControlFlow> {
        match event.map_nonuser_event() {
            Ok(event) => {
                if self.handle_early_mouse_grab(&event).is_handled() {
                    return Ok(ControlFlow::Poll);
                }
                if self.context.egui.context.handle_event(&event).is_handled() {
                    return Ok(ControlFlow::Poll);
                }
                if self
                    .state
                    .handle_event(&event, self.context.grab)
                    .is_handled()
                {
                    return Ok(ControlFlow::Poll);
                }
                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::Resized(_) => {
                            self.recreate_swapchain()?;
                        }
                        WindowEvent::KeyboardInput {
                            device_id: _,
                            input,
                            is_synthetic: _,
                        } => {
                            if let Some(keycode) = input.virtual_keycode {
                                match keycode {
                                    VirtualKeyCode::F1 => {
                                        if input.state == ElementState::Pressed {
                                            self.context.windows.information =
                                                !self.context.windows.information;
                                        }
                                    }
                                    VirtualKeyCode::F2 => {
                                        if input.state == ElementState::Pressed {
                                            self.context.windows.frame_times =
                                                !self.context.windows.frame_times;
                                        }
                                    }
                                    VirtualKeyCode::F3 => {
                                        if input.state == ElementState::Pressed {
                                            self.context.windows.log = !self.context.windows.log;
                                        }
                                    }
                                    VirtualKeyCode::F9 => {
                                        if input.state == ElementState::Pressed {
                                            self.context.vsync = !self.context.vsync;
                                            self.recreate_swapchain()?;
                                        }
                                    }
                                    VirtualKeyCode::F10 => {
                                        if input.state == ElementState::Pressed {
                                            if self.context.window.fullscreen().is_some() {
                                                self.context.window.set_fullscreen(None);
                                            } else {
                                                self.context.window.set_fullscreen(Some(
                                                    Fullscreen::Borderless(None),
                                                ));
                                            }
                                        }
                                    }
                                    VirtualKeyCode::Escape => self.set_grab(false)?,
                                    _ => {}
                                }
                            }
                        }
                        WindowEvent::MouseInput { button, .. } => {
                            if self.state.can_grab() && button == MouseButton::Left {
                                self.set_grab(true)?;
                            };
                        }
                        WindowEvent::Focused(focus) => {
                            if !focus {
                                self.set_grab(false)?;
                            }
                        }
                        WindowEvent::CloseRequested => return Ok(ControlFlow::Exit),
                        _ => {}
                    },
                    Event::MainEventsCleared => {
                        let now = Instant::now();
                        self.state.update(now).await;
                        self.context.debug.begin_frame();
                        let ctx = self.context.egui.context.begin();
                        self.context.debug.render(
                            &ctx,
                            &mut self.context.windows,
                            self.state.game().map(|game| &game.context.scene.context),
                            &self.context.filter_handle,
                        );
                        self.state.render_egui(&ctx, &self.context.proxy);
                        self.context.egui.context.end(if self.context.grab {
                            None
                        } else {
                            Some(&self.context.window)
                        })?;
                        let (resize, timestamps) = match self.render() {
                            Ok(result) => (result.suboptimal, result.timestamps),
                            Err(err) => match err {
                                RenderError::Error(error) => return Err(error),
                                RenderError::OutOfDate => (true, None),
                            },
                        };
                        self.context
                            .debug
                            .end_frame(timestamps, self.state.connection());
                        if resize {
                            self.recreate_swapchain()?;
                        }
                    }
                    _ => {}
                }
            }
            Err(event) => {
                if let Event::UserEvent(action) = event {
                    match action {
                        Action::Notification(notification) => {
                            self.state.apply(&self.context, notification)?
                        }
                        Action::Log(buf) => {
                            self.context.debug.log(buf);
                        }
                        Action::Disconnected => {
                            return Ok(ControlFlow::Exit);
                        }
                        Action::Connected(endpoint, _, server) => {
                            let mut messages = MessageReceiver::new(endpoint.receiver);
                            let proxy = self.context.proxy.clone();
                            let task = spawn(async move {
                                while let Some(message) = messages.recv().await {
                                    let notification = message.try_into().unwrap();
                                    if let Err(error) =
                                        proxy.send_event(Action::Notification(notification))
                                    {
                                        error!("{:?}", error);
                                        break;
                                    }
                                }
                                if let Err(error) = proxy.send_event(Action::Disconnected) {
                                    error!("{:?}", error);
                                };
                            });
                            let connection = Connection::new(endpoint.connection);
                            {
                                let connection = connection.clone();
                                let proxy = self.context.proxy.clone();
                                spawn(async move {
                                    let (sender, receiver) = value_channel();
                                    connection
                                        .send(Message::from(Request::Slots(sender)))
                                        .await
                                        .unwrap();
                                    let slots = receiver.recv().await.unwrap();
                                    proxy.send_event(Action::UpdateLobbySlots(slots)).unwrap();
                                });
                            }
                            self.state = RootState::Connected(Session::new(
                                connection,
                                task,
                                server,
                                SessionState::Lobby {
                                    slots: [u32::MAX; SLOT_COUNT],
                                },
                            ))
                        }
                        Action::Error(error) => self.state = RootState::Report { error },
                        Action::Create => {
                            let (sender, mut receiver) = mpsc::channel(16);
                            let generator =
                                Generator::new(Template {}, sender, self.context.device.clone());
                            let control = generator.control.clone();
                            let proxy = self.context.proxy.clone();
                            let task = Some(spawn(async move {
                                while let Some(notification) = receiver.recv().await {
                                    proxy
                                        .send_event(Action::GeneratorNotification(notification))
                                        .unwrap()
                                }
                                proxy.send_event(Action::GeneratorFinished).unwrap()
                            }));
                            self.state = RootState::Generate {
                                generator,
                                control,
                                task,
                            }
                        }
                        Action::GeneratorNotification(_) => {}
                        Action::GeneratorFinished => {
                            if let RootState::Generate {
                                generator, task, ..
                            } = &mut self.state
                            {
                                task.take().unwrap().await?;
                                match generator.join().await {
                                    Ok(()) => self.state = RootState::GenerateFinished,
                                    Err(error) => {
                                        self.state = RootState::Report {
                                            error: eyre::Error::new(error),
                                        }
                                    }
                                }
                            }
                        }
                        Action::Close => return Ok(ControlFlow::Exit),
                        Action::UpdateLobbySlots(new_slots) => {
                            if let RootState::Connected(session) = &mut self.state {
                                if let SessionState::Lobby { slots } = &mut session.state {
                                    *slots = new_slots
                                }
                            }
                        }
                        Action::UpdateLobbySlot(slot, id) => {
                            if let RootState::Connected(session) = &mut self.state {
                                if let SessionState::Lobby { slots } = &mut session.state {
                                    slots[slot as usize] = id
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(ControlFlow::Poll)
    }

    fn handle_early_mouse_grab(&mut self, event: &Event<()>) -> HandleFlow {
        if self.context.grab {
            if let Event::WindowEvent {
                event: WindowEvent::CursorMoved { .. },
                ..
            } = event
            {
                return HandleFlow::Handled;
            }
        }
        HandleFlow::Unhandled
    }

    fn recreate_swapchain(&mut self) -> eyre::Result<()> {
        self.context.device.wait_idle()?;
        self.context.swapchain = Arc::new(create_swapchain(
            &self.context.device,
            &self.context.surface,
            &self.context.window,
            !self.context.vsync,
            Some(&self.context.swapchain),
        )?);
        Ok(())
    }

    fn set_grab(&mut self, grab: bool) -> eyre::Result<()> {
        if self.context.grab == grab {
            return Ok(());
        }
        self.context.grab = grab;
        if grab {
            self.context.window.set_cursor_visible(false);
            self.context.window.set_cursor_grab(true)?;
        } else {
            let size = self.context.window.inner_size();
            let position = PhysicalPosition {
                x: size.width as i32 / 2,
                y: size.height as i32 / 2,
            };
            self.context
                .window
                .set_cursor_position(::winit::dpi::Position::Physical(position))?;
            self.context.window.set_cursor_visible(true);
            self.context.window.set_cursor_grab(false)?;
        }
        Ok(())
    }
}
