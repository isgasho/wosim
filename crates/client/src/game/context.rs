use std::mem::size_of;
use std::time::{Duration, Instant};

use client_gpu::Model;
use eyre::Context;
use gpu_util::glam::{vec3, vec4, Vec3};
use nalgebra::RealField;
use network::{Connection, Message};
use physics::{InteractionGroups, Ray};
use protocol::{Entity, Position, Request, Rotation, WorldEnter};
use util::handle::HandleFlow;
use util::interpolation::Interpolate;
use vulkan::{
    mip_levels_for_extent, AccessFlags, BufferCopy, BufferMemoryBarrier, DependencyFlags,
    DescriptorPool, DescriptorPoolSetup, DrawIndexedIndirectCommand, PipelineBindPoint,
    PipelineStageFlags, RenderPass,
};
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

use crate::character::{NPCVec, PCVec};
use crate::cull::Cull;
use crate::depth::{Depth, DepthImage};
use crate::region::RegionVec;
use crate::root::{RootContext, RootFrame, RootSurface};
use crate::scene::{Camera, MeshData, Scene, Vertex};
use crate::terrain::Terrain;
use crate::{scene::ControlState, world::World};

pub struct GameContext {
    pub self_id: u32,
    pub world: World,
    pub cube_model: u32,
    pub cull: Cull,
    pub depth: Depth,
    pub terrain: Terrain,
    pub scene: Scene,
    pub last_payload: Option<(Position, Rotation)>,
    pub last_send: Instant,
    pub last_update: Instant,
    pub control_state: ControlState,
    pub descriptor_pool: DescriptorPool,
    pub target: Option<(Entity, f32)>,
}

impl GameContext {
    pub fn new(root_context: &RootContext, enter: WorldEnter) -> eyre::Result<Self> {
        let descriptor_pool = GameContext::pool_setup().create_pool(&root_context.device)?;
        let camera = Camera {
            translation: enter.pos,
            roll: enter.rotation.roll,
            pitch: enter.rotation.pitch,
            yaw: enter.rotation.yaw,
            fovy: f32::pi() / 3.0,
            znear: 0.1,
            zfar: 10000000.0,
        };
        let mut scene = Scene::new(root_context, &descriptor_pool, camera)?;
        let world = World {
            regions: RegionVec::new(),
            physics: physics::World::default(),
            npcs: NPCVec::new(),
            pcs: PCVec::new(),
            region_size: enter.region_size,
            size: enter.size,
            tick: enter.tick,
            tick_delta: enter.tick_delta,
            client_delta: Duration::from_millis(150),
            tick_time: Instant::now(),
            max_active_regions: enter.max_active_regions,
        };
        let terrain = Terrain::new(root_context, &scene, &descriptor_pool, &world)?;
        let depth = Depth::new(root_context, &descriptor_pool)?;
        let cull = Cull::new(root_context, &scene, &descriptor_pool)?;
        let cube = MeshData {
            vertices: vec![
                Vertex {
                    pos: vec3(-1.0, -1.0, -1.0),
                    normal: vec3(-1.0, -1.0, -1.0).normalize(),
                    color: vec3(0.0, 0.0, 0.0),
                },
                Vertex {
                    pos: vec3(-1.0, -1.0, 1.0),
                    normal: vec3(-1.0, -1.0, 1.0).normalize(),
                    color: vec3(0.0, 0.0, 1.0),
                },
                Vertex {
                    pos: vec3(-1.0, 1.0, -1.0),
                    normal: vec3(-1.0, 1.0, -1.0).normalize(),
                    color: vec3(0.0, 1.0, 0.0),
                },
                Vertex {
                    pos: vec3(-1.0, 1.0, 1.0),
                    normal: vec3(-1.0, 1.0, 1.0).normalize(),
                    color: vec3(0.0, 1.0, 1.0),
                },
                Vertex {
                    pos: vec3(1.0, -1.0, -1.0),
                    normal: vec3(1.0, -1.0, -1.0).normalize(),
                    color: vec3(1.0, 0.0, 0.0),
                },
                Vertex {
                    pos: vec3(1.0, -1.0, 1.0),
                    normal: vec3(1.0, -1.0, 1.0).normalize(),
                    color: vec3(1.0, 0.0, 1.0),
                },
                Vertex {
                    pos: vec3(1.0, 1.0, -1.0),
                    normal: vec3(1.0, 1.0, -1.0).normalize(),
                    color: vec3(1.0, 1.0, 0.0),
                },
                Vertex {
                    pos: vec3(1.0, 1.0, 1.0),
                    normal: vec3(1.0, 1.0, 1.0).normalize(),
                    color: vec3(1.0, 1.0, 1.0),
                },
            ],
            indices: vec![
                0, 1, 3, 0, 3, 2, 0, 2, 4, 2, 6, 4, 0, 4, 5, 0, 5, 1, 1, 5, 7, 1, 7, 3, 2, 3, 6, 3,
                7, 6, 4, 6, 5, 5, 6, 7,
            ],
        };
        let cube_mesh = scene.context.insert_mesh(cube);
        let cube_model = scene.context.insert_model(Model {
            bounds: vec4(0.0, 0.0, 0.0, 3f32.sqrt()),
            mesh: cube_mesh,
        });
        scene.context.flush(&root_context.device)?;
        Ok(Self {
            world,
            control_state: ControlState {
                backward: false,
                fast: false,
                forward: false,
                left: false,
                right: false,
            },
            last_update: Instant::now(),
            last_send: Instant::now(),
            last_payload: None,
            scene,
            cull,
            depth,
            descriptor_pool,
            terrain,
            cube_model,
            self_id: enter.self_id,
            target: None,
        })
    }

    pub fn handle_event(&mut self, event: &Event<()>, grab: bool) -> HandleFlow {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        device_id: _,
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(keycode),
                                state,
                                ..
                            },
                        is_synthetic: _,
                    },
                ..
            } => match keycode {
                VirtualKeyCode::W => {
                    self.control_state.forward = *state == ElementState::Pressed;
                    HandleFlow::Handled
                }
                VirtualKeyCode::A => {
                    self.control_state.left = *state == ElementState::Pressed;
                    HandleFlow::Handled
                }
                VirtualKeyCode::S => {
                    self.control_state.backward = *state == ElementState::Pressed;
                    HandleFlow::Handled
                }
                VirtualKeyCode::D => {
                    self.control_state.right = *state == ElementState::Pressed;
                    HandleFlow::Handled
                }
                VirtualKeyCode::LShift => {
                    self.control_state.fast = *state == ElementState::Pressed;
                    HandleFlow::Handled
                }
                _ => HandleFlow::Unhandled,
            },
            Event::DeviceEvent { event, .. } => {
                if grab {
                    if let DeviceEvent::MouseMotion { delta } = event {
                        self.scene.context.camera.yaw += -0.0008 * delta.0 as f32;
                        self.scene.context.camera.pitch += -0.0008 * delta.1 as f32;
                        self.scene.context.camera.pitch = self
                            .scene
                            .context
                            .camera
                            .pitch
                            .clamp(-f32::pi() / 2.0, f32::pi() / 2.0);
                        HandleFlow::Handled
                    } else {
                        HandleFlow::Unhandled
                    }
                } else {
                    HandleFlow::Unhandled
                }
            }
            _ => HandleFlow::Unhandled,
        }
    }

    pub fn update(&mut self, now: Instant, connection: &Connection<Request>) {
        let duration = now.duration_since(self.last_update);
        self.last_update = now;
        let speed = if self.control_state.fast { 100.0 } else { 10.0 };
        let distance = duration.as_secs_f32() * speed;
        let mut translation = Vec3::default();
        if self.control_state.forward {
            translation.z -= distance;
        }
        if self.control_state.backward {
            translation.z += distance;
        }
        if self.control_state.left {
            translation.x -= distance;
        }
        if self.control_state.right {
            translation.x += distance;
        }
        let translation = self.scene.context.camera.rotation_xy() * translation;
        self.scene.context.camera.translation += translation;
        self.scene.context.camera.translation.x = self.scene.context.camera.translation.x.clamp(
            0.0,
            self.world.region_size as f32 * self.world.size as f32 + 1.0,
        );
        self.scene.context.camera.translation.z = self.scene.context.camera.translation.z.clamp(
            0.0,
            self.world.region_size as f32 * self.world.size as f32 + 1.0,
        );
        let region_pos = self.world.region(self.scene.context.camera.translation);
        if let Some(region_index) = self
            .world
            .regions
            .index
            .get(&(region_pos.into_index(self.world.size) as usize))
        {
            let heights = &self.world.regions.heights[*region_index];
            let mut offset = self
                .world
                .region_offset(self.scene.context.camera.translation, region_pos);
            let (x0, z0) = (offset.x as usize, offset.z as usize);
            let (x1, z1) = (x0 + 1, z0 + 1);
            let (t_x, t_z) = (offset.x - offset.x.floor(), offset.z - offset.z.floor());
            let (y00, y10, y01, y11) = (
                heights[z0 * (self.world.region_size as usize + 1) + x0] as f32,
                heights[z0 * (self.world.region_size as usize + 1) + x1] as f32,
                heights[z1 * (self.world.region_size as usize + 1) + x0] as f32,
                heights[z1 * (self.world.region_size as usize + 1) + x0] as f32,
            );
            let min_y = Interpolate::interpolate(
                Interpolate::interpolate(y00, y10, t_x),
                Interpolate::interpolate(y01, y11, t_x),
                t_z,
            ) + 1.0;
            offset.y += self.world.physics.gravity[1] * duration.as_secs_f32();
            self.scene.context.camera.translation.y = offset.y.max(min_y);
        }
        self.target = if let Some((handle, toi)) = self.world.physics.cast_ray(
            &Ray {
                origin: self.scene.context.camera.translation.into(),
                dir: (self.scene.context.camera.rotation() * vec3(0.0, 0.0, -1.0)).into(),
            },
            f32::MAX,
            true,
            InteractionGroups::all(),
            None,
        ) {
            let collider = self.world.physics.colliders.get(handle).unwrap();
            if let Some(handle) = collider.parent() {
                let entity = Entity::from(self.world.physics.bodies.get(handle).unwrap().user_data);
                Some((entity, toi))
            } else {
                None
            }
        } else {
            None
        };
        let duration = now.duration_since(self.last_send);
        if duration.as_millis() > 1000 / 30 {
            self.last_send = now;
            let payload = (
                self.scene.context.camera.translation,
                Rotation {
                    roll: self.scene.context.camera.roll,
                    pitch: self.scene.context.camera.pitch,
                    yaw: self.scene.context.camera.yaw,
                },
            );
            if self.last_payload != Some(payload) {
                self.last_payload = Some(payload);
                let _ = connection.spawn_send(Message::from(Request::UpdateSelf(payload)));
            }
        }
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        root_surface: &RootSurface,
        root_frame: &mut RootFrame,
    ) -> eyre::Result<()> {
        self.scene.update(root_context, &mut self.world)?;
        let extent = root_context.swapchain.image_extent();
        let mip_levels = mip_levels_for_extent(extent);
        let depth_sampler = self.depth.sampler.try_get(mip_levels, || {
            Depth::create_sampler(root_context, mip_levels)
        })?;
        let depth_context = &self.depth.context;
        let depth_image = self.depth.image.try_get(extent, || {
            DepthImage::new(
                root_context,
                root_surface,
                root_frame,
                depth_context,
                extent,
                mip_levels,
                depth_sampler,
            )
        })?;
        self.cull
            .prepare_render(root_context, depth_image, depth_sampler);
        self.terrain
            .prepare_render(root_context, root_frame, &self.world)
            .wrap_err("could not prepare terrain rendering")?;
        let command_buffer = &root_frame.command_buffer;
        let cull_frame = &self.cull.frames[root_context.frame_count];
        let scene_frame = &self.scene.frames[root_context.frame_count];
        command_buffer.fill_buffer(&scene_frame.draw_count, 0, size_of::<u32>() as u64, 0);
        let buffer_memory_barriers = [BufferMemoryBarrier::builder()
            .src_access_mask(AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(AccessFlags::SHADER_READ | AccessFlags::SHADER_WRITE)
            .src_queue_family_index(0)
            .dst_queue_family_index(0)
            .buffer(*scene_frame.draw_count)
            .offset(0)
            .size(size_of::<u32>() as u64)
            .build()];
        command_buffer.pipeline_barrier(
            PipelineStageFlags::TRANSFER,
            PipelineStageFlags::COMPUTE_SHADER,
            DependencyFlags::empty(),
            &[],
            &buffer_memory_barriers,
            &[],
        );
        command_buffer.bind_pipeline(PipelineBindPoint::COMPUTE, &self.cull.context.pipeline);
        command_buffer.bind_descriptor_sets(
            PipelineBindPoint::COMPUTE,
            &self.cull.context.pipeline_layout,
            0,
            &[&cull_frame.descriptor_set],
            &[],
        );
        if !scene_frame.objects.is_empty() {
            command_buffer.dispatch((scene_frame.objects.len() as u32 + 255) / 256, 1, 1);
            let buffer_memory_barriers = [
                BufferMemoryBarrier::builder()
                    .src_access_mask(AccessFlags::SHADER_WRITE)
                    .dst_access_mask(AccessFlags::INDIRECT_COMMAND_READ)
                    .src_queue_family_index(0)
                    .dst_queue_family_index(0)
                    .buffer(*scene_frame.commands)
                    .offset(0)
                    .size(
                        (scene_frame.objects.len() * size_of::<DrawIndexedIndirectCommand>())
                            as u64,
                    )
                    .build(),
                BufferMemoryBarrier::builder()
                    .src_access_mask(AccessFlags::SHADER_WRITE)
                    .dst_access_mask(AccessFlags::TRANSFER_READ)
                    .src_queue_family_index(0)
                    .dst_queue_family_index(0)
                    .buffer(*scene_frame.draw_count)
                    .offset(0)
                    .size(size_of::<u32>() as u64)
                    .build(),
            ];
            command_buffer.pipeline_barrier(
                PipelineStageFlags::COMPUTE_SHADER,
                PipelineStageFlags::DRAW_INDIRECT | PipelineStageFlags::TRANSFER,
                DependencyFlags::empty(),
                &[],
                &buffer_memory_barriers,
                &[],
            );
        }
        let regions = [BufferCopy::builder()
            .src_offset(0)
            .dst_offset(0)
            .size(size_of::<u32>() as u64)
            .build()];
        command_buffer.copy_buffer(
            &scene_frame.draw_count,
            scene_frame.draw_count_read_back.buffer(),
            &regions,
        );
        Ok(())
    }

    pub fn render(
        &mut self,
        root_context: &RootContext,
        render_pass: &RenderPass,
        root_frame: &mut RootFrame,
        pre_pass: bool,
    ) -> Result<(), vulkan::Error> {
        self.terrain
            .render(root_context, render_pass, root_frame, pre_pass)?;
        self.scene
            .render(root_context, render_pass, root_frame, pre_pass)?;
        Ok(())
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        Depth::pool_setup() + Cull::pool_setup() + Scene::pool_setup() + Terrain::pool_setup()
    }
}
