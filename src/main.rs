//! A compute shader that simulates particle motion

use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        Render, RenderApp, RenderSet,
    },
};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

/// This example uses a shader source file from the assets subdirectory
const SHADER_ASSET_PATH: &str = "compute.wgsl";

const DISPLAY_FACTOR: u32 = 4;
const SIZE: (u32, u32) = (1280 / DISPLAY_FACTOR, 720 / DISPLAY_FACTOR);
const WORKGROUP_SIZE: u32 = 8;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (
                            (SIZE.0 * DISPLAY_FACTOR) as f32,
                            (SIZE.1 * DISPLAY_FACTOR) as f32,
                        )
                            .into(),
                        // uncomment for unthrottled FPS
                        // present_mode: bevy::window::PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            ParticleComputePlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
}

#[derive(Resource, Clone, ExtractResource)]
pub struct ParticleBuffer {
    pub buffer: Buffer,
}

pub fn setup(mut commands: Commands, render_device: Res<RenderDevice>) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 100.0)),
        ..Default::default()
    });

    let particle_count = 1000;

    // Initialize particle data
    let particles = vec![
        Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
        };
        particle_count as usize
    ];

    // Create the buffer
    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("Particle Buffer"),
        contents: bytemuck::cast_slice(&particles),
        usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
    });

    commands.insert_resource(ParticleBuffer { buffer });
}

struct ParticleComputePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ParticleLabel;

impl Plugin for ParticleComputePlugin {
    fn build(&self, app: &mut App) {
        // Extract the game of life image resource from the main world into the render world
        // for operation on by the compute shader and display on the sprite.
        app.add_plugins(ExtractResourcePlugin::<ParticleBuffer>::default());
        let render_app = app.sub_app_mut(RenderApp);
        // This seems wrong. The prepare_bind_group should only be called once, but it is called
        // every frame.
        render_app.add_systems(
            Render,
            prepare_bind_group.in_set(RenderSet::PrepareBindGroups),
        );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(ParticleLabel, ParticleNode::default());
        render_graph.add_node_edge(ParticleLabel, bevy::render::graph::CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app.init_resource::<ParticleComputePipeline>();
    }
}

#[derive(Resource)]
struct ParticleBindGroups(BindGroup);

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ParticleComputePipeline>,
    particle_buffer: Res<ParticleBuffer>, // Access the ParticleBuffer resource
    render_device: Res<RenderDevice>,
) {
    let particle_bind_group = render_device.create_bind_group(
        "Particle Bind Group",
        &pipeline.particle_bind_group_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: particle_buffer.buffer.as_entire_binding(),
        }],
    );

    println!("Pipeline - prepare_bind_group");
    commands.insert_resource(ParticleBindGroups(particle_bind_group));
}

#[derive(Resource)]
struct ParticleComputePipeline {
    particle_bind_group_layout: BindGroupLayout,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
}

impl FromWorld for ParticleComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let particle_bind_group_layout = render_device.create_bind_group_layout(
            "Particle Bind Group Layout",
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        let shader = world.load_asset(SHADER_ASSET_PATH);
        let pipeline_cache = world.resource::<PipelineCache>();
        let init_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: vec![particle_bind_group_layout.clone()],
            push_constant_ranges: Vec::new(),
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Cow::from("init"),
        });
        let update_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: vec![particle_bind_group_layout.clone()],
            push_constant_ranges: Vec::new(),
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("update"),
        });

        Self {
            particle_bind_group_layout,
            init_pipeline,
            update_pipeline,
        }
    }
}

enum ParticleState {
    Loading,
    Init,
    Update(usize),
}

struct ParticleNode {
    state: ParticleState,
}

impl Default for ParticleNode {
    fn default() -> Self {
        Self {
            state: ParticleState::Loading,
        }
    }
}

impl render_graph::Node for ParticleNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<ParticleComputePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        // if the corresponding pipeline has loaded, transition to the next stage
        match self.state {
            ParticleState::Loading => {
                match pipeline_cache.get_compute_pipeline_state(pipeline.init_pipeline) {
                    CachedPipelineState::Ok(_) => {
                        println!("Pipeline - update -  Loading");
                        self.state = ParticleState::Init;
                    }
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SHADER_ASSET_PATH}:\n{err}")
                    }
                    _ => {}
                }
            }
            ParticleState::Init => {
                println!("Pipeline - update - Init");
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                {
                    self.state = ParticleState::Update(1);
                }
            }
            ParticleState::Update(0) => {
                println!("Pipeline - update - Update 0");
                self.state = ParticleState::Update(1);
            }
            ParticleState::Update(1) => {
                println!("Pipeline - update - Update 1");
                self.state = ParticleState::Update(0);
            }
            ParticleState::Update(_) => unreachable!(),
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_group = &world.resource::<ParticleBindGroups>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleComputePipeline>();

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());

        // select the pipeline based on the current state
        match self.state {
            ParticleState::Loading => {
                println!("Pipeline - run - Loading");
            }
            ParticleState::Init => {
                println!("Pipeline - run - Init");
                let init_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.init_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_pipeline(init_pipeline);
                pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
            }
            ParticleState::Update(index) => {
                println!("Pipeline - run - Update");
                let update_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.update_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_pipeline(update_pipeline);
                pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
            }
        }

        Ok(())
    }
}
