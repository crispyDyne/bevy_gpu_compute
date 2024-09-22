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

/// This example uses a shader source file from the assets subdirectory
const SHADER_COMPUTE_PATH: &str = "compute.wgsl";
const SHADER_ASSET_PATH: &str = "render.wgsl";

const DISPLAY_FACTOR: u32 = 4;
const SIZE: (u32, u32) = (1280 / DISPLAY_FACTOR, 720 / DISPLAY_FACTOR);
const WORKGROUP_SIZE: u32 = 8;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
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
            }),
            // .set(ImagePlugin::default_nearest()),
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

#[derive(Resource, Clone, ExtractResource)]
struct ComputeMaterialHandle(Handle<ExtendedMaterial<StandardMaterial, ComputeMaterial>>);

// This struct defines the data that will be passed to your shader
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ComputeMaterial {
    #[storage(100, read_only)]
    compute: Handle<ShaderStorageBuffer>,
}

impl MaterialExtension for ComputeMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
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
        transform: Transform::from_xyz(-10.0, -1.0, 1.0)
            .looking_at(Vec3::new(0., 0., 0.5), Vec3::Z),
        ..Default::default()
    });

    commands.spawn((DirectionalLightBundle {
        transform: Transform::from_xyz(-10.0, 5.0, 2.0).looking_at(Vec3::ZERO, Vec3::Z),
        directional_light: DirectionalLight {
            illuminance: 100000.0,
            ..Default::default()
        },
        ..Default::default()
    },));

    let particle_count = 1000;

    // Initialize particle data
    let particles = vec![
        Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
        };
        particle_count as usize
    ];

    let mut storage_buffer = ShaderStorageBuffer::from(particles.clone());
    storage_buffer.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
    storage_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

    let storage_buffer_handle = buffers.add(storage_buffer);
    let compute_material = ComputeMaterial {
        compute: storage_buffer_handle.clone(),
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
    let mesh = Sphere::new(1.0);
    for particle in particles {
        commands.spawn(MaterialMeshBundle {
            mesh: meshes.add(mesh.clone()),
            material: material_handle.clone(),
            transform: Transform::from_translation(particle.position),
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
    // material_handle: Res<ComputeMaterialHandle>,
    // materials: ResMut<Assets<ComputeMaterial>>,
    // mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
) {
    // let material = materials.get_mut(&material_handle.0).unwrap();
    // let buffer = buffers.get_mut(&material.compute).unwrap();

    println!("Pipeline - prepare_bind_group");
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
                        panic!("Initializing assets/{SHADER_COMPUTE_PATH}:\n{err}")
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
                let bind_group = &world.resource::<ParticleBindGroups>().0;
                println!("Pipeline - run - Init");
                let init_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.init_pipeline)
                    .unwrap();
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_pipeline(init_pipeline);
                pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
            }
            ParticleState::Update(index) => {
                let bind_group = &world.resource::<ParticleBindGroups>().0;
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
