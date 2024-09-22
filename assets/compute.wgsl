// Define Particle struct
struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
}

struct ParticleConfig {
    count: u32,
}

@group(0) @binding(100) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(101) var<uniform> particleConfig: ParticleConfig;

@compute @workgroup_size(64, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // Optional initialization logic can be added here
}

@compute @workgroup_size(64, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    // Get the particle index
    let index = invocation_id.x;


    // Update the particle's velocity and position
    particles[index].velocity.z -= 0.002; // Gravity effect with speed factor
    particles[index].position.z += particles[index].velocity.z * 0.01; // Position update
}
