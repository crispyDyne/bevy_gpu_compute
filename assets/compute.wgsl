// define Particle struct
struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
// @group(0) @binding(1) var<uniform> particleCount: u32;


@compute @workgroup_size(8, 8, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // nothing to do
}


@compute @workgroup_size(8, 8, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // // apply gravity
    // for (var i: u32 = 0u; i < particleCount; i++) {
    //     particles[i].velocity.z -= 0.1;
    // }

    // // update position
    // for (var i: u32 = 0u; i < particleCount; i++) {
    //     particles[i].position += particles[i].velocity * 0.01;
    // }
}