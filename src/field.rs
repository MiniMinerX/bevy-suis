use bevy::{
    math::{Vec3A, vec3a},
    prelude::*,
};

pub const RAYMARCH_MAX_STEPS: u32 = 1000;
pub const RAYMARCH_MIN_STEP_SIZE: f32 = 0.001;
pub const RAYMARCH_MAX_DISTANCE: f32 = 10_000.0;

pub struct RayMarchResult {
    pub closest_distance: f32,
    pub deepest_point_ray_length: f32,
    pub ray_lenght: f32,
    pub ray_steps: u32,
}

#[derive(Component, Debug, Clone, Copy)]
pub enum Field {
    Sphere(f32),
    Cuboid(Cuboid),
    Torus(Torus),
    Cylinder(Cylinder),
}
impl Field {
    pub fn closest_point(
        &self,
        field_transform: &GlobalTransform,
        point: impl Into<Vec3A>,
    ) -> Vec3A {
        let point = point.into();
        point - self.normal(field_transform, point) * self.distance(field_transform, point)
    }
    /// point should be in world-space
    pub fn normal(&self, field_transform: &GlobalTransform, point: impl Into<Vec3A>) -> Dir3A {
        let point = point.into();
        let distance_vec = Vec3A::splat(self.distance(field_transform, point));
        const R: f32 = 0.0001;
        let r_vec = Vec3A::new(
            self.distance(field_transform, point + vec3a(R, 0.0, 0.0)),
            self.distance(field_transform, point + vec3a(0.0, R, 0.0)),
            self.distance(field_transform, point + vec3a(0.0, 0.0, R)),
        );
        let local_normal = distance_vec - r_vec;
        Dir3A::new(-field_transform.affine().transform_vector3a(local_normal)).unwrap()
    }
    /// point should be in world-space
    pub fn distance(&self, field_transform: &GlobalTransform, point: impl Into<Vec3A>) -> f32 {
        let point = point.into();
        
        // 1. Handle Scaling: To get accurate world-space distances, 
        // we need to know the scale of the object.
        let (scale, _rotation, translation) = field_transform.to_scale_rotation_translation();
        let uniform_scale = scale.max_element(); // SDFs work best with uniform scaling

        // 2. Transform point to local space manually to handle scaling properly
        let rotation_inv = _rotation.inverse();
        // Convert translation to Vec3A to match point's type
        let translation = Vec3A::from(translation);
        let p = rotation_inv * (point - translation);
        let p = p / uniform_scale; // Normalize to local unit space

        let local_dist = match self {
            Field::Sphere(radius) => p.length() - radius,
            Field::Cuboid(cuboid) => {
                // Ensure that both operands are Vec3A by converting cuboid.half_size to Vec3A if necessary
                let q = p.abs() - Vec3A::from(cuboid.half_size);
                let outside = q.max(Vec3A::ZERO).length();
                let inside = q.x.max(q.y).max(q.z).min(0.0);
                outside + inside
            }
            Field::Torus(torus) => {
                let q = Vec2::new(p.xz().length() - torus.major_radius, p.y);
                q.length() - torus.minor_radius
            }
            Field::Cylinder(cylinder) => {
                let d = Vec2::new(p.xz().length(), p.y).abs() - 
                        Vec2::new(cylinder.radius, cylinder.half_height);
                d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
            }
        };

        // 3. Scale the distance back to world units
        local_dist * uniform_scale
    }

    pub fn raymarch(&self, field_transform: &GlobalTransform, ray: Ray3d) -> RayMarchResult {
        let mut result = RayMarchResult {
            closest_distance: f32::MAX,
            deepest_point_ray_length: 0.,
            ray_lenght: 0.,
            ray_steps: 0,
        };

        // Standard epsilon for "hitting" a surface
        const HIT_EPSILON: f32 = 0.001;

        while result.ray_steps < RAYMARCH_MAX_STEPS && result.ray_lenght < RAYMARCH_MAX_DISTANCE {
            let point = ray.origin + (ray.direction.as_vec3() * result.ray_lenght);
            let distance = self.distance(field_transform, point);

            // Update closest approach tracking
            if distance < result.closest_distance {
                result.closest_distance = distance;
                result.deepest_point_ray_length = result.ray_lenght;
            }

            // HIT DETECTION: Stop if we are close enough to the surface
            if distance < HIT_EPSILON {
                break; 
            }

            // Move the ray forward by the safe distance
            // We use a small minimum to prevent infinite loops at surface edges
            result.ray_lenght += distance.max(RAYMARCH_MIN_STEP_SIZE);
            result.ray_steps += 1;
        }

        result
    }
}
