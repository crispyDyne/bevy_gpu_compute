//! A compute shader that simulates particle motion

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetUsages, RenderAssets},
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        Render, RenderApp, RenderSet,
    },
};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;

const SHADER_COMPUTE_PATH: &str = "compute.wgsl";
const SHADER_RENDER_PATH: &str = "render.wgsl";

const DEPTH: u32 = 100;
const WIDTH: u32 = 100;
const WORKGROUP_SIZE: u32 = 32;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // uncomment for unthrottled FPS
                    // present_mode: bevy::window::PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
            MaterialPlugin::<ExtendedMaterial<StandardMaterial, ComputeMaterial>>::default(),
            ParticleComputePlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
}

#[repr(C)]
#[derive(Default, ShaderType, Clone, Copy, Zeroable, Pod)]
pub struct ParticleConfig {
    pub particle_count: u32,
}

#[derive(Resource, Clone, ExtractResource)]
struct ParticleConfigBuffer {
    buffer: Buffer,
}

#[allow(dead_code)] // should be able to use this instead of the ParticleConfigBuffer
#[derive(Resource, Clone, ExtractResource)]
struct ComputeMaterialHandle(Handle<ExtendedMaterial<StandardMaterial, ComputeMaterial>>);

// This struct defines the data that will be passed to your shader
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ComputeMaterial {
    #[storage(100, read_only)]
    compute: Handle<ShaderStorageBuffer>,
    #[uniform(101)]
    particle_count: u32,
}

impl MaterialExtension for ComputeMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_RENDER_PATH.into()
    }
}
#[derive(Resource, Clone, ExtractResource)]
struct StorageBufferID {
    storage_buffer_id: AssetId<ShaderStorageBuffer>,
}

fn setup(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, ComputeMaterial>>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-50.0, -3.0, -20.0)
            .looking_at(Vec3::new(0.0, 0.0, 20.0), Vec3::Z),
        ..Default::default()
    });

    commands.spawn((DirectionalLightBundle {
        transform: Transform::from_xyz(-5.0, 5.0, 2.0).looking_at(Vec3::ZERO, Vec3::Z),
        directional_light: DirectionalLight {
            illuminance: 10000.0,
            ..Default::default()
        },
        ..Default::default()
    },));

    let particle_count = WIDTH * DEPTH;

    let mut particles = vec![];

    let half_depth = DEPTH as f32 / 2.0;
    let half_width = WIDTH as f32 / 2.0;
    for x in 0..DEPTH {
        for y in 0..WIDTH {
            let position = Vec3::new(
                (x as f32 - half_depth) / half_depth,
                (y as f32 - half_width) / half_width,
                0.0,
            );
            particles.push(Particle {
                position,
                velocity: Vec3::new(
                    0.1 * position.y / (position.length().powi(2) + 0.2),
                    -0.1 * position.x / (position.length().powi(2) + 0.2),
                    100.0 * ((position.y + position.x) as f32 % 0.0314159),
                ),
            });
        }
    }

    let mut storage_buffer = ShaderStorageBuffer::from(particles.clone());
    storage_buffer.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
    storage_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

    let storage_buffer_handle = buffers.add(storage_buffer);
    let compute_material = ComputeMaterial {
        compute: storage_buffer_handle.clone(),
        particle_count,
    };

    let extended_material = ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            ..Default::default()
        },
        extension: compute_material,
    };
    let material_handle = materials.add(extended_material);
    let storage_buffer_id = storage_buffer_handle.id();
    println!("Storage Buffer ID: {:?}", storage_buffer_id);
    commands.insert_resource(StorageBufferID { storage_buffer_id });

    // Create the particle config buffer
    let particle_config = ParticleConfig { particle_count };
    let config_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("Particle Config Buffer"),
        contents: bytemuck::cast_slice(&[particle_config]),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    commands.insert_resource(ParticleConfigBuffer {
        buffer: config_buffer,
    });

    // create meshes for particles
    let mesh = Sphere::new(2.0 / (particle_count as f32).sqrt()).mesh();
    let mesh_handle = meshes.add(mesh.clone());
    for _ in 0..particle_count {
        commands.spawn(MaterialMeshBundle {
            mesh: mesh_handle.clone(),
            material: material_handle.clone(),
            ..Default::default()
        });
    }
    commands.insert_resource(ComputeMaterialHandle(material_handle.clone()));
}

struct ParticleComputePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ParticleLabel;

impl Plugin for ParticleComputePlugin {
    fn build(&self, app: &mut App) {
        // Extract the game of life image resource from the main world into the render world
        // for operation on by the compute shader and display on the sprite.
        app.add_plugins(ExtractResourcePlugin::<ParticleConfigBuffer>::default());
        app.add_plugins(ExtractResourcePlugin::<StorageBufferID>::default());
        app.add_plugins(ExtractResourcePlugin::<ComputeMaterialHandle>::default());
        let render_app = app.sub_app_mut(RenderApp);
        // This seems wrong. The prepare_bind_group should only be called once, but it is called
        // every frame.
        render_app.add_systems(Render, prepare_bind_group.in_set(RenderSet::Prepare));

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
    config_buffer: Res<ParticleConfigBuffer>,
    render_device: Res<RenderDevice>,
    storage_buffer_id: Res<StorageBufferID>,
    render_assets: Res<RenderAssets<GpuShaderStorageBuffer>>,
    // local variable that tracks if the system has already run
    mut ran: Local<bool>,
) {
    if *ran {
        // seems like there should be a better way to do this
        return;
    }
    println!("Pipeline - prepare_bind_group");
    *ran = true;
    let storage_buffer = render_assets
        .get(storage_buffer_id.storage_buffer_id.clone())
        .unwrap();

    let particle_bind_group = render_device.create_bind_group(
        "Particle Bind Group",
        &pipeline.particle_bind_group_layout,
        &[
            BindGroupEntry {
                binding: 100,
                resource: storage_buffer.buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 101,
                resource: config_buffer.buffer.as_entire_binding(),
            },
        ],
    );

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
            &[
                BindGroupLayoutEntry {
                    binding: 100,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 101,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: bevy::render::render_resource::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(ParticleConfig::min_size()),
                    },
                    count: None,
                },
            ],
        );

        let shader = world.load_asset(SHADER_COMPUTE_PATH);
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
    Update,
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
                        self.state = ParticleState::Init;
                    }
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SHADER_COMPUTE_PATH}:\n{err}")
                    }
                    _ => {
                        // waiting for the pipeline to load
                    }
                }
            }
            ParticleState::Init => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                {
                    self.state = ParticleState::Update;
                }
            }
            ParticleState::Update => {}
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleComputePipeline>();

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());

        let particle_count = WIDTH * DEPTH;
        let workgroup_count = (particle_count as f32 / WORKGROUP_SIZE as f32).ceil() as u32;

        // select the pipeline based on the current state
        match self.state {
            ParticleState::Loading => {}
            ParticleState::Init => {
                let bind_group = &world.resource::<ParticleBindGroups>().0;
                let init_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.init_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_pipeline(init_pipeline);
                pass.dispatch_workgroups(workgroup_count, 1, 1);
            }
            ParticleState::Update => {
                let bind_group = &world.resource::<ParticleBindGroups>().0;
                let update_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.update_pipeline)
                    .unwrap();

                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_pipeline(update_pipeline);
                pass.dispatch_workgroups(workgroup_count, 1, 1);
            }
        }

        Ok(())
    }
}
