#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip
}

struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
}

@group(2) @binding(100) var<storage, read> particles: array<Particle, 1000>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let computed_position = vertex.position + particles[vertex.instance_index].position;
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(computed_position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);

    // // color changes based on z position positive is red, negative is blue
    // let z = computed_position.z;
    // let red = saturate(z / 10.0);
    // let blue = saturate(-z / 10.0);


    // out.color = vec4<f32>(red, 0.0, blue, 1.0);

    return out;
}