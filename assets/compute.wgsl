// Define Particle struct
struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
}

struct ParticleConfig {
    count: u32,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> particleConfig: ParticleConfig;

@compute @workgroup_size(64, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // Optional initialization logic can be added here
}

@compute @workgroup_size(64, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    if (invocation_id.x >= particleConfig.count) {
        return;
    }

    // Get the particle index
    let index = invocation_id.x;

    // Calculate a speed factor based on the invocation id
    let speed_factor = 2.0 * ((f32(index) % 3.14159) / 3.14159 - 0.5); // Adjusts speed per particle

    // Update the particle's velocity and position
    particles[index].velocity.z -= 0.1 * speed_factor; // Gravity effect with speed factor
    particles[index].position.z += particles[index].velocity.z * 0.01; // Position update
}
