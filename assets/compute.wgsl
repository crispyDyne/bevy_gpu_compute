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

@compute @workgroup_size(32, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // Optional initialization logic can be added here
}

@compute @workgroup_size(32, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>) {

    let index = invocation_id.x; // Get the particle index
    if index >= particleConfig.count {
        // not sure if this is nessary
        // Without it, is there a risk that some workgroups will access data out of bounds?
        return;
    }

    let dt = 0.02; // Time delta
    var force = vec3<f32>(0.0, 0.0, 0.0); // Force accumulator

    // Bounce off the floor
    if particles[index].position.z < -0.1 {
        let deflection = -0.1 - particles[index].position.z;
        force.z += 10.0 * deflection * dt; // Bounce
    }

    // Gravity between particles
    for (var i: u32 = 0u; i < particleConfig.count; i++) {
        if i == index {
            continue;
        }

        let delta = particles[i].position - particles[index].position;
        let distance = length(delta)+0.1;
        let direction = delta / distance;
        force += direction * 0.0003 / (distance * distance) ;
    }

    // Gravity
    force.z -= 0.02;

    // Integrate the particle's velocity and position
    particles[index].position += particles[index].velocity * dt; // Integrate Position
    particles[index].velocity += force * dt; // Integrate Velocity (mass = 1)
}
