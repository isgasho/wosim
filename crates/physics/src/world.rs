use nalgebra::Isometry3;
use rapier3d::{
    na::{vector, Vector3},
    prelude::{
        BroadPhase, CCDSolver, Collider, ColliderHandle, ColliderSet, IntegrationParameters,
        InteractionGroups, IslandManager, JointSet, NarrowPhase, PhysicsPipeline, QueryPipeline,
        Ray, RigidBody, RigidBodyBuilder, RigidBodyHandle, RigidBodySet, RigidBodyType,
    },
};

pub struct World {
    physics_pipeline: PhysicsPipeline,
    pub colliders: ColliderSet,
    pub bodies: RigidBodySet,
    integration_parameters: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    joints: JointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    events: (),
    pub gravity: Vector3<f32>,
    pub query_pipeline: QueryPipeline,
}

impl World {
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.joints,
            &mut self.ccd_solver,
            &self.physics_hooks,
            &self.events,
        );
        self.query_pipeline
            .update(&self.islands, &self.bodies, &self.colliders);
    }

    pub fn add_collider(&mut self, collider: Collider) -> ColliderHandle {
        self.colliders.insert(collider)
    }

    pub fn remove_collider(&mut self, handle: ColliderHandle, wake_up: bool) -> Option<Collider> {
        self.colliders
            .remove(handle, &mut self.islands, &mut self.bodies, wake_up)
    }

    pub fn remove_body(&mut self, handle: RigidBodyHandle) -> RigidBody {
        self.bodies
            .remove(
                handle,
                &mut self.islands,
                &mut self.colliders,
                &mut self.joints,
            )
            .unwrap()
    }

    pub fn cast_ray(
        &self,
        ray: &Ray,
        max_toi: f32,
        solid: bool,
        query_groups: InteractionGroups,
        filter: Option<&dyn Fn(ColliderHandle) -> bool>,
    ) -> Option<(ColliderHandle, f32)> {
        self.query_pipeline
            .cast_ray(&self.colliders, ray, max_toi, solid, query_groups, filter)
    }

    pub fn add_body(
        &mut self,
        body_type: RigidBodyType,
        position: Isometry3<f32>,
        collider: Collider,
        user_data: u128,
    ) -> RigidBodyHandle {
        let body = match body_type {
            RigidBodyType::Dynamic => RigidBodyBuilder::new_dynamic(),
            RigidBodyType::Static => RigidBodyBuilder::new_static(),
            RigidBodyType::KinematicPositionBased => {
                RigidBodyBuilder::new_kinematic_position_based()
            }
            RigidBodyType::KinematicVelocityBased => {
                RigidBodyBuilder::new_kinematic_velocity_based()
            }
        }
        .position(position)
        .user_data(user_data)
        .build();
        let handle = self.bodies.insert(body);
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        handle
    }

    pub fn set_next_position(&mut self, handle: RigidBodyHandle, pos: Isometry3<f32>) {
        self.bodies[handle].set_next_kinematic_position(pos)
    }
}

impl Default for World {
    fn default() -> Self {
        Self {
            colliders: ColliderSet::new(),
            gravity: vector![0.0, -9.81, 0.0],
            physics_pipeline: PhysicsPipeline::new(),
            integration_parameters: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            joints: JointSet::new(),
            ccd_solver: CCDSolver::new(),
            bodies: RigidBodySet::new(),
            events: (),
            physics_hooks: (),
            query_pipeline: QueryPipeline::new(),
        }
    }
}
