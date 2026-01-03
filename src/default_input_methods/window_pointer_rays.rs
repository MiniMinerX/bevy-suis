use std::cmp::Ordering;

use bevy::{
    camera::RenderTarget, ecs::{lifecycle::HookContext, world::DeferredWorld}, input::mouse::MouseWheel, prelude::*, window::{PrimaryWindow, WindowRef}
};

use crate::{
    InputMethodDisabled, SuisPreUpdateSets,
    input_method::InputMethod,
    input_method_data::{NonSpatialInputData, SpatialInputData},
    order_helper::InputHandlerQueryHelper,
};

pub struct SuisWindowPointerRayPlugin;

impl Plugin for SuisWindowPointerRayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SuisMouseConfig>();
        app.register_type::<SuisCameraRay>();

        app.add_systems(
            PreUpdate,
            (update_camera_ray, update_mouse_data)
                .chain()
                .in_set(SuisPreUpdateSets::UpdateInputMethods),
        );
    }
}

/// Attach this to any Camera you want to act as a SUIS pointer source.
/// The `pointer_entity` is automatically spawned via the `on_add` hook.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[component(on_add = spawn_pointer_for_camera)]
pub struct SuisCameraRay {
    pub pointer_entity: Entity,
}

impl Default for SuisCameraRay {
    fn default() -> Self {
        Self { pointer_entity: Entity::PLACEHOLDER }
    }
}

/// Hook that spawns the actual SUIS InputMethod entity when the tag is added to a camera.
fn spawn_pointer_for_camera(mut world: bevy::ecs::world::DeferredWorld, ctx: bevy::ecs::lifecycle::HookContext) {
    let pointer = world
        .commands()
        .spawn((
            Name::new("Camera Mouse Pointer"),
            InputMethod::new(),
            SpatialInputData::Ray(Ray3d::new(Vec3::ZERO, Dir3::NEG_Z)),
            MouseInputMethod,
            NonSpatialInputData::default(),
            Transform::default(),
        ))
        .id();

    // Store the spawned entity back into the component on the camera
    if let Some(mut comp) = world.entity_mut(ctx.entity).get_mut::<SuisCameraRay>() {
        comp.pointer_entity = pointer;
    }
}

#[derive(Clone, Copy, Component, Debug, Default)]
pub struct MouseInputMethod;

#[derive(Resource)]
pub struct SuisMouseConfig {
    pub discrete_multiplier: f32,
    pub continuous_multiplier: f32,
}
impl Default for SuisMouseConfig {
    fn default() -> Self {
        SuisMouseConfig {
            discrete_multiplier: 0.02,
            continuous_multiplier: 0.002,
        }
    }
}

/// Updates the 3D Ray based on the camera's viewport and mouse position.
fn update_camera_ray(
    primary_window: Query<Entity, With<PrimaryWindow>>,
    windows: Query<&Window>,
    cams: Query<(&Camera, &GlobalTransform, &SuisCameraRay)>,
    mut input_methods: Query<(
        &mut SpatialInputData,
        &mut InputMethod,
        &mut Transform,
        Has<InputMethodDisabled>,
    )>,
    mut cmds: Commands,
) {
    let Ok(primary_window_ent) = primary_window.single() else { return; };

    for (camera, cam_global_transform, suis_ray) in cams.iter() {
        let window_ent = match camera.target {
            RenderTarget::Window(WindowRef::Primary) => primary_window_ent,
            RenderTarget::Window(WindowRef::Entity(e)) => e,
            _ => continue,
        };

        let Ok(window) = windows.get(window_ent) else { continue; };
        let Ok((mut spatial_data, mut input_method, mut handler_transform, disabled)) =
            input_methods.get_mut(suis_ray.pointer_entity)
        else { continue; };

        let mut ray_found = false;

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(ray) = camera.viewport_to_world(cam_global_transform, cursor_pos) {
                *spatial_data = SpatialInputData::Ray(ray);

                // 1. Position: Origin of the ray
                handler_transform.translation = ray.origin;

                // 2. Rotation: Construct a stable frame using the Camera's Right vector
                // This prevents axial spinning because the ray's "Up" is tied to the Camera's "Up"
                let ray_forward = *ray.direction; // This is our new Z (forward)
                let cam_right = cam_global_transform.right(); // Use camera's stable X axis
                
                // Calculate the ray's local Up by crossing Forward and Right
                // Bevy is Right-Handed: Cross(Forward, Right) = Up
                let ray_up = ray_forward.cross(cam_right.into()).normalize_or(Vec3::Y);
                
                // Create the rotation from these look-at parameters
                handler_transform.rotation = Quat::from_mat3(&Mat3::from_cols(
                    ray_up.cross(ray_forward).normalize(), // Local Right
                    ray_up,                                // Local Up
                    -ray_forward,                          // Local Forward (Bevy uses -Z)
                ));

                // 3. Scale: Force normalization
                handler_transform.scale = Vec3::ONE;

                ray_found = true;

                if disabled {
                    cmds.entity(suis_ray.pointer_entity)
                        .remove::<InputMethodDisabled>();
                }
            }
        }

        if !ray_found && !disabled {
            cmds.entity(suis_ray.pointer_entity)
                .insert(InputMethodDisabled);
        }
    }
}

fn update_mouse_data(
    mut query: Query<
        (
            &mut NonSpatialInputData,
            &mut InputMethod,
            &SpatialInputData,
        ),
        With<MouseInputMethod>,
    >,
    mut scroll: EventReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    config: Res<SuisMouseConfig>,
    handler_query: InputHandlerQueryHelper,
) {
    // Accumulate scroll for the frame
    let mut scroll_delta = Vec2::ZERO;
    for e in scroll.read() {
        match e.unit {
            bevy::input::mouse::MouseScrollUnit::Line => {
                scroll_delta += Vec2::new(e.x, e.y) * config.discrete_multiplier;
            }
            bevy::input::mouse::MouseScrollUnit::Pixel => {
                scroll_delta += Vec2::new(e.x, e.y) * config.continuous_multiplier;
            }
        }
    }

    for (mut data, mut input_method, spatial_data) in query.iter_mut() {
        data.select = buttons.pressed(MouseButton::Left) as u8 as f32;
        data.context = buttons.pressed(MouseButton::Middle) as u8 as f32;
        data.secondary = buttons.pressed(MouseButton::Right) as u8 as f32;
        data.grab = buttons.pressed(MouseButton::Right) as u8 as f32;
        data.scroll = Some(scroll_delta);

        // Sorting handlers based on spatial distance
        let mut handlers = handler_query.query_all_handler_fields(|(handler, field, field_transform)| {
            (handler, spatial_data.distance(field, field_transform))
        });

        handlers.sort_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(Ordering::Equal));
        let handler_order = handlers.into_iter().map(|(e, _)| e).collect();
        input_method.set_handler_order(handler_order);
    }
}