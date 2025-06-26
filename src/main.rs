use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::collections::VecDeque;

pub struct TrailPlugin;

impl Plugin for TrailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_trails, generate_trail_meshes));
    }
}

#[derive(Component)]
pub struct Trail {
    /// Maximum number of trail points to keep
    pub max_points: usize,
    /// How often to add new trail points (in seconds)
    pub emit_rate: f32,
    /// Width of the trail
    pub width: f32,
    /// Material handle for the trail
    pub material: Handle<StandardMaterial>,
    /// Internal timer for emission
    pub(crate) timer: Timer,
    /// Stored trail points
    pub(crate) points: VecDeque<TrailPoint>,
    /// Generated mesh entity
    pub(crate) mesh_entity: Option<Entity>,
}

#[derive(Clone)]
struct TrailPoint {
    position: Vec3,
    timestamp: f32,
}

impl Trail {
    pub fn new(
        max_points: usize,
        emit_rate: f32,
        width: f32,
        material: Handle<StandardMaterial>,
    ) -> Self {
        Self {
            max_points,
            emit_rate,
            width,
            material,
            timer: Timer::from_seconds(1.0 / emit_rate, TimerMode::Repeating),
            points: VecDeque::new(),
            mesh_entity: None,
        }
    }
}

fn update_trails(
    mut commands: Commands,
    time: Res<Time>,
    mut trail_query: Query<(Entity, &mut Trail, &Transform)>,
) {
    for (entity, mut trail, transform) in trail_query.iter_mut() {
        trail.timer.tick(time.delta());
        
        // Add new trail point if timer elapsed
        if trail.timer.just_finished() {
            let new_point = TrailPoint {
                position: transform.translation,
                timestamp: time.elapsed_seconds(),
            };
            
            trail.points.push_back(new_point);
            
            // Remove old points if we exceed max_points
            while trail.points.len() > trail.max_points {
                trail.points.pop_front();
            }
        }
        
        // Remove points that are too old (optional fade-out based on time)
        let current_time = time.elapsed_seconds();
        let max_age = 5.0; // Trail points live for 5 seconds
        
        while let Some(front) = trail.points.front() {
            if current_time - front.timestamp > max_age {
                trail.points.pop_front();
            } else {
                break;
            }
        }
        
        // Clean up mesh entity if no points remain
        if trail.points.is_empty() {
            if let Some(mesh_entity) = trail.mesh_entity {
                commands.entity(mesh_entity).despawn();
                trail.mesh_entity = None;
            }
        }
    }
}

fn generate_trail_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut trail_query: Query<&mut Trail>,
) {
    for mut trail in trail_query.iter_mut() {
        if trail.points.len() < 2 {
            continue;
        }
        
        let mesh = create_trail_mesh(&trail.points, trail.width);
        let mesh_handle = meshes.add(mesh);
        
        // Remove old mesh entity if it exists
        if let Some(old_entity) = trail.mesh_entity {
            commands.entity(old_entity).despawn();
        }
        
        // Spawn new mesh entity
        let mesh_entity = commands.spawn(PbrBundle {
            mesh: mesh_handle,
            material: trail.material.clone(),
            ..default()
        }).id();
        
        trail.mesh_entity = Some(mesh_entity);
    }
}

fn create_trail_mesh(points: &VecDeque<TrailPoint>, width: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    
    if points.len() < 2 {
        return Mesh::new(PrimitiveTopology::TriangleList, default());
    }
    
    let half_width = width * 0.5;
    
    // Generate vertices along the trail
    for (i, point) in points.iter().enumerate() {
        let progress = i as f32 / (points.len() - 1) as f32;
        
        // Calculate direction vector
        let (forward, right) = if i == 0 {
            // First point - use direction to next point
            let next = &points[i + 1];
            let dir = (next.position - point.position).normalize_or_zero();
            let right = if dir.dot(Vec3::Y).abs() < 0.9 {
                dir.cross(Vec3::Y).normalize()
            } else {
                dir.cross(Vec3::X).normalize()
            };
            (dir, right)
        } else if i == points.len() - 1 {
            // Last point - use direction from previous point
            let prev = &points[i - 1];
            let dir = (point.position - prev.position).normalize_or_zero();
            let right = if dir.dot(Vec3::Y).abs() < 0.9 {
                dir.cross(Vec3::Y).normalize()
            } else {
                dir.cross(Vec3::X).normalize()
            };
            (dir, right)
        } else {
            // Middle point - average of directions
            let prev = &points[i - 1];
            let next = &points[i + 1];
            let dir = ((point.position - prev.position) + (next.position - point.position))
                .normalize_or_zero();
            let right = if dir.dot(Vec3::Y).abs() < 0.9 {
                dir.cross(Vec3::Y).normalize()
            } else {
                dir.cross(Vec3::X).normalize()
            };
            (dir, right)
        };
        
        // Calculate width based on progress (taper towards end)
        let current_width = half_width * progress; //(1.0 - progress * 1.);
        
        // Add left and right vertices
        let left_pos = point.position - right * current_width;
        let right_pos = point.position + right * current_width;
        
        vertices.push([left_pos.x, left_pos.y, left_pos.z]);
        vertices.push([right_pos.x, right_pos.y, right_pos.z]);
        
        // Add normals (pointing up for now, could be improved)
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
        
        // Add UVs
        uvs.push([0.0, progress]);
        uvs.push([1.0, progress]);
    }
    
    // Generate indices for triangles
    for i in 0..(points.len() - 1) {
        let base = i * 2;
        
        // First triangle
        indices.push(base as u32);
        indices.push((base + 1) as u32);
        indices.push((base + 2) as u32);
        
        // Second triangle
        indices.push((base + 1) as u32);
        indices.push((base + 3) as u32);
        indices.push((base + 2) as u32);
    }
    
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    
    mesh
}

// Example usage and demo scene
#[derive(Component)]
struct MovingObject {
    speed: f32,
    radius: f32,
    time: f32,
}

pub fn setup_trail_demo(
    assets: Res<AssetServer>, 
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    
    // Add light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            illuminance: 10000.0,
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
        ..default()
    });
    
    // Create trail material
    let trail_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.2, 0.2, 0.8),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    
    // Spawn moving object with trail
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(0.2)),
            material: materials.add(StandardMaterial {
                base_color: LinearRgba::RED.into(),
                ..default()
            }),
            transform: Transform::from_xyz(3.0, 0.0, 0.0),
            ..default()
        },
        Trail::new(50, 12.0, 0.5, trail_material.clone()),
        MovingObject {
            speed: 2.0,
            radius: 3.0,
            time: 0.0,
        },
    ));
    
    // Add another moving object with different trail
    let trail_material2 = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 1.0, 0.2, 0.8),
        base_color_texture: Some(assets.load("splatter1.png")),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(0.15)),
            material: materials.add(StandardMaterial {
                base_color: LinearRgba::GREEN.into(),
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 2.0, 0.0),
            ..default()
        },
        Trail::new(80, 45.0, 5.3, trail_material2),
        MovingObject {
            speed: 1.5,
            radius: 2.0,
            time: 1.57, // Quarter phase offset
        },
    ));
}

pub fn move_objects(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut MovingObject)>, //, Without<Trail>>,
) {
    for (mut transform, mut obj) in query.iter_mut() {
        obj.time += time.delta_seconds() * obj.speed;
        
        // Circular motion
        transform.translation.x = obj.radius * obj.time.cos();
        transform.translation.z = obj.radius * obj.time.sin();
        
        // Add some vertical motion
        transform.translation.y = 1.0 + 0.5 * (obj.time * 2.0).sin();
    }
}

// Complete example app
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TrailPlugin)
        .add_systems(Startup, setup_trail_demo)
        .add_systems(Update, move_objects)
        .run();
}